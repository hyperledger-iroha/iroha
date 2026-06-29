#![allow(unexpected_cfgs)]

//! SoraFS proof-of-personhood credential payloads and deterministic validators.

use std::collections::BTreeSet;

use blake3::Hasher;
use ed25519_dalek::{
    PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, Signature as DalekSignature, Signer, SigningKey, Verifier,
    VerifyingKey,
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

/// Schema version for [`PopCredentialV1`].
pub const POP_CREDENTIAL_VERSION_V1: u8 = 1;
/// Schema version for [`PopCommitmentRootV1`].
pub const POP_COMMITMENT_ROOT_VERSION_V1: u8 = 1;
/// Schema version for [`PopRevocationListV1`].
pub const POP_REVOCATION_LIST_VERSION_V1: u8 = 1;
/// Schema version for [`PopIssuedCredentialBundleV1`].
pub const POP_ISSUED_CREDENTIAL_BUNDLE_VERSION_V1: u8 = 1;
/// Schema version for [`PopEnrollmentRequestV1`].
pub const POP_ENROLLMENT_REQUEST_VERSION_V1: u8 = 1;
/// Schema version for [`PopRenewalRequestV1`].
pub const POP_RENEWAL_REQUEST_VERSION_V1: u8 = 1;
/// Schema version for [`PopMembershipProofV1`].
pub const POP_MEMBERSHIP_PROOF_VERSION_V1: u8 = 1;

const POP_CREDENTIAL_SIGNATURE_DOMAIN_V1: &[u8] = b"sorafs.pop.credential.signature.v1";
const POP_ROOT_SIGNATURE_DOMAIN_V1: &[u8] = b"sorafs.pop.commitment-root.signature.v1";
const POP_REVOCATION_SIGNATURE_DOMAIN_V1: &[u8] = b"sorafs.pop.revocation-list.signature.v1";
const POP_MEMBERSHIP_PROOF_DOMAIN_V1: &[u8] = b"sorafs.pop.membership-proof.transcript.v1";

/// Proof-of-personhood credential class used for juror eligibility routing.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
#[norito(tag = "class", content = "value", rename_all = "snake_case")]
pub enum PopEligibilityClassV1 {
    /// General juror pool.
    General,
    /// Region-scoped juror pool.
    Regional,
    /// Domain-expert juror pool.
    Expert,
    /// Emergency juror pool.
    Emergency,
    /// Observer-only credential that cannot vote.
    Observer,
}

/// Signature algorithm used by SFM-4b1 payload publishers.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
#[norito(tag = "algorithm", content = "value", rename_all = "snake_case")]
pub enum PopSignatureAlgorithmV1 {
    /// Ed25519 signatures over canonical Norito payload digests.
    Ed25519,
}

/// Membership proof transcript scheme.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
#[norito(tag = "proof_system", content = "value", rename_all = "snake_case")]
pub enum PopMembershipProofSystemV1 {
    /// Deterministic transcript digest for local policy fixtures and reference validation.
    ///
    /// This proves only transcript consistency. Production PoP verification rejects this
    /// proof system until a privacy-preserving membership proof verifier is selected.
    TranscriptDigestV1,
}

/// Credential attribute commitment.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PopCredentialAttributeV1 {
    /// Stable attribute name.
    pub key: String,
    /// BLAKE3-256 commitment to the attribute value and salt.
    pub value_commitment: [u8; 32],
}

impl PopCredentialAttributeV1 {
    /// Validate the committed attribute shape.
    pub fn validate(&self) -> Result<(), PopCredentialValidationError> {
        validate_text("attribute key", &self.key)?;
        validate_digest("attribute value commitment", self.value_commitment)?;
        Ok(())
    }
}

/// Detached signature attached to PoP credential payloads.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PopSignatureV1 {
    /// Signature algorithm.
    pub algorithm: PopSignatureAlgorithmV1,
    /// Public key bytes.
    pub public_key: Vec<u8>,
    /// Raw signature bytes.
    pub signature: Vec<u8>,
}

impl PopSignatureV1 {
    /// Validate signature material for the advertised algorithm.
    pub fn validate(&self) -> Result<(), PopCredentialValidationError> {
        match self.algorithm {
            PopSignatureAlgorithmV1::Ed25519 => {
                if self.public_key.len() != PUBLIC_KEY_LENGTH {
                    return Err(PopCredentialValidationError::InvalidPublicKeyLength {
                        length: self.public_key.len(),
                    });
                }
                if self.signature.len() != SIGNATURE_LENGTH {
                    return Err(PopCredentialValidationError::InvalidSignatureLength {
                        length: self.signature.len(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Proof-of-personhood credential issued to a juror wallet.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PopCredentialV1 {
    /// Schema version (`POP_CREDENTIAL_VERSION_V1`).
    pub version: u8,
    /// Unique credential identifier.
    pub credential_id: [u8; 32],
    /// Holder commitment; the canonical account identity is not embedded.
    pub holder_commitment: [u8; 32],
    /// Juror eligibility class granted by the credential.
    pub eligibility_class: PopEligibilityClassV1,
    /// Committed attributes used by future policy checks.
    pub attributes: Vec<PopCredentialAttributeV1>,
    /// Issuer identifier.
    pub issuer_id: String,
    /// Unix epoch seconds when the credential was issued.
    pub issued_at_epoch: u64,
    /// Unix epoch seconds when the credential expires.
    pub expires_at_epoch: u64,
    /// Earliest Unix epoch seconds when renewal can be requested.
    pub renewal_at_epoch: u64,
    /// Revocation nonce listed by governance if the credential is revoked.
    pub revocation_nonce: [u8; 32],
    /// Commitment root containing this credential.
    pub commitment_root: [u8; 32],
    /// Commitment tree version containing this credential.
    pub commitment_tree_version: u64,
    /// Revocation list version observed at issuance.
    pub revocation_list_version: u64,
    /// Issuer signature over the canonical credential payload.
    pub issuer_signature: PopSignatureV1,
}

impl PopCredentialV1 {
    /// Validate structural invariants that do not depend on wall-clock time.
    pub fn validate(&self) -> Result<(), PopCredentialValidationError> {
        if self.version != POP_CREDENTIAL_VERSION_V1 {
            return Err(PopCredentialValidationError::UnsupportedVersion {
                payload: "credential",
                found: self.version,
            });
        }
        validate_digest("credential id", self.credential_id)?;
        validate_digest("holder commitment", self.holder_commitment)?;
        validate_attributes(&self.attributes)?;
        validate_text("issuer id", &self.issuer_id)?;
        if self.issued_at_epoch == 0 {
            return Err(PopCredentialValidationError::InvalidTimestamp {
                field: "issued_at_epoch",
            });
        }
        if self.issued_at_epoch >= self.expires_at_epoch {
            return Err(PopCredentialValidationError::InvalidValidityWindow {
                starts_at_epoch: self.issued_at_epoch,
                ends_at_epoch: self.expires_at_epoch,
            });
        }
        if self.renewal_at_epoch < self.issued_at_epoch
            || self.renewal_at_epoch > self.expires_at_epoch
        {
            return Err(PopCredentialValidationError::InvalidRenewalEpoch {
                renewal_at_epoch: self.renewal_at_epoch,
                issued_at_epoch: self.issued_at_epoch,
                expires_at_epoch: self.expires_at_epoch,
            });
        }
        validate_digest("revocation nonce", self.revocation_nonce)?;
        validate_digest("commitment root", self.commitment_root)?;
        if self.commitment_tree_version == 0 {
            return Err(PopCredentialValidationError::InvalidVersionCounter {
                field: "commitment_tree_version",
            });
        }
        if self.revocation_list_version == 0 {
            return Err(PopCredentialValidationError::InvalidVersionCounter {
                field: "revocation_list_version",
            });
        }
        self.issuer_signature.validate()
    }

    /// Validate structural invariants and expiry at `now_epoch`.
    pub fn validate_at(&self, now_epoch: u64) -> Result<(), PopCredentialValidationError> {
        self.validate()?;
        if now_epoch >= self.expires_at_epoch {
            return Err(PopCredentialValidationError::ExpiredCredential {
                now_epoch,
                expires_at_epoch: self.expires_at_epoch,
            });
        }
        Ok(())
    }
}

/// Published commitment root for the active PoP credential set.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PopCommitmentRootV1 {
    /// Schema version (`POP_COMMITMENT_ROOT_VERSION_V1`).
    pub version: u8,
    /// Root digest of the credential commitment tree.
    pub root_digest: [u8; 32],
    /// Number of leaves included in the root.
    pub tree_size: u64,
    /// Monotonic commitment tree version.
    pub tree_version: u64,
    /// Issuer identifier.
    pub issuer_id: String,
    /// Unix epoch seconds when the root was published.
    pub published_at_epoch: u64,
    /// Previous root digest, if this is not the initial root.
    pub previous_root_digest: Option<[u8; 32]>,
    /// Governance event digest that authorised this root.
    pub governance_event_digest: [u8; 32],
    /// Publisher signature over the canonical root payload.
    pub publisher_signature: PopSignatureV1,
}

impl PopCommitmentRootV1 {
    /// Validate root publication invariants.
    pub fn validate(&self) -> Result<(), PopCredentialValidationError> {
        if self.version != POP_COMMITMENT_ROOT_VERSION_V1 {
            return Err(PopCredentialValidationError::UnsupportedVersion {
                payload: "commitment root",
                found: self.version,
            });
        }
        validate_digest("commitment root", self.root_digest)?;
        if self.tree_size == 0 {
            return Err(PopCredentialValidationError::InvalidVersionCounter { field: "tree_size" });
        }
        if self.tree_version == 0 {
            return Err(PopCredentialValidationError::InvalidVersionCounter {
                field: "tree_version",
            });
        }
        validate_text("issuer id", &self.issuer_id)?;
        if self.published_at_epoch == 0 {
            return Err(PopCredentialValidationError::InvalidTimestamp {
                field: "published_at_epoch",
            });
        }
        if let Some(previous) = self.previous_root_digest {
            validate_digest("previous commitment root", previous)?;
            if previous == self.root_digest {
                return Err(PopCredentialValidationError::DuplicateDigest {
                    field: "previous_root_digest",
                });
            }
        }
        validate_digest("governance event digest", self.governance_event_digest)?;
        self.publisher_signature.validate()
    }
}

/// Governance reason attached to a revocation entry.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
#[norito(tag = "reason", content = "value", rename_all = "snake_case")]
pub enum PopRevocationReasonV1 {
    /// Credential was rotated to a newer nonce.
    Rotated,
    /// Holder requested withdrawal.
    HolderRequested,
    /// Issuer found enrollment evidence invalid.
    EnrollmentInvalid,
    /// Governance suspended the credential.
    GovernanceSuspension,
    /// Credential expired and was removed from active proof sets.
    Expired,
}

/// Single PoP credential revocation entry.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct PopRevocationEntryV1 {
    /// Revocation nonce from the credential.
    pub nonce: [u8; 32],
    /// Unix epoch seconds when the nonce was revoked.
    pub revoked_at_epoch: u64,
    /// Governance reason code.
    pub reason: PopRevocationReasonV1,
}

impl PopRevocationEntryV1 {
    /// Validate a revocation entry.
    pub fn validate(&self) -> Result<(), PopCredentialValidationError> {
        validate_digest("revocation nonce", self.nonce)?;
        if self.revoked_at_epoch == 0 {
            return Err(PopCredentialValidationError::InvalidTimestamp {
                field: "revoked_at_epoch",
            });
        }
        Ok(())
    }
}

/// Published revocation list for PoP credential nonces.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PopRevocationListV1 {
    /// Schema version (`POP_REVOCATION_LIST_VERSION_V1`).
    pub version: u8,
    /// Monotonic revocation list version.
    pub list_version: u64,
    /// Commitment root this list applies to.
    pub commitment_root: [u8; 32],
    /// Issuer identifier.
    pub issuer_id: String,
    /// Unix epoch seconds when the list was published.
    pub published_at_epoch: u64,
    /// Sorted revocation entries.
    pub entries: Vec<PopRevocationEntryV1>,
    /// Publisher signature over the canonical revocation-list payload.
    pub publisher_signature: PopSignatureV1,
}

impl PopRevocationListV1 {
    /// Validate revocation-list invariants.
    pub fn validate(&self) -> Result<(), PopCredentialValidationError> {
        if self.version != POP_REVOCATION_LIST_VERSION_V1 {
            return Err(PopCredentialValidationError::UnsupportedVersion {
                payload: "revocation list",
                found: self.version,
            });
        }
        if self.list_version == 0 {
            return Err(PopCredentialValidationError::InvalidVersionCounter {
                field: "list_version",
            });
        }
        validate_digest("commitment root", self.commitment_root)?;
        validate_text("issuer id", &self.issuer_id)?;
        if self.published_at_epoch == 0 {
            return Err(PopCredentialValidationError::InvalidTimestamp {
                field: "published_at_epoch",
            });
        }

        let mut previous_nonce = None;
        for entry in &self.entries {
            entry.validate()?;
            if let Some(previous) = previous_nonce {
                if previous == entry.nonce {
                    return Err(PopCredentialValidationError::DuplicateRevocationNonce);
                }
                if previous > entry.nonce {
                    return Err(PopCredentialValidationError::UnsortedRevocationList);
                }
            }
            previous_nonce = Some(entry.nonce);
        }
        self.publisher_signature.validate()
    }

    /// Returns true if the nonce appears in the revocation list.
    #[must_use]
    pub fn contains_nonce(&self, nonce: [u8; 32]) -> bool {
        self.entries
            .binary_search_by_key(&nonce, |entry| entry.nonce)
            .is_ok()
    }
}

/// Cohesive issuer publication emitted when a PoP credential is issued.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PopIssuedCredentialBundleV1 {
    /// Schema version (`POP_ISSUED_CREDENTIAL_BUNDLE_VERSION_V1`).
    pub version: u8,
    /// Signed credential issued to the juror wallet.
    pub credential: PopCredentialV1,
    /// Signed commitment-root publication that includes the credential.
    pub commitment_root: PopCommitmentRootV1,
    /// Signed revocation-list snapshot observed at issuance.
    pub revocation_list: PopRevocationListV1,
}

impl PopIssuedCredentialBundleV1 {
    /// Validate issuer signatures and cross-publication consistency.
    pub fn validate(&self) -> Result<(), PopCredentialValidationError> {
        if self.version != POP_ISSUED_CREDENTIAL_BUNDLE_VERSION_V1 {
            return Err(PopCredentialValidationError::UnsupportedVersion {
                payload: "issued credential bundle",
                found: self.version,
            });
        }
        verify_pop_credential_signature_v1(&self.credential)?;
        verify_pop_commitment_root_signature_v1(&self.commitment_root)?;
        verify_pop_revocation_list_signature_v1(&self.revocation_list)?;
        validate_pop_issuer_publications_v1(
            &self.credential,
            &self.commitment_root,
            &self.revocation_list,
        )
    }
}

/// Enrollment request submitted before credential issuance.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PopEnrollmentRequestV1 {
    /// Schema version (`POP_ENROLLMENT_REQUEST_VERSION_V1`).
    pub version: u8,
    /// Unique enrollment request identifier.
    pub request_id: [u8; 32],
    /// Canonical applicant identifier or account alias supplied to the issuer.
    pub applicant_id: String,
    /// Requested eligibility class.
    pub requested_class: PopEligibilityClassV1,
    /// Requested attribute keys.
    pub requested_attributes: Vec<String>,
    /// Digest of off-chain enrollment attestations.
    pub attestation_digest: [u8; 32],
    /// Unix epoch seconds when the request was submitted.
    pub submitted_at_epoch: u64,
    /// Unix epoch seconds after which the request is stale.
    pub expires_at_epoch: u64,
}

impl PopEnrollmentRequestV1 {
    /// Validate enrollment request invariants.
    pub fn validate(&self) -> Result<(), PopCredentialValidationError> {
        if self.version != POP_ENROLLMENT_REQUEST_VERSION_V1 {
            return Err(PopCredentialValidationError::UnsupportedVersion {
                payload: "enrollment request",
                found: self.version,
            });
        }
        validate_digest("enrollment request id", self.request_id)?;
        validate_text("applicant id", &self.applicant_id)?;
        validate_text_list("requested attribute", &self.requested_attributes)?;
        validate_digest("attestation digest", self.attestation_digest)?;
        if self.submitted_at_epoch == 0 {
            return Err(PopCredentialValidationError::InvalidTimestamp {
                field: "submitted_at_epoch",
            });
        }
        if self.submitted_at_epoch >= self.expires_at_epoch {
            return Err(PopCredentialValidationError::InvalidValidityWindow {
                starts_at_epoch: self.submitted_at_epoch,
                ends_at_epoch: self.expires_at_epoch,
            });
        }
        Ok(())
    }
}

/// Renewal request submitted before credential rotation.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PopRenewalRequestV1 {
    /// Schema version (`POP_RENEWAL_REQUEST_VERSION_V1`).
    pub version: u8,
    /// Unique renewal request identifier.
    pub request_id: [u8; 32],
    /// Credential being renewed.
    pub previous_credential_id: [u8; 32],
    /// Current holder commitment.
    pub holder_commitment: [u8; 32],
    /// New holder commitment to use after rotation.
    pub rotation_commitment: [u8; 32],
    /// Requested new expiry.
    pub requested_expires_at_epoch: u64,
    /// Unix epoch seconds when the request was submitted.
    pub submitted_at_epoch: u64,
    /// Digest of renewal attestation evidence.
    pub attestation_digest: [u8; 32],
}

impl PopRenewalRequestV1 {
    /// Validate renewal request invariants.
    pub fn validate(&self) -> Result<(), PopCredentialValidationError> {
        if self.version != POP_RENEWAL_REQUEST_VERSION_V1 {
            return Err(PopCredentialValidationError::UnsupportedVersion {
                payload: "renewal request",
                found: self.version,
            });
        }
        validate_digest("renewal request id", self.request_id)?;
        validate_digest("previous credential id", self.previous_credential_id)?;
        validate_digest("holder commitment", self.holder_commitment)?;
        validate_digest("rotation commitment", self.rotation_commitment)?;
        if self.rotation_commitment == self.holder_commitment {
            return Err(PopCredentialValidationError::DuplicateDigest {
                field: "rotation_commitment",
            });
        }
        if self.submitted_at_epoch == 0 {
            return Err(PopCredentialValidationError::InvalidTimestamp {
                field: "submitted_at_epoch",
            });
        }
        if self.submitted_at_epoch >= self.requested_expires_at_epoch {
            return Err(PopCredentialValidationError::InvalidValidityWindow {
                starts_at_epoch: self.submitted_at_epoch,
                ends_at_epoch: self.requested_expires_at_epoch,
            });
        }
        validate_digest("attestation digest", self.attestation_digest)?;
        Ok(())
    }
}

/// Membership proof presented by a juror client for a verifier challenge.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PopMembershipProofV1 {
    /// Schema version (`POP_MEMBERSHIP_PROOF_VERSION_V1`).
    pub version: u8,
    /// Unique proof identifier.
    pub proof_id: [u8; 32],
    /// Credential identifier bound to the proof.
    pub credential_id: [u8; 32],
    /// Holder commitment disclosed to the proof verifier.
    pub holder_commitment: [u8; 32],
    /// Eligibility class proven by the credential.
    pub eligibility_class: PopEligibilityClassV1,
    /// Commitment root used by the proof.
    pub commitment_root: [u8; 32],
    /// Commitment tree version used by the proof.
    pub commitment_tree_version: u64,
    /// Revocation list version used by the proof.
    pub revocation_list_version: u64,
    /// Per-verifier nullifier for replay prevention.
    pub nullifier: [u8; 32],
    /// Verifier challenge digest.
    pub challenge_digest: [u8; 32],
    /// Domain/context string supplied by the verifier.
    pub verifier_context: String,
    /// Membership proof system.
    pub proof_system: PopMembershipProofSystemV1,
    /// Deterministic proof transcript digest.
    pub proof_digest: [u8; 32],
    /// Unix epoch seconds when this proof expires.
    pub expires_at_epoch: u64,
}

impl PopMembershipProofV1 {
    /// Validate proof shape and transcript digest.
    pub fn validate(&self) -> Result<(), PopCredentialValidationError> {
        if self.version != POP_MEMBERSHIP_PROOF_VERSION_V1 {
            return Err(PopCredentialValidationError::UnsupportedVersion {
                payload: "membership proof",
                found: self.version,
            });
        }
        validate_digest("proof id", self.proof_id)?;
        validate_digest("credential id", self.credential_id)?;
        validate_digest("holder commitment", self.holder_commitment)?;
        validate_digest("commitment root", self.commitment_root)?;
        if self.commitment_tree_version == 0 {
            return Err(PopCredentialValidationError::InvalidVersionCounter {
                field: "commitment_tree_version",
            });
        }
        if self.revocation_list_version == 0 {
            return Err(PopCredentialValidationError::InvalidVersionCounter {
                field: "revocation_list_version",
            });
        }
        validate_digest("nullifier", self.nullifier)?;
        validate_digest("challenge digest", self.challenge_digest)?;
        validate_text("verifier context", &self.verifier_context)?;
        validate_digest("proof digest", self.proof_digest)?;
        if self.expires_at_epoch == 0 {
            return Err(PopCredentialValidationError::InvalidTimestamp {
                field: "expires_at_epoch",
            });
        }
        let expected = pop_membership_proof_transcript_digest_v1(self)?;
        if self.proof_digest != expected {
            return Err(PopCredentialValidationError::ProofDigestMismatch);
        }
        Ok(())
    }

    /// Validate proof shape and expiry at `now_epoch`.
    pub fn validate_at(&self, now_epoch: u64) -> Result<(), PopCredentialValidationError> {
        self.validate()?;
        if now_epoch >= self.expires_at_epoch {
            return Err(PopCredentialValidationError::ExpiredProof {
                now_epoch,
                expires_at_epoch: self.expires_at_epoch,
            });
        }
        Ok(())
    }
}

/// Derive the canonical Ed25519 digest for a PoP credential signature.
pub fn pop_credential_signature_digest_v1(
    credential: &PopCredentialV1,
) -> Result<[u8; 32], PopCredentialValidationError> {
    let mut signable = credential.clone();
    signable.issuer_signature.signature.clear();
    pop_signature_digest(POP_CREDENTIAL_SIGNATURE_DOMAIN_V1, &signable)
}

/// Verify the issuer signature attached to a PoP credential.
pub fn verify_pop_credential_signature_v1(
    credential: &PopCredentialV1,
) -> Result<(), PopCredentialValidationError> {
    credential.validate()?;
    let digest = pop_credential_signature_digest_v1(credential)?;
    verify_pop_signature_v1(&credential.issuer_signature, &digest)
}

/// Sign a PoP credential with the canonical SFM-4b1 Ed25519 digest.
pub fn sign_pop_credential_ed25519_v1(
    mut credential: PopCredentialV1,
    signing_key: &SigningKey,
) -> Result<PopCredentialV1, PopCredentialValidationError> {
    credential.issuer_signature = empty_ed25519_pop_signature(signing_key);
    let digest = pop_credential_signature_digest_v1(&credential)?;
    credential.issuer_signature.signature = signing_key.sign(&digest).to_bytes().to_vec();
    verify_pop_credential_signature_v1(&credential)?;
    Ok(credential)
}

/// Derive the canonical Ed25519 digest for a commitment-root signature.
pub fn pop_commitment_root_signature_digest_v1(
    root: &PopCommitmentRootV1,
) -> Result<[u8; 32], PopCredentialValidationError> {
    let mut signable = root.clone();
    signable.publisher_signature.signature.clear();
    pop_signature_digest(POP_ROOT_SIGNATURE_DOMAIN_V1, &signable)
}

/// Verify the publisher signature attached to a commitment root.
pub fn verify_pop_commitment_root_signature_v1(
    root: &PopCommitmentRootV1,
) -> Result<(), PopCredentialValidationError> {
    root.validate()?;
    let digest = pop_commitment_root_signature_digest_v1(root)?;
    verify_pop_signature_v1(&root.publisher_signature, &digest)
}

/// Sign a commitment root with the canonical SFM-4b1 Ed25519 digest.
pub fn sign_pop_commitment_root_ed25519_v1(
    mut root: PopCommitmentRootV1,
    signing_key: &SigningKey,
) -> Result<PopCommitmentRootV1, PopCredentialValidationError> {
    root.publisher_signature = empty_ed25519_pop_signature(signing_key);
    let digest = pop_commitment_root_signature_digest_v1(&root)?;
    root.publisher_signature.signature = signing_key.sign(&digest).to_bytes().to_vec();
    verify_pop_commitment_root_signature_v1(&root)?;
    Ok(root)
}

/// Derive the canonical Ed25519 digest for a revocation-list signature.
pub fn pop_revocation_list_signature_digest_v1(
    revocations: &PopRevocationListV1,
) -> Result<[u8; 32], PopCredentialValidationError> {
    let mut signable = revocations.clone();
    signable.publisher_signature.signature.clear();
    pop_signature_digest(POP_REVOCATION_SIGNATURE_DOMAIN_V1, &signable)
}

/// Verify the publisher signature attached to a revocation list.
pub fn verify_pop_revocation_list_signature_v1(
    revocations: &PopRevocationListV1,
) -> Result<(), PopCredentialValidationError> {
    revocations.validate()?;
    let digest = pop_revocation_list_signature_digest_v1(revocations)?;
    verify_pop_signature_v1(&revocations.publisher_signature, &digest)
}

/// Sign a revocation list with the canonical SFM-4b1 Ed25519 digest.
pub fn sign_pop_revocation_list_ed25519_v1(
    mut revocations: PopRevocationListV1,
    signing_key: &SigningKey,
) -> Result<PopRevocationListV1, PopCredentialValidationError> {
    revocations.publisher_signature = empty_ed25519_pop_signature(signing_key);
    let digest = pop_revocation_list_signature_digest_v1(&revocations)?;
    revocations.publisher_signature.signature = signing_key.sign(&digest).to_bytes().to_vec();
    verify_pop_revocation_list_signature_v1(&revocations)?;
    Ok(revocations)
}

/// Sign and validate the canonical issuer bundle for a newly issued credential.
pub fn issue_pop_credential_bundle_ed25519_v1(
    credential: PopCredentialV1,
    commitment_root: PopCommitmentRootV1,
    revocation_list: PopRevocationListV1,
    signing_key: &SigningKey,
) -> Result<PopIssuedCredentialBundleV1, PopCredentialValidationError> {
    let bundle = PopIssuedCredentialBundleV1 {
        version: POP_ISSUED_CREDENTIAL_BUNDLE_VERSION_V1,
        credential: sign_pop_credential_ed25519_v1(credential, signing_key)?,
        commitment_root: sign_pop_commitment_root_ed25519_v1(commitment_root, signing_key)?,
        revocation_list: sign_pop_revocation_list_ed25519_v1(revocation_list, signing_key)?,
    };
    bundle.validate()?;
    Ok(bundle)
}

/// Derive the deterministic transcript digest for a membership proof.
pub fn pop_membership_proof_transcript_digest_v1(
    proof: &PopMembershipProofV1,
) -> Result<[u8; 32], PopCredentialValidationError> {
    let mut transcript = proof.clone();
    transcript.proof_digest = [0; 32];
    pop_signature_digest(POP_MEMBERSHIP_PROOF_DOMAIN_V1, &transcript)
}

/// Fill and validate the deterministic transcript digest for a membership proof.
pub fn finalize_pop_membership_proof_digest_v1(
    mut proof: PopMembershipProofV1,
) -> Result<PopMembershipProofV1, PopCredentialValidationError> {
    proof.proof_digest = pop_membership_proof_transcript_digest_v1(&proof)?;
    proof.validate()?;
    Ok(proof)
}

/// Verify a PoP membership proof against active root and revocation material.
///
/// The production verifier is fail-closed while V1 only has the local
/// transcript-digest proof foundation. Use
/// [`verify_pop_membership_transcript_policy_v1`] only for local fixture and
/// reference-policy checks that intentionally accept transcript-digest proofs.
pub fn verify_pop_membership_proof_v1(
    proof: &PopMembershipProofV1,
    _credential: &PopCredentialV1,
    _commitment_root: &PopCommitmentRootV1,
    _revocations: &PopRevocationListV1,
    _now_epoch: u64,
    _seen_nullifiers: &[[u8; 32]],
) -> Result<(), PopCredentialValidationError> {
    match proof.proof_system {
        PopMembershipProofSystemV1::TranscriptDigestV1 => {
            Err(PopCredentialValidationError::PolicyOnlyProofSystem)
        }
    }
}

/// Verify a transcript-digest PoP proof for local policy fixtures.
///
/// This helper performs the deterministic transcript, expiry, root,
/// revocation-list, revoked-nonce, and nullifier checks used by local reference
/// tooling. It is not a production privacy-preserving membership verifier.
pub fn verify_pop_membership_transcript_policy_v1(
    proof: &PopMembershipProofV1,
    credential: &PopCredentialV1,
    commitment_root: &PopCommitmentRootV1,
    revocations: &PopRevocationListV1,
    now_epoch: u64,
    seen_nullifiers: &[[u8; 32]],
) -> Result<(), PopCredentialValidationError> {
    match proof.proof_system {
        PopMembershipProofSystemV1::TranscriptDigestV1 => {}
    }
    proof.validate_at(now_epoch)?;
    credential.validate_at(now_epoch)?;
    commitment_root.validate()?;
    revocations.validate()?;
    verify_pop_credential_signature_v1(credential)?;
    verify_pop_commitment_root_signature_v1(commitment_root)?;
    verify_pop_revocation_list_signature_v1(revocations)?;

    if proof.credential_id != credential.credential_id {
        return Err(PopCredentialValidationError::ProofCredentialMismatch);
    }
    if proof.holder_commitment != credential.holder_commitment {
        return Err(PopCredentialValidationError::ProofHolderCommitmentMismatch);
    }
    if proof.eligibility_class != credential.eligibility_class {
        return Err(PopCredentialValidationError::ProofEligibilityClassMismatch);
    }
    if proof.commitment_root != commitment_root.root_digest
        || credential.commitment_root != commitment_root.root_digest
        || revocations.commitment_root != commitment_root.root_digest
    {
        return Err(PopCredentialValidationError::WrongCommitmentRoot);
    }
    if proof.commitment_tree_version != commitment_root.tree_version
        || credential.commitment_tree_version != commitment_root.tree_version
    {
        return Err(PopCredentialValidationError::CommitmentTreeVersionMismatch);
    }
    if credential.issuer_id.as_str() != commitment_root.issuer_id.as_str()
        || credential.issuer_id.as_str() != revocations.issuer_id.as_str()
    {
        return Err(PopCredentialValidationError::IssuerMismatch);
    }
    if proof.revocation_list_version < revocations.list_version {
        return Err(PopCredentialValidationError::StaleRevocationList {
            proof_version: proof.revocation_list_version,
            current_version: revocations.list_version,
        });
    }
    if proof.revocation_list_version > revocations.list_version {
        return Err(
            PopCredentialValidationError::RevocationListVersionMismatch {
                proof_version: proof.revocation_list_version,
                current_version: revocations.list_version,
            },
        );
    }
    if credential.revocation_list_version > revocations.list_version {
        return Err(PopCredentialValidationError::CredentialRevocationListMismatch);
    }
    if revocations.contains_nonce(credential.revocation_nonce) {
        return Err(PopCredentialValidationError::RevokedCredential);
    }
    if seen_nullifiers.contains(&proof.nullifier) {
        return Err(PopCredentialValidationError::ReplayedProof);
    }
    Ok(())
}

/// Errors returned by SFM-4b1 PoP credential validators.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PopCredentialValidationError {
    /// Payload version is not supported.
    #[error("unsupported {payload} version {found}")]
    UnsupportedVersion { payload: &'static str, found: u8 },
    /// Required digest-like field is all zeroes.
    #[error("{field} must be non-zero")]
    InvalidDigest { field: &'static str },
    /// Text field is empty after trimming.
    #[error("{field} must not be empty")]
    EmptyText { field: &'static str },
    /// Text field contains control characters or surrounding whitespace.
    #[error("{field} contains invalid text")]
    InvalidText { field: &'static str },
    /// Duplicate committed attribute key.
    #[error("duplicate credential attribute key {key}")]
    DuplicateAttribute { key: String },
    /// Duplicate requested attribute key.
    #[error("duplicate requested attribute key {key}")]
    DuplicateRequestedAttribute { key: String },
    /// Timestamp field is invalid.
    #[error("{field} timestamp must be greater than zero")]
    InvalidTimestamp { field: &'static str },
    /// Monotonic counter field is invalid.
    #[error("{field} must be greater than zero")]
    InvalidVersionCounter { field: &'static str },
    /// Validity window is empty or inverted.
    #[error("validity window {starts_at_epoch}..{ends_at_epoch} is invalid")]
    InvalidValidityWindow {
        starts_at_epoch: u64,
        ends_at_epoch: u64,
    },
    /// Renewal epoch falls outside the credential validity window.
    #[error("renewal epoch {renewal_at_epoch} is outside {issued_at_epoch}..{expires_at_epoch}")]
    InvalidRenewalEpoch {
        renewal_at_epoch: u64,
        issued_at_epoch: u64,
        expires_at_epoch: u64,
    },
    /// A digest field duplicates a field that must rotate.
    #[error("{field} duplicates an incompatible digest")]
    DuplicateDigest { field: &'static str },
    /// Ed25519 public key length is invalid.
    #[error("ed25519 public key must be 32 bytes, got {length}")]
    InvalidPublicKeyLength { length: usize },
    /// Ed25519 signature length is invalid.
    #[error("ed25519 signature must be 64 bytes, got {length}")]
    InvalidSignatureLength { length: usize },
    /// Ed25519 public key bytes are invalid.
    #[error("invalid ed25519 public key: {reason}")]
    InvalidPublicKey { reason: String },
    /// Signature payload could not be encoded.
    #[error("failed to encode signature payload: {reason}")]
    SignaturePayloadEncoding { reason: String },
    /// Signature verification failed.
    #[error("signature verification failed: {reason}")]
    SignatureVerification { reason: String },
    /// Credential is expired.
    #[error("credential expired at {expires_at_epoch}, now {now_epoch}")]
    ExpiredCredential {
        now_epoch: u64,
        expires_at_epoch: u64,
    },
    /// Proof is expired.
    #[error("membership proof expired at {expires_at_epoch}, now {now_epoch}")]
    ExpiredProof {
        now_epoch: u64,
        expires_at_epoch: u64,
    },
    /// Revocation list entries are not sorted by nonce.
    #[error("revocation list entries must be sorted by nonce")]
    UnsortedRevocationList,
    /// Revocation list has duplicate nonces.
    #[error("revocation list has duplicate nonces")]
    DuplicateRevocationNonce,
    /// Proof digest does not match the canonical transcript.
    #[error("membership proof transcript digest mismatch")]
    ProofDigestMismatch,
    /// Proof credential id does not match the supplied credential.
    #[error("membership proof credential id mismatch")]
    ProofCredentialMismatch,
    /// Proof holder commitment does not match the supplied credential.
    #[error("membership proof holder commitment mismatch")]
    ProofHolderCommitmentMismatch,
    /// Proof eligibility class does not match the supplied credential.
    #[error("membership proof eligibility class mismatch")]
    ProofEligibilityClassMismatch,
    /// Commitment root does not match credential, proof, or revocation material.
    #[error("membership proof uses the wrong commitment root")]
    WrongCommitmentRoot,
    /// Commitment tree version does not match the published root.
    #[error("commitment tree version mismatch")]
    CommitmentTreeVersionMismatch,
    /// Credential, commitment root, and revocation list issuer ids do not match.
    #[error("credential, commitment root, and revocation list issuer ids must match")]
    IssuerMismatch,
    /// Credential, commitment root, and revocation list issuer public keys do not match.
    #[error("credential, commitment root, and revocation list issuer public keys must match")]
    IssuerKeyMismatch,
    /// Proof used a stale revocation list version.
    #[error(
        "membership proof revocation list version {proof_version} is stale; current {current_version}"
    )]
    StaleRevocationList {
        proof_version: u64,
        current_version: u64,
    },
    /// Proof used a revocation list version newer than the supplied list.
    #[error(
        "membership proof revocation list version {proof_version} does not match supplied {current_version}"
    )]
    RevocationListVersionMismatch {
        proof_version: u64,
        current_version: u64,
    },
    /// Credential was issued against a revocation list version that does not match supplied state.
    #[error("credential revocation list version does not match supplied revocation list")]
    CredentialRevocationListMismatch,
    /// Credential nonce is revoked.
    #[error("credential revocation nonce is revoked")]
    RevokedCredential,
    /// Proof nullifier has already been consumed.
    #[error("membership proof nullifier has already been consumed")]
    ReplayedProof,
    /// Proof system is policy-only and cannot satisfy production verification.
    #[error("membership proof system is policy-only and is not accepted by production verifier")]
    PolicyOnlyProofSystem,
}

fn pop_signature_digest<T: norito::core::NoritoSerialize>(
    domain: &[u8],
    payload: &T,
) -> Result<[u8; 32], PopCredentialValidationError> {
    let payload_bytes = norito::to_bytes(payload).map_err(|err| {
        PopCredentialValidationError::SignaturePayloadEncoding {
            reason: err.to_string(),
        }
    })?;
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(&payload_bytes);
    Ok(*hasher.finalize().as_bytes())
}

fn empty_ed25519_pop_signature(signing_key: &SigningKey) -> PopSignatureV1 {
    PopSignatureV1 {
        algorithm: PopSignatureAlgorithmV1::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: Vec::new(),
    }
}

fn verify_pop_signature_v1(
    signature: &PopSignatureV1,
    digest: &[u8; 32],
) -> Result<(), PopCredentialValidationError> {
    signature.validate()?;
    match signature.algorithm {
        PopSignatureAlgorithmV1::Ed25519 => verify_ed25519_pop_signature(signature, digest),
    }
}

fn verify_ed25519_pop_signature(
    signature: &PopSignatureV1,
    digest: &[u8; 32],
) -> Result<(), PopCredentialValidationError> {
    let mut public_key = [0u8; PUBLIC_KEY_LENGTH];
    public_key.copy_from_slice(&signature.public_key);
    let verifying_key = VerifyingKey::from_bytes(&public_key).map_err(|err| {
        PopCredentialValidationError::InvalidPublicKey {
            reason: err.to_string(),
        }
    })?;

    let mut signature_bytes = [0u8; SIGNATURE_LENGTH];
    signature_bytes.copy_from_slice(&signature.signature);
    let signature = DalekSignature::from_bytes(&signature_bytes);

    verifying_key.verify(digest, &signature).map_err(|err| {
        PopCredentialValidationError::SignatureVerification {
            reason: err.to_string(),
        }
    })
}

fn validate_digest(
    field: &'static str,
    digest: [u8; 32],
) -> Result<(), PopCredentialValidationError> {
    if digest.iter().all(|byte| *byte == 0) {
        return Err(PopCredentialValidationError::InvalidDigest { field });
    }
    Ok(())
}

fn validate_text(field: &'static str, value: &str) -> Result<(), PopCredentialValidationError> {
    if value.trim().is_empty() {
        return Err(PopCredentialValidationError::EmptyText { field });
    }
    if value != value.trim() || value.chars().any(char::is_control) {
        return Err(PopCredentialValidationError::InvalidText { field });
    }
    Ok(())
}

fn validate_attributes(
    attributes: &[PopCredentialAttributeV1],
) -> Result<(), PopCredentialValidationError> {
    let mut keys = BTreeSet::new();
    for attribute in attributes {
        attribute.validate()?;
        if !keys.insert(attribute.key.clone()) {
            return Err(PopCredentialValidationError::DuplicateAttribute {
                key: attribute.key.clone(),
            });
        }
    }
    Ok(())
}

fn validate_pop_issuer_publications_v1(
    credential: &PopCredentialV1,
    commitment_root: &PopCommitmentRootV1,
    revocation_list: &PopRevocationListV1,
) -> Result<(), PopCredentialValidationError> {
    if credential.commitment_root != commitment_root.root_digest
        || revocation_list.commitment_root != commitment_root.root_digest
    {
        return Err(PopCredentialValidationError::WrongCommitmentRoot);
    }
    if credential.commitment_tree_version != commitment_root.tree_version {
        return Err(PopCredentialValidationError::CommitmentTreeVersionMismatch);
    }
    if credential.issuer_id.as_str() != commitment_root.issuer_id.as_str()
        || credential.issuer_id.as_str() != revocation_list.issuer_id.as_str()
    {
        return Err(PopCredentialValidationError::IssuerMismatch);
    }
    if credential.issuer_signature.public_key.as_slice()
        != commitment_root.publisher_signature.public_key.as_slice()
        || credential.issuer_signature.public_key.as_slice()
            != revocation_list.publisher_signature.public_key.as_slice()
    {
        return Err(PopCredentialValidationError::IssuerKeyMismatch);
    }
    if credential.revocation_list_version != revocation_list.list_version {
        return Err(PopCredentialValidationError::CredentialRevocationListMismatch);
    }
    if revocation_list.contains_nonce(credential.revocation_nonce) {
        return Err(PopCredentialValidationError::RevokedCredential);
    }
    Ok(())
}

fn validate_text_list(
    field: &'static str,
    values: &[String],
) -> Result<(), PopCredentialValidationError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !seen.insert(value.clone()) {
            return Err(PopCredentialValidationError::DuplicateRequestedAttribute {
                key: value.clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn empty_signature() -> PopSignatureV1 {
        PopSignatureV1 {
            algorithm: PopSignatureAlgorithmV1::Ed25519,
            public_key: vec![1; 32],
            signature: vec![2; 64],
        }
    }

    fn credential() -> PopCredentialV1 {
        PopCredentialV1 {
            version: POP_CREDENTIAL_VERSION_V1,
            credential_id: digest(0x11),
            holder_commitment: digest(0x12),
            eligibility_class: PopEligibilityClassV1::General,
            attributes: vec![PopCredentialAttributeV1 {
                key: "residency".to_string(),
                value_commitment: digest(0x13),
            }],
            issuer_id: "issuer.sorafs".to_string(),
            issued_at_epoch: 100,
            expires_at_epoch: 1_000,
            renewal_at_epoch: 800,
            revocation_nonce: digest(0x14),
            commitment_root: digest(0x15),
            commitment_tree_version: 7,
            revocation_list_version: 3,
            issuer_signature: empty_signature(),
        }
    }

    fn root() -> PopCommitmentRootV1 {
        PopCommitmentRootV1 {
            version: POP_COMMITMENT_ROOT_VERSION_V1,
            root_digest: digest(0x15),
            tree_size: 32,
            tree_version: 7,
            issuer_id: "issuer.sorafs".to_string(),
            published_at_epoch: 120,
            previous_root_digest: Some(digest(0x16)),
            governance_event_digest: digest(0x17),
            publisher_signature: empty_signature(),
        }
    }

    fn revocations() -> PopRevocationListV1 {
        PopRevocationListV1 {
            version: POP_REVOCATION_LIST_VERSION_V1,
            list_version: 3,
            commitment_root: digest(0x15),
            issuer_id: "issuer.sorafs".to_string(),
            published_at_epoch: 130,
            entries: Vec::new(),
            publisher_signature: empty_signature(),
        }
    }

    fn enrollment() -> PopEnrollmentRequestV1 {
        PopEnrollmentRequestV1 {
            version: POP_ENROLLMENT_REQUEST_VERSION_V1,
            request_id: digest(0x21),
            applicant_id: "alice@sora".to_string(),
            requested_class: PopEligibilityClassV1::General,
            requested_attributes: vec!["residency".to_string()],
            attestation_digest: digest(0x22),
            submitted_at_epoch: 100,
            expires_at_epoch: 200,
        }
    }

    fn renewal() -> PopRenewalRequestV1 {
        PopRenewalRequestV1 {
            version: POP_RENEWAL_REQUEST_VERSION_V1,
            request_id: digest(0x31),
            previous_credential_id: digest(0x11),
            holder_commitment: digest(0x12),
            rotation_commitment: digest(0x32),
            requested_expires_at_epoch: 2_000,
            submitted_at_epoch: 900,
            attestation_digest: digest(0x33),
        }
    }

    fn proof() -> PopMembershipProofV1 {
        finalize_pop_membership_proof_digest_v1(PopMembershipProofV1 {
            version: POP_MEMBERSHIP_PROOF_VERSION_V1,
            proof_id: digest(0x41),
            credential_id: digest(0x11),
            holder_commitment: digest(0x12),
            eligibility_class: PopEligibilityClassV1::General,
            commitment_root: digest(0x15),
            commitment_tree_version: 7,
            revocation_list_version: 3,
            nullifier: digest(0x42),
            challenge_digest: digest(0x43),
            verifier_context: "jury-case-1".to_string(),
            proof_system: PopMembershipProofSystemV1::TranscriptDigestV1,
            proof_digest: [0; 32],
            expires_at_epoch: 2_000,
        })
        .expect("proof digest")
    }

    fn signed_material() -> (PopCredentialV1, PopCommitmentRootV1, PopRevocationListV1) {
        let key = signing_key(0x55);
        let credential =
            sign_pop_credential_ed25519_v1(credential(), &key).expect("credential signature");
        let root = sign_pop_commitment_root_ed25519_v1(root(), &key).expect("root signature");
        let revocations = sign_pop_revocation_list_ed25519_v1(revocations(), &key)
            .expect("revocations signature");
        (credential, root, revocations)
    }

    fn issued_bundle() -> PopIssuedCredentialBundleV1 {
        issue_pop_credential_bundle_ed25519_v1(
            credential(),
            root(),
            revocations(),
            &signing_key(0x55),
        )
        .expect("issue bundle")
    }

    fn norito_roundtrip<T>(value: &T) -> T
    where
        T: norito::core::NoritoSerialize
            + for<'decode> norito::NoritoDeserialize<'decode>
            + PartialEq
            + std::fmt::Debug,
    {
        let bytes = norito::to_bytes(value).expect("encode");
        let decoded = norito::decode_from_bytes(&bytes).expect("decode");
        assert_eq!(value, &decoded);
        decoded
    }

    #[test]
    fn payloads_roundtrip_through_norito() {
        let (credential, root, revocations) = signed_material();
        norito_roundtrip(&credential);
        norito_roundtrip(&root);
        norito_roundtrip(&revocations);
        norito_roundtrip(&issued_bundle());
        norito_roundtrip(&enrollment());
        norito_roundtrip(&renewal());
        norito_roundtrip(&proof());
    }

    #[test]
    fn signatures_verify_and_reject_forgery() {
        let (credential, root, revocations) = signed_material();
        verify_pop_credential_signature_v1(&credential).expect("credential verifies");
        verify_pop_commitment_root_signature_v1(&root).expect("root verifies");
        verify_pop_revocation_list_signature_v1(&revocations).expect("revocations verify");

        let mut forged = credential.clone();
        forged.credential_id = digest(0x99);
        let err = verify_pop_credential_signature_v1(&forged).expect_err("forgery rejected");
        assert!(matches!(
            err,
            PopCredentialValidationError::SignatureVerification { .. }
        ));
    }

    #[test]
    fn issued_credential_bundle_signs_and_validates_publications() {
        let bundle = issued_bundle();
        bundle.validate().expect("issued bundle validates");
        verify_pop_membership_transcript_policy_v1(
            &proof(),
            &bundle.credential,
            &bundle.commitment_root,
            &bundle.revocation_list,
            500,
            &[],
        )
        .expect("bundle material verifies membership proof");
    }

    #[test]
    fn issued_credential_bundle_rejects_inconsistent_revocation_version() {
        let mut revocations = revocations();
        revocations.list_version += 1;
        let err = issue_pop_credential_bundle_ed25519_v1(
            credential(),
            root(),
            revocations,
            &signing_key(0x55),
        )
        .expect_err("revocation version mismatch");
        assert_eq!(
            err,
            PopCredentialValidationError::CredentialRevocationListMismatch
        );
    }

    #[test]
    fn issued_credential_bundle_rejects_issuer_key_drift() {
        let mut bundle = issued_bundle();
        bundle.commitment_root =
            sign_pop_commitment_root_ed25519_v1(root(), &signing_key(0x66)).expect("resign root");
        let err = bundle.validate().expect_err("issuer key drift");
        assert_eq!(err, PopCredentialValidationError::IssuerKeyMismatch);
    }

    #[test]
    fn membership_proof_verifies_active_unrevoked_credential() {
        let (credential, root, revocations) = signed_material();
        verify_pop_membership_transcript_policy_v1(
            &proof(),
            &credential,
            &root,
            &revocations,
            500,
            &[],
        )
        .expect("membership proof verifies");
    }

    #[test]
    fn production_membership_verifier_rejects_policy_only_transcripts() {
        let (credential, root, revocations) = signed_material();
        let err =
            verify_pop_membership_proof_v1(&proof(), &credential, &root, &revocations, 500, &[])
                .expect_err("policy-only transcript is not production proof");
        assert_eq!(err, PopCredentialValidationError::PolicyOnlyProofSystem);
    }

    #[test]
    fn expired_credentials_are_rejected() {
        let (credential, root, revocations) = signed_material();
        let err = verify_pop_membership_transcript_policy_v1(
            &proof(),
            &credential,
            &root,
            &revocations,
            1_000,
            &[],
        )
        .expect_err("expired credential");
        assert!(matches!(
            err,
            PopCredentialValidationError::ExpiredCredential { .. }
        ));
    }

    #[test]
    fn revoked_nonces_are_rejected() {
        let (credential, root, mut revocations) = signed_material();
        revocations.entries.push(PopRevocationEntryV1 {
            nonce: credential.revocation_nonce,
            revoked_at_epoch: 200,
            reason: PopRevocationReasonV1::GovernanceSuspension,
        });
        revocations =
            sign_pop_revocation_list_ed25519_v1(revocations, &signing_key(0x55)).expect("resign");
        let err = verify_pop_membership_transcript_policy_v1(
            &proof(),
            &credential,
            &root,
            &revocations,
            500,
            &[],
        )
        .expect_err("revoked credential");
        assert_eq!(err, PopCredentialValidationError::RevokedCredential);
    }

    #[test]
    fn wrong_roots_are_rejected() {
        let (credential, mut root, revocations) = signed_material();
        root.root_digest = digest(0x66);
        root = sign_pop_commitment_root_ed25519_v1(root, &signing_key(0x55)).expect("resign");
        let err = verify_pop_membership_transcript_policy_v1(
            &proof(),
            &credential,
            &root,
            &revocations,
            500,
            &[],
        )
        .expect_err("wrong root");
        assert_eq!(err, PopCredentialValidationError::WrongCommitmentRoot);
    }

    #[test]
    fn stale_revocation_lists_are_rejected() {
        let (credential, root, mut revocations) = signed_material();
        revocations.list_version = 4;
        revocations =
            sign_pop_revocation_list_ed25519_v1(revocations, &signing_key(0x55)).expect("resign");
        let err = verify_pop_membership_transcript_policy_v1(
            &proof(),
            &credential,
            &root,
            &revocations,
            500,
            &[],
        )
        .expect_err("stale revocation list");
        assert!(matches!(
            err,
            PopCredentialValidationError::StaleRevocationList {
                proof_version: 3,
                current_version: 4
            }
        ));
    }

    #[test]
    fn future_revocation_list_versions_are_rejected() {
        let (credential, root, revocations) = signed_material();
        let mut proof = proof();
        proof.revocation_list_version = 4;
        proof.proof_digest = [0; 32];
        proof = finalize_pop_membership_proof_digest_v1(proof).expect("refinalize proof");
        let err = verify_pop_membership_transcript_policy_v1(
            &proof,
            &credential,
            &root,
            &revocations,
            500,
            &[],
        )
        .expect_err("future revocation list");
        assert!(matches!(
            err,
            PopCredentialValidationError::RevocationListVersionMismatch {
                proof_version: 4,
                current_version: 3
            }
        ));
    }

    #[test]
    fn issuer_mismatches_are_rejected() {
        let (credential, mut root, revocations) = signed_material();
        root.issuer_id = "other-issuer.sorafs".to_string();
        root = sign_pop_commitment_root_ed25519_v1(root, &signing_key(0x55)).expect("resign root");
        let err = verify_pop_membership_transcript_policy_v1(
            &proof(),
            &credential,
            &root,
            &revocations,
            500,
            &[],
        )
        .expect_err("issuer mismatch");
        assert_eq!(err, PopCredentialValidationError::IssuerMismatch);
    }

    #[test]
    fn replayed_nullifiers_are_rejected() {
        let (credential, root, revocations) = signed_material();
        let proof = proof();
        let err = verify_pop_membership_transcript_policy_v1(
            &proof,
            &credential,
            &root,
            &revocations,
            500,
            &[proof.nullifier],
        )
        .expect_err("replayed proof");
        assert_eq!(err, PopCredentialValidationError::ReplayedProof);
    }

    #[test]
    fn transcript_digest_detects_tampering() {
        let mut proof = proof();
        proof.verifier_context = "other-context".to_string();
        let err = proof.validate().expect_err("tampered transcript");
        assert_eq!(err, PopCredentialValidationError::ProofDigestMismatch);
    }

    #[test]
    fn revocation_entries_must_be_sorted_and_unique() {
        let mut list = revocations();
        list.entries = vec![
            PopRevocationEntryV1 {
                nonce: digest(0x20),
                revoked_at_epoch: 200,
                reason: PopRevocationReasonV1::Rotated,
            },
            PopRevocationEntryV1 {
                nonce: digest(0x10),
                revoked_at_epoch: 201,
                reason: PopRevocationReasonV1::Rotated,
            },
        ];
        let err = list.validate().expect_err("unsorted list");
        assert_eq!(err, PopCredentialValidationError::UnsortedRevocationList);

        list.entries[1].nonce = digest(0x20);
        let err = list.validate().expect_err("duplicate nonce");
        assert_eq!(err, PopCredentialValidationError::DuplicateRevocationNonce);
    }
}
