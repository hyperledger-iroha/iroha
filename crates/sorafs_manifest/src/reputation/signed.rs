//! Externally governed signatures and freshness policy for reputation snapshots.

use std::cmp::Ordering;

use blake3::Hasher;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use super::{
    MAX_REPUTATION_PROVIDERS, MAX_REPUTATION_TRUST_EDGES, ReputationProviderInputV1,
    ReputationSnapshotV1, ReputationTrustEdgeV1, ReputationValidationError,
    build_reputation_snapshot_with_trust_edges,
};

/// Schema version for [`ReputationSnapshotTrustPolicyV1`].
pub const REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1: u8 = 1;
/// Schema version for [`ReputationTrustedSignerV1`].
pub const REPUTATION_TRUSTED_SIGNER_VERSION_V1: u8 = 1;
/// Schema version for [`SignedReputationSnapshotV1`].
pub const SIGNED_REPUTATION_SNAPSHOT_VERSION_V1: u8 = 1;
/// Maximum number of keys in one governed reputation trust policy.
pub const MAX_REPUTATION_TRUSTED_SIGNERS: usize = 64;
/// Maximum signatures accepted on one reputation snapshot.
pub const MAX_REPUTATION_SNAPSHOT_SIGNATURES: usize = 64;
/// Maximum signer identifier length.
pub const MAX_REPUTATION_SIGNER_ID_LEN: usize = 128;
/// Maximum snapshot age a V1 policy may permit.
pub const MAX_REPUTATION_SNAPSHOT_AGE_SECS: u64 = 30 * 24 * 60 * 60;
/// Maximum future-clock allowance a V1 policy may permit.
pub const MAX_REPUTATION_FUTURE_SKEW_SECS: u64 = 5 * 60;
/// Schema version for [`ReputationScoringEvidenceV1`].
pub const REPUTATION_SCORING_EVIDENCE_VERSION_V1: u8 = 1;
/// Maximum canonical Norito size of an external reputation trust policy.
pub const MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES: usize = 64 * 1024;
/// Maximum canonical Norito size of one signed reputation snapshot and its evidence.
pub const MAX_SIGNED_REPUTATION_SNAPSHOT_ENCODED_BYTES: usize = 64 * 1024 * 1024;
/// Maximum cumulative allocation allowed while decoding a signed snapshot.
pub const MAX_SIGNED_REPUTATION_SNAPSHOT_DECODE_ALLOCATED_BYTES: usize = 256 * 1024 * 1024;

/// Canonical scoring inputs required to independently replay a snapshot.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ReputationScoringEvidenceV1 {
    /// Schema version.
    pub version: u8,
    /// Provider inputs in strictly increasing provider-id order.
    pub provider_inputs: Vec<ReputationProviderInputV1>,
    /// Trust edges in canonical source/destination order.
    pub trust_edges: Vec<ReputationTrustEdgeV1>,
}

impl ReputationScoringEvidenceV1 {
    /// Validate canonical ordering, uniqueness, and input bounds.
    pub fn validate(&self) -> Result<(), SignedReputationSnapshotError> {
        if self.version != REPUTATION_SCORING_EVIDENCE_VERSION_V1 {
            return Err(
                SignedReputationSnapshotError::UnsupportedScoringEvidenceVersion {
                    found: self.version,
                },
            );
        }
        if self.provider_inputs.is_empty() {
            return Err(SignedReputationSnapshotError::EmptyScoringEvidence);
        }
        super::validate_provider_count(self.provider_inputs.len())?;
        let mut previous: Option<&str> = None;
        for input in &self.provider_inputs {
            input.validate()?;
            if let Some(previous_id) = previous {
                match previous_id.cmp(input.provider_id.as_str()) {
                    Ordering::Equal => {
                        return Err(SignedReputationSnapshotError::DuplicateScoringProvider {
                            provider_id: input.provider_id.clone(),
                        });
                    }
                    Ordering::Greater => {
                        return Err(SignedReputationSnapshotError::ScoringProvidersNotSorted);
                    }
                    Ordering::Less => {}
                }
            }
            previous = Some(&input.provider_id);
        }
        super::validate_trust_edges(&self.trust_edges)?;
        Ok(())
    }

    /// Return the domain-separated canonical evidence digest.
    pub fn canonical_digest(&self) -> Result<[u8; 32], SignedReputationSnapshotError> {
        self.validate()?;
        hash_canonical(
            b"sorafs-reputation-scoring-evidence-v1",
            "reputation scoring evidence",
            self,
            MAX_SIGNED_REPUTATION_SNAPSHOT_ENCODED_BYTES,
        )
    }

    /// Return canonical bytes after exact preflight against the envelope cap.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SignedReputationSnapshotError> {
        self.validate()?;
        encode_canonical_bounded(
            "reputation scoring evidence",
            self,
            MAX_SIGNED_REPUTATION_SNAPSHOT_ENCODED_BYTES,
        )
    }

    /// Replay all fixed-point scoring and require an exact snapshot match.
    pub fn verify_snapshot(
        &self,
        snapshot: &ReputationSnapshotV1,
    ) -> Result<(), SignedReputationSnapshotError> {
        self.validate()?;
        snapshot.validate()?;
        let replay = build_reputation_snapshot_with_trust_edges(
            snapshot.snapshot_id,
            snapshot.generated_at_unix,
            snapshot.weights,
            &self.provider_inputs,
            &self.trust_edges,
            snapshot.previous_snapshot_id,
        )?;
        if replay != *snapshot {
            return Err(SignedReputationSnapshotError::ScoringReplayMismatch);
        }
        Ok(())
    }
}

/// One externally governed Ed25519 signer authorized by a trust policy.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ReputationTrustedSignerV1 {
    /// Schema version.
    pub version: u8,
    /// Stable governance signer identifier.
    pub signer_id: String,
    /// Strong canonical Ed25519 public key.
    pub public_key: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH],
}

impl ReputationTrustedSignerV1 {
    /// Validate the signer identifier and public key.
    pub fn validate(&self) -> Result<(), SignedReputationSnapshotError> {
        if self.version != REPUTATION_TRUSTED_SIGNER_VERSION_V1 {
            return Err(SignedReputationSnapshotError::UnsupportedSignerVersion {
                found: self.version,
            });
        }
        validate_signer_id(&self.signer_id)?;
        crate::checked_ed25519_verifying_key_from_bytes(&self.public_key).map_err(|reason| {
            SignedReputationSnapshotError::InvalidPublicKey {
                signer_id: self.signer_id.clone(),
                reason,
            }
        })?;
        Ok(())
    }
}

/// External trust and freshness policy used to admit reputation snapshots.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ReputationSnapshotTrustPolicyV1 {
    /// Schema version.
    pub version: u8,
    /// Non-zero governance-assigned policy identifier.
    pub policy_id: [u8; 32],
    /// First Unix second at which the policy is valid, inclusive.
    pub valid_from_unix: u64,
    /// Unix second at which the policy expires, exclusive.
    pub valid_until_unix: u64,
    /// Maximum accepted age of a snapshot at admission.
    pub max_snapshot_age_secs: u64,
    /// Maximum accepted clock skew into the future.
    pub max_future_skew_secs: u64,
    /// Minimum distinct trusted signatures required.
    pub min_signatures: u16,
    /// Trusted signers in strictly increasing signer-id order.
    pub signers: Vec<ReputationTrustedSignerV1>,
    /// Revoked signer identifiers in strictly increasing order.
    ///
    /// Retaining revoked identities in the policy makes revocation explicit and
    /// auditable while preventing those keys from contributing to quorum.
    pub revoked_signer_ids: Vec<String>,
}

impl ReputationSnapshotTrustPolicyV1 {
    /// Validate all policy bounds and canonical signer ordering.
    pub fn validate(&self) -> Result<(), SignedReputationSnapshotError> {
        if self.version != REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1 {
            return Err(SignedReputationSnapshotError::UnsupportedPolicyVersion {
                found: self.version,
            });
        }
        if crate::inert_bytes(&self.policy_id) {
            return Err(SignedReputationSnapshotError::InvalidPolicyId);
        }
        if self.valid_from_unix == 0 || self.valid_until_unix <= self.valid_from_unix {
            return Err(SignedReputationSnapshotError::InvalidPolicyValidityWindow);
        }
        if self.max_snapshot_age_secs == 0
            || self.max_snapshot_age_secs > MAX_REPUTATION_SNAPSHOT_AGE_SECS
        {
            return Err(SignedReputationSnapshotError::InvalidMaximumSnapshotAge {
                found: self.max_snapshot_age_secs,
                max: MAX_REPUTATION_SNAPSHOT_AGE_SECS,
            });
        }
        if self.max_future_skew_secs > MAX_REPUTATION_FUTURE_SKEW_SECS {
            return Err(SignedReputationSnapshotError::InvalidMaximumFutureSkew {
                found: self.max_future_skew_secs,
                max: MAX_REPUTATION_FUTURE_SKEW_SECS,
            });
        }
        if self.signers.is_empty() {
            return Err(SignedReputationSnapshotError::EmptyTrustedSignerSet);
        }
        if self.signers.len() > MAX_REPUTATION_TRUSTED_SIGNERS {
            return Err(SignedReputationSnapshotError::TooManyTrustedSigners {
                count: self.signers.len(),
                max: MAX_REPUTATION_TRUSTED_SIGNERS,
            });
        }
        let mut previous_id: Option<&str> = None;
        let mut previous_keys: Vec<[u8; ed25519_dalek::PUBLIC_KEY_LENGTH]> = Vec::new();
        previous_keys
            .try_reserve_exact(self.signers.len())
            .map_err(|_| SignedReputationSnapshotError::AllocationFailed {
                context: "reputation trust policy public keys",
            })?;
        for signer in &self.signers {
            signer.validate()?;
            if let Some(previous) = previous_id {
                match previous.cmp(signer.signer_id.as_str()) {
                    Ordering::Equal => {
                        return Err(SignedReputationSnapshotError::DuplicateTrustedSigner {
                            signer_id: signer.signer_id.clone(),
                        });
                    }
                    Ordering::Greater => {
                        return Err(SignedReputationSnapshotError::TrustedSignersNotSorted);
                    }
                    Ordering::Less => {}
                }
            }
            if previous_keys.contains(&signer.public_key) {
                return Err(SignedReputationSnapshotError::DuplicateTrustedPublicKey);
            }
            previous_keys.push(signer.public_key);
            previous_id = Some(&signer.signer_id);
        }

        if self.revoked_signer_ids.len() > self.signers.len() {
            return Err(SignedReputationSnapshotError::TooManyRevocations {
                count: self.revoked_signer_ids.len(),
                signer_count: self.signers.len(),
            });
        }
        let mut previous_revocation: Option<&str> = None;
        for signer_id in &self.revoked_signer_ids {
            validate_signer_id(signer_id)?;
            if let Some(previous) = previous_revocation {
                match previous.cmp(signer_id.as_str()) {
                    Ordering::Equal => {
                        return Err(SignedReputationSnapshotError::DuplicateRevocation {
                            signer_id: signer_id.clone(),
                        });
                    }
                    Ordering::Greater => {
                        return Err(SignedReputationSnapshotError::RevocationsNotSorted);
                    }
                    Ordering::Less => {}
                }
            }
            if self.signer(signer_id).is_none() {
                return Err(SignedReputationSnapshotError::UnknownRevokedSigner {
                    signer_id: signer_id.clone(),
                });
            }
            previous_revocation = Some(signer_id);
        }
        let active_signer_count = self
            .signers
            .len()
            .checked_sub(self.revoked_signer_ids.len())
            .ok_or(SignedReputationSnapshotError::InvalidSignatureThreshold {
                threshold: self.min_signatures,
                signer_count: 0,
            })?;
        let threshold = usize::from(self.min_signatures);
        if threshold == 0 || threshold > active_signer_count {
            return Err(SignedReputationSnapshotError::InvalidSignatureThreshold {
                threshold: self.min_signatures,
                signer_count: active_signer_count,
            });
        }
        Ok(())
    }

    /// Return the domain-separated canonical digest of this policy.
    pub fn canonical_digest(&self) -> Result<[u8; 32], SignedReputationSnapshotError> {
        self.validate()?;
        hash_canonical(
            b"sorafs-reputation-trust-policy-v1",
            "reputation trust policy",
            self,
            MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES,
        )
    }

    /// Return canonical policy bytes after exact size preflight.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SignedReputationSnapshotError> {
        self.validate()?;
        encode_canonical_bounded(
            "reputation trust policy",
            self,
            MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES,
        )
    }

    fn signer(&self, signer_id: &str) -> Option<&ReputationTrustedSignerV1> {
        self.signers
            .binary_search_by(|signer| signer.signer_id.as_str().cmp(signer_id))
            .ok()
            .map(|index| &self.signers[index])
    }

    fn is_revoked(&self, signer_id: &str) -> bool {
        self.revoked_signer_ids
            .binary_search_by(|revoked| revoked.as_str().cmp(signer_id))
            .is_ok()
    }
}

/// One signature over a reputation snapshot and external policy digest.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ReputationSnapshotSignatureV1 {
    /// Stable signer identifier from the external trust policy.
    pub signer_id: String,
    /// Canonical fixed-width Ed25519 signature.
    pub signature: [u8; ed25519_dalek::SIGNATURE_LENGTH],
}

/// Reputation snapshot plus externally authorized threshold signatures.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SignedReputationSnapshotV1 {
    /// Schema version.
    pub version: u8,
    /// Digest of the external policy used for verification.
    pub policy_digest: [u8; 32],
    /// Canonical reputation snapshot.
    pub snapshot: ReputationSnapshotV1,
    /// Digest of the embedded deterministic scoring evidence.
    pub scoring_evidence_digest: [u8; 32],
    /// Inputs and trust edges needed to replay the snapshot exactly.
    pub scoring_evidence: ReputationScoringEvidenceV1,
    /// Distinct signatures in strictly increasing signer-id order.
    pub signatures: Vec<ReputationSnapshotSignatureV1>,
}

impl SignedReputationSnapshotV1 {
    /// Validate all policy-independent structure and embedded scoring evidence.
    pub fn validate_structure(&self) -> Result<(), SignedReputationSnapshotError> {
        preflight_canonical_encoded_len(
            "signed reputation snapshot",
            self,
            MAX_SIGNED_REPUTATION_SNAPSHOT_ENCODED_BYTES,
        )?;
        if self.version != SIGNED_REPUTATION_SNAPSHOT_VERSION_V1 {
            return Err(
                SignedReputationSnapshotError::UnsupportedSignedSnapshotVersion {
                    found: self.version,
                },
            );
        }
        if crate::inert_bytes(&self.policy_digest) {
            return Err(SignedReputationSnapshotError::InvalidPolicyDigest);
        }
        self.snapshot.validate()?;
        if crate::inert_bytes(&self.scoring_evidence_digest) {
            return Err(SignedReputationSnapshotError::InvalidScoringEvidenceDigest);
        }
        let evidence_digest = self.scoring_evidence.canonical_digest()?;
        if self.scoring_evidence_digest != evidence_digest {
            return Err(SignedReputationSnapshotError::ScoringEvidenceDigestMismatch);
        }
        self.scoring_evidence.verify_snapshot(&self.snapshot)?;

        if self.signatures.is_empty() {
            return Err(SignedReputationSnapshotError::EmptySnapshotSignatures);
        }
        if self.signatures.len() > MAX_REPUTATION_SNAPSHOT_SIGNATURES {
            return Err(SignedReputationSnapshotError::TooManySnapshotSignatures {
                count: self.signatures.len(),
                max: MAX_REPUTATION_SNAPSHOT_SIGNATURES,
            });
        }
        let mut previous_id: Option<&str> = None;
        for signature in &self.signatures {
            validate_signer_id(&signature.signer_id)?;
            if let Some(previous) = previous_id {
                match previous.cmp(signature.signer_id.as_str()) {
                    Ordering::Equal => {
                        return Err(SignedReputationSnapshotError::DuplicateSnapshotSigner {
                            signer_id: signature.signer_id.clone(),
                        });
                    }
                    Ordering::Greater => {
                        return Err(SignedReputationSnapshotError::SnapshotSignaturesNotSorted);
                    }
                    Ordering::Less => {}
                }
            }
            crate::checked_ed25519_signature_from_bytes(&signature.signature).map_err(
                |reason| SignedReputationSnapshotError::InvalidSignature {
                    signer_id: signature.signer_id.clone(),
                    reason,
                },
            )?;
            previous_id = Some(&signature.signer_id);
        }
        Ok(())
    }

    /// Return the exact digest trusted signers must sign.
    pub fn signing_digest(&self) -> Result<[u8; 32], SignedReputationSnapshotError> {
        snapshot_signing_digest(
            &self.snapshot,
            self.policy_digest,
            self.scoring_evidence_digest,
        )
    }

    /// Return canonical envelope bytes after structural validation and exact size preflight.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SignedReputationSnapshotError> {
        self.validate_structure()?;
        encode_canonical_bounded(
            "signed reputation snapshot",
            self,
            MAX_SIGNED_REPUTATION_SNAPSHOT_ENCODED_BYTES,
        )
    }

    /// Verify structure, external policy, time bounds, and every signature.
    pub fn verify(
        &self,
        policy: &ReputationSnapshotTrustPolicyV1,
        admitted_at_unix: u64,
    ) -> Result<(), SignedReputationSnapshotError> {
        self.validate_structure()?;
        policy.validate()?;
        let expected_policy_digest = policy.canonical_digest()?;
        if self.policy_digest != expected_policy_digest {
            return Err(SignedReputationSnapshotError::PolicyDigestMismatch);
        }
        validate_admission_time(policy, &self.snapshot, admitted_at_unix)?;

        if self.signatures.len() > policy.signers.len() {
            return Err(SignedReputationSnapshotError::TooManySnapshotSignatures {
                count: self.signatures.len(),
                max: policy.signers.len(),
            });
        }
        if self.signatures.len() < usize::from(policy.min_signatures) {
            return Err(SignedReputationSnapshotError::SignatureQuorumNotMet {
                found: self.signatures.len(),
                required: policy.min_signatures,
            });
        }

        let digest = self.signing_digest()?;
        for signature in &self.signatures {
            let trusted = policy.signer(&signature.signer_id).ok_or_else(|| {
                SignedReputationSnapshotError::UntrustedSnapshotSigner {
                    signer_id: signature.signer_id.clone(),
                }
            })?;
            if policy.is_revoked(&signature.signer_id) {
                return Err(SignedReputationSnapshotError::RevokedSnapshotSigner {
                    signer_id: signature.signer_id.clone(),
                });
            }
            let signature_value = crate::checked_ed25519_signature_from_bytes(&signature.signature)
                .map_err(|reason| SignedReputationSnapshotError::InvalidSignature {
                    signer_id: signature.signer_id.clone(),
                    reason,
                })?;
            let verifying_key = crate::checked_ed25519_verifying_key_from_bytes(
                &trusted.public_key,
            )
            .map_err(|reason| SignedReputationSnapshotError::InvalidPublicKey {
                signer_id: signature.signer_id.clone(),
                reason,
            })?;
            verifying_key
                .verify_strict(&digest, &signature_value)
                .map_err(
                    |error| SignedReputationSnapshotError::SignatureVerification {
                        signer_id: signature.signer_id.clone(),
                        reason: error.to_string(),
                    },
                )?;
        }
        Ok(())
    }
}

/// Decode and validate one canonical external reputation trust policy.
pub fn decode_reputation_trust_policy(
    bytes: &[u8],
) -> Result<ReputationSnapshotTrustPolicyV1, SignedReputationSnapshotError> {
    let policy: ReputationSnapshotTrustPolicyV1 = decode_canonical(
        "reputation trust policy",
        bytes,
        MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES,
        norito::DecodeLimits::new(
            MAX_REPUTATION_SIGNER_ID_LEN * 2,
            MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES,
            4_096,
            MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES * 4,
            32,
        ),
    )?;
    policy.validate()?;
    Ok(policy)
}

/// Decode one canonical signed snapshot and validate policy-independent structure.
pub fn decode_signed_reputation_snapshot(
    bytes: &[u8],
) -> Result<SignedReputationSnapshotV1, SignedReputationSnapshotError> {
    let envelope: SignedReputationSnapshotV1 = decode_canonical(
        "signed reputation snapshot",
        bytes,
        MAX_SIGNED_REPUTATION_SNAPSHOT_ENCODED_BYTES,
        norito::DecodeLimits::new(
            MAX_REPUTATION_TRUST_EDGES.max(MAX_REPUTATION_PROVIDERS),
            MAX_SIGNED_REPUTATION_SNAPSHOT_ENCODED_BYTES,
            4_000_000,
            MAX_SIGNED_REPUTATION_SNAPSHOT_DECODE_ALLOCATED_BYTES,
            64,
        ),
    )?;
    envelope.validate_structure()?;
    Ok(envelope)
}

/// Decode a canonical signed snapshot and perform complete external-policy admission.
pub fn decode_and_verify_signed_reputation_snapshot(
    bytes: &[u8],
    policy: &ReputationSnapshotTrustPolicyV1,
    admitted_at_unix: u64,
) -> Result<SignedReputationSnapshotV1, SignedReputationSnapshotError> {
    let envelope = decode_signed_reputation_snapshot(bytes)?;
    envelope.verify(policy, admitted_at_unix)?;
    Ok(envelope)
}

fn decode_canonical<T>(
    payload: &'static str,
    bytes: &[u8],
    max_bytes: usize,
    limits: norito::DecodeLimits,
) -> Result<T, SignedReputationSnapshotError>
where
    T: for<'decode> norito::NoritoDeserialize<'decode> + norito::NoritoSerialize,
{
    if bytes.len() > max_bytes {
        return Err(SignedReputationSnapshotError::EncodedPayloadTooLarge {
            payload,
            len: bytes.len(),
            max: max_bytes,
        });
    }
    let decoded = norito::decode_from_bytes_with_limits(bytes, limits).map_err(|error| {
        SignedReputationSnapshotError::Decoding {
            payload,
            reason: error.to_string(),
        }
    })?;
    let canonical = encode_canonical_bounded(payload, &decoded, max_bytes)?;
    if canonical != bytes {
        return Err(SignedReputationSnapshotError::NonCanonicalEncoding { payload });
    }
    Ok(decoded)
}

/// Compute the domain-separated snapshot digest bound to an external policy.
pub fn snapshot_signing_digest(
    snapshot: &ReputationSnapshotV1,
    policy_digest: [u8; 32],
    scoring_evidence_digest: [u8; 32],
) -> Result<[u8; 32], SignedReputationSnapshotError> {
    snapshot.validate()?;
    if crate::inert_bytes(&policy_digest) {
        return Err(SignedReputationSnapshotError::InvalidPolicyDigest);
    }
    if crate::inert_bytes(&scoring_evidence_digest) {
        return Err(SignedReputationSnapshotError::InvalidScoringEvidenceDigest);
    }
    let snapshot_bytes = encode_canonical_bounded(
        "reputation snapshot",
        snapshot,
        MAX_SIGNED_REPUTATION_SNAPSHOT_ENCODED_BYTES,
    )?;
    let encoded_len = u64::try_from(snapshot_bytes.len())
        .map_err(|_| SignedReputationSnapshotError::LengthOverflow)?;
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs-reputation-snapshot-signature-v1");
    hasher.update(&policy_digest);
    hasher.update(&scoring_evidence_digest);
    hasher.update(&encoded_len.to_le_bytes());
    hasher.update(&snapshot_bytes);
    Ok(*hasher.finalize().as_bytes())
}

/// Validate exact predecessor linkage and strictly increasing generation time.
pub fn validate_reputation_snapshot_transition(
    previous: Option<&ReputationSnapshotV1>,
    next: &ReputationSnapshotV1,
) -> Result<(), SignedReputationSnapshotError> {
    next.validate()?;
    match previous {
        Some(previous) => {
            previous.validate()?;
            if next.previous_snapshot_id != Some(previous.snapshot_id) {
                return Err(SignedReputationSnapshotError::SnapshotPredecessorMismatch);
            }
            if next.generated_at_unix <= previous.generated_at_unix {
                return Err(SignedReputationSnapshotError::SnapshotTimeDidNotAdvance);
            }
        }
        None if next.previous_snapshot_id.is_some() => {
            return Err(SignedReputationSnapshotError::UnexpectedInitialPredecessor);
        }
        None => {}
    }
    Ok(())
}

fn validate_admission_time(
    policy: &ReputationSnapshotTrustPolicyV1,
    snapshot: &ReputationSnapshotV1,
    admitted_at_unix: u64,
) -> Result<(), SignedReputationSnapshotError> {
    if admitted_at_unix == 0 {
        return Err(SignedReputationSnapshotError::InvalidAdmissionTime);
    }
    if !(policy.valid_from_unix..policy.valid_until_unix).contains(&admitted_at_unix) {
        return Err(SignedReputationSnapshotError::PolicyInactiveAtAdmission);
    }
    if !(policy.valid_from_unix..policy.valid_until_unix).contains(&snapshot.generated_at_unix) {
        return Err(SignedReputationSnapshotError::PolicyInactiveAtGeneration);
    }
    let latest_allowed = admitted_at_unix
        .checked_add(policy.max_future_skew_secs)
        .ok_or(SignedReputationSnapshotError::TimeOverflow)?;
    if snapshot.generated_at_unix > latest_allowed {
        return Err(SignedReputationSnapshotError::SnapshotFromFuture);
    }
    let expires_at = snapshot
        .generated_at_unix
        .checked_add(policy.max_snapshot_age_secs)
        .ok_or(SignedReputationSnapshotError::TimeOverflow)?;
    if admitted_at_unix > expires_at {
        return Err(SignedReputationSnapshotError::SnapshotTooOld);
    }
    Ok(())
}

fn hash_canonical<T: norito::NoritoSerialize>(
    domain: &[u8],
    payload: &'static str,
    value: &T,
    max_bytes: usize,
) -> Result<[u8; 32], SignedReputationSnapshotError> {
    let bytes = encode_canonical_bounded(payload, value, max_bytes)?;
    let encoded_len =
        u64::try_from(bytes.len()).map_err(|_| SignedReputationSnapshotError::LengthOverflow)?;
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(&encoded_len.to_le_bytes());
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}

fn encode_canonical_bounded<T: norito::NoritoSerialize>(
    payload: &'static str,
    value: &T,
    max_bytes: usize,
) -> Result<Vec<u8>, SignedReputationSnapshotError> {
    preflight_canonical_encoded_len(payload, value, max_bytes)?;
    let bytes = norito::to_bytes(value)
        .map_err(|error| SignedReputationSnapshotError::Encoding(error.to_string()))?;
    if bytes.len() > max_bytes {
        return Err(SignedReputationSnapshotError::EncodedPayloadTooLarge {
            payload,
            len: bytes.len(),
            max: max_bytes,
        });
    }
    Ok(bytes)
}

fn preflight_canonical_encoded_len<T: norito::NoritoSerialize>(
    payload: &'static str,
    value: &T,
    max_bytes: usize,
) -> Result<usize, SignedReputationSnapshotError> {
    if let Some(len) = value.encoded_len_exact()
        && len > max_bytes
    {
        return Err(SignedReputationSnapshotError::EncodedPayloadTooLarge {
            payload,
            len,
            max: max_bytes,
        });
    }
    let exact_payload_len = norito::core::encoded_payload_len(value)
        .map_err(|_| SignedReputationSnapshotError::EncodedLengthUnavailable { payload })?;
    if exact_payload_len > max_bytes {
        return Err(SignedReputationSnapshotError::EncodedPayloadTooLarge {
            payload,
            len: exact_payload_len,
            max: max_bytes,
        });
    }
    Ok(exact_payload_len)
}

fn validate_signer_id(signer_id: &str) -> Result<(), SignedReputationSnapshotError> {
    if signer_id.is_empty()
        || signer_id.len() > MAX_REPUTATION_SIGNER_ID_LEN
        || !signer_id.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b'.' | b':')
        })
    {
        return Err(SignedReputationSnapshotError::InvalidSignerId);
    }
    Ok(())
}

/// Signed reputation snapshot policy and verification failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignedReputationSnapshotError {
    /// Scoring-evidence schema version is unsupported.
    #[error("unsupported reputation scoring-evidence version {found}")]
    UnsupportedScoringEvidenceVersion {
        /// Observed version.
        found: u8,
    },
    /// Trusted signer schema version is unsupported.
    #[error("unsupported reputation trusted-signer version {found}")]
    UnsupportedSignerVersion {
        /// Observed version.
        found: u8,
    },
    /// Trust-policy schema version is unsupported.
    #[error("unsupported reputation trust-policy version {found}")]
    UnsupportedPolicyVersion {
        /// Observed version.
        found: u8,
    },
    /// Signed-snapshot schema version is unsupported.
    #[error("unsupported signed reputation snapshot version {found}")]
    UnsupportedSignedSnapshotVersion {
        /// Observed version.
        found: u8,
    },
    /// Signer identifier is malformed.
    #[error("reputation signer identifier is malformed")]
    InvalidSignerId,
    /// Policy identifier is inert.
    #[error("reputation policy id must not be all zero")]
    InvalidPolicyId,
    /// Policy digest is inert.
    #[error("reputation policy digest must not be all zero")]
    InvalidPolicyDigest,
    /// Scoring evidence digest is inert.
    #[error("reputation scoring-evidence digest must not be all zero")]
    InvalidScoringEvidenceDigest,
    /// Scoring evidence contains no provider inputs.
    #[error("reputation scoring evidence must contain provider inputs")]
    EmptyScoringEvidence,
    /// Scoring evidence repeats a provider input.
    #[error("duplicate reputation scoring provider `{provider_id}`")]
    DuplicateScoringProvider {
        /// Duplicate provider identifier.
        provider_id: String,
    },
    /// Scoring inputs are not canonically ordered.
    #[error("reputation scoring providers must be sorted by provider id")]
    ScoringProvidersNotSorted,
    /// Embedded scoring evidence digest does not match its bytes.
    #[error("reputation scoring-evidence digest mismatch")]
    ScoringEvidenceDigestMismatch,
    /// Replaying the embedded evidence did not reproduce the snapshot.
    #[error("reputation scoring evidence does not reproduce the snapshot")]
    ScoringReplayMismatch,
    /// Policy validity interval is empty or malformed.
    #[error("reputation policy validity window is invalid")]
    InvalidPolicyValidityWindow,
    /// Maximum snapshot age is invalid.
    #[error("reputation policy maximum snapshot age {found} exceeds 1..={max}")]
    InvalidMaximumSnapshotAge {
        /// Observed age.
        found: u64,
        /// Maximum age.
        max: u64,
    },
    /// Maximum future skew is invalid.
    #[error("reputation policy maximum future skew {found} exceeds {max}")]
    InvalidMaximumFutureSkew {
        /// Observed skew.
        found: u64,
        /// Maximum skew.
        max: u64,
    },
    /// Policy has no trusted signers.
    #[error("reputation policy must contain trusted signers")]
    EmptyTrustedSignerSet,
    /// Policy has too many trusted signers.
    #[error("reputation policy signer count {count} exceeds maximum {max}")]
    TooManyTrustedSigners {
        /// Observed count.
        count: usize,
        /// Maximum count.
        max: usize,
    },
    /// Policy signature threshold is invalid.
    #[error("reputation policy threshold {threshold} is invalid for {signer_count} signers")]
    InvalidSignatureThreshold {
        /// Observed threshold.
        threshold: u16,
        /// Signer count.
        signer_count: usize,
    },
    /// Trusted signer identifiers are duplicated.
    #[error("duplicate reputation trusted signer `{signer_id}`")]
    DuplicateTrustedSigner {
        /// Duplicate signer identifier.
        signer_id: String,
    },
    /// Trusted signers are not canonically sorted.
    #[error("reputation trusted signers must be sorted by signer id")]
    TrustedSignersNotSorted,
    /// Two signer identifiers reuse one public key.
    #[error("reputation trust policy must not reuse a public key")]
    DuplicateTrustedPublicKey,
    /// Revocation count exceeds the policy signer inventory.
    #[error("reputation policy revocation count {count} exceeds {signer_count} signers")]
    TooManyRevocations {
        /// Observed revocation count.
        count: usize,
        /// Policy signer count.
        signer_count: usize,
    },
    /// Revocation list repeats a signer identifier.
    #[error("duplicate reputation signer revocation `{signer_id}`")]
    DuplicateRevocation {
        /// Duplicate signer identifier.
        signer_id: String,
    },
    /// Revocation identifiers are not canonically sorted.
    #[error("reputation signer revocations must be sorted by signer id")]
    RevocationsNotSorted,
    /// Revocation list names a signer absent from the policy.
    #[error("reputation policy revokes unknown signer `{signer_id}`")]
    UnknownRevokedSigner {
        /// Unknown signer identifier.
        signer_id: String,
    },
    /// A trusted public key is malformed or weak.
    #[error("invalid reputation public key for `{signer_id}`: {reason}")]
    InvalidPublicKey {
        /// Signer identifier.
        signer_id: String,
        /// Validation reason.
        reason: String,
    },
    /// Envelope policy digest does not match the external policy.
    #[error("signed reputation snapshot policy digest mismatch")]
    PolicyDigestMismatch,
    /// Admission timestamp is zero.
    #[error("signed reputation snapshot admission time must not be zero")]
    InvalidAdmissionTime,
    /// Policy was inactive at admission.
    #[error("reputation trust policy is inactive at admission")]
    PolicyInactiveAtAdmission,
    /// Policy was inactive when the snapshot was generated.
    #[error("reputation trust policy is inactive at snapshot generation")]
    PolicyInactiveAtGeneration,
    /// Snapshot exceeds the allowed clock skew.
    #[error("reputation snapshot is too far in the future")]
    SnapshotFromFuture,
    /// Snapshot exceeds the maximum age.
    #[error("reputation snapshot is too old")]
    SnapshotTooOld,
    /// Time arithmetic overflowed.
    #[error("reputation snapshot time arithmetic overflow")]
    TimeOverflow,
    /// Signature count exceeds a policy or schema bound.
    #[error("signed reputation snapshot signature count {count} exceeds maximum {max}")]
    TooManySnapshotSignatures {
        /// Observed count.
        count: usize,
        /// Maximum count.
        max: usize,
    },
    /// Signed envelope contains no authorization signatures.
    #[error("signed reputation snapshot must contain at least one signature")]
    EmptySnapshotSignatures,
    /// Signature count is below the external threshold.
    #[error("signed reputation snapshot has {found} signatures; {required} required")]
    SignatureQuorumNotMet {
        /// Observed count.
        found: usize,
        /// Required count.
        required: u16,
    },
    /// Snapshot signatures repeat a signer.
    #[error("duplicate reputation snapshot signer `{signer_id}`")]
    DuplicateSnapshotSigner {
        /// Duplicate signer identifier.
        signer_id: String,
    },
    /// Snapshot signatures are not canonically ordered.
    #[error("reputation snapshot signatures must be sorted by signer id")]
    SnapshotSignaturesNotSorted,
    /// Snapshot signature uses a signer outside the external policy.
    #[error("untrusted reputation snapshot signer `{signer_id}`")]
    UntrustedSnapshotSigner {
        /// Signer identifier.
        signer_id: String,
    },
    /// Snapshot signature comes from a policy-revoked signer.
    #[error("revoked reputation snapshot signer `{signer_id}`")]
    RevokedSnapshotSigner {
        /// Revoked signer identifier.
        signer_id: String,
    },
    /// Signature encoding is invalid.
    #[error("invalid reputation signature for `{signer_id}`: {reason}")]
    InvalidSignature {
        /// Signer identifier.
        signer_id: String,
        /// Validation reason.
        reason: String,
    },
    /// Cryptographic verification failed.
    #[error("reputation signature verification failed for `{signer_id}`: {reason}")]
    SignatureVerification {
        /// Signer identifier.
        signer_id: String,
        /// Verification reason.
        reason: String,
    },
    /// Next snapshot does not extend the retained head.
    #[error("reputation snapshot predecessor does not match the retained head")]
    SnapshotPredecessorMismatch,
    /// Snapshot time is not strictly monotonic.
    #[error("reputation snapshot generation time did not advance")]
    SnapshotTimeDidNotAdvance,
    /// First snapshot unexpectedly names a predecessor.
    #[error("initial reputation snapshot must not name a predecessor")]
    UnexpectedInitialPredecessor,
    /// Canonical encoded length cannot be represented.
    #[error("canonical reputation payload length overflow")]
    LengthOverflow,
    /// Encoded policy or snapshot exceeds its pre-decode byte cap.
    #[error("{payload} encoded length {len} exceeds maximum {max}")]
    EncodedPayloadTooLarge {
        /// Payload kind.
        payload: &'static str,
        /// Observed byte length.
        len: usize,
        /// Maximum accepted byte length.
        max: usize,
    },
    /// Canonical encoder cannot preflight an exact payload length.
    #[error("{payload} does not expose an exact canonical encoded length")]
    EncodedLengthUnavailable {
        /// Payload kind.
        payload: &'static str,
    },
    /// Bounded canonical Norito decoding failed.
    #[error("failed to decode {payload}: {reason}")]
    Decoding {
        /// Payload kind.
        payload: &'static str,
        /// Decoder diagnostic.
        reason: String,
    },
    /// Decoded bytes used a valid but noncanonical Norito representation.
    #[error("{payload} is not encoded with canonical Norito bytes")]
    NonCanonicalEncoding {
        /// Payload kind.
        payload: &'static str,
    },
    /// A bounded allocation failed.
    #[error("reputation signed-snapshot allocation failed for {context}")]
    AllocationFailed {
        /// Allocation context.
        context: &'static str,
    },
    /// Canonical serialization failed.
    #[error("reputation signed-snapshot encoding failed: {0}")]
    Encoding(String),
    /// The embedded snapshot is invalid.
    #[error(transparent)]
    Snapshot(#[from] ReputationValidationError),
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::reputation::{
        MAX_REPUTATION_DEGRADATION_FLAGS, REPUTATION_PROVIDER_INPUT_VERSION_V1,
        REPUTATION_PROVIDER_METRICS_VERSION_V1, ReputationDegradationFlagV1,
        ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
        ReputationWeightsV1, build_reputation_snapshot,
    };

    const GENERATED_AT: u64 = 1_800_000_000;

    fn input(provider_id: &str) -> ReputationProviderInputV1 {
        ReputationProviderInputV1 {
            version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
            provider_id: provider_id.to_string(),
            metrics: ReputationProviderMetricsV1 {
                version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
                por_success_bps: 9_800,
                pdp_success_bps: 9_700,
                potr_success_bps: 9_600,
                latency_health_bps: 9_500,
                dispute_rate_bps: 0,
                token_violation_rate_bps: 0,
                repair_breach_rate_bps: 0,
            },
            reserve_stage: ReputationReserveStageV1::Active,
            previous_score_bps: None,
            active_dispute: false,
            slashing_event: false,
        }
    }

    fn signing_keys() -> [SigningKey; 3] {
        [
            SigningKey::from_bytes(&[1; 32]),
            SigningKey::from_bytes(&[2; 32]),
            SigningKey::from_bytes(&[3; 32]),
        ]
    }

    fn policy() -> ReputationSnapshotTrustPolicyV1 {
        let keys = signing_keys();
        ReputationSnapshotTrustPolicyV1 {
            version: REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1,
            policy_id: [0xA5; 32],
            valid_from_unix: GENERATED_AT - 1_000,
            valid_until_unix: GENERATED_AT + 10_000,
            max_snapshot_age_secs: 600,
            max_future_skew_secs: 30,
            min_signatures: 2,
            signers: keys
                .iter()
                .enumerate()
                .map(|(index, key)| ReputationTrustedSignerV1 {
                    version: REPUTATION_TRUSTED_SIGNER_VERSION_V1,
                    signer_id: format!("council-{}", index + 1),
                    public_key: key.verifying_key().to_bytes(),
                })
                .collect(),
            revoked_signer_ids: Vec::new(),
        }
    }

    fn snapshot(
        snapshot_id: [u8; 16],
        generated_at_unix: u64,
        previous_snapshot_id: Option<[u8; 16]>,
    ) -> ReputationSnapshotV1 {
        let inputs = provider_inputs();
        build_reputation_snapshot(
            snapshot_id,
            generated_at_unix,
            ReputationWeightsV1::default(),
            &inputs,
            previous_snapshot_id,
        )
        .expect("valid snapshot")
    }

    fn provider_inputs() -> Vec<ReputationProviderInputV1> {
        vec![input("provider-a"), input("provider-b")]
    }

    fn scoring_evidence() -> ReputationScoringEvidenceV1 {
        ReputationScoringEvidenceV1 {
            version: REPUTATION_SCORING_EVIDENCE_VERSION_V1,
            provider_inputs: provider_inputs(),
            trust_edges: Vec::new(),
        }
    }

    fn signed_snapshot() -> (ReputationSnapshotTrustPolicyV1, SignedReputationSnapshotV1) {
        let policy = policy();
        let scoring_evidence = scoring_evidence();
        let mut envelope = SignedReputationSnapshotV1 {
            version: SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
            policy_digest: policy.canonical_digest().expect("policy digest"),
            snapshot: snapshot([0x11; 16], GENERATED_AT, None),
            scoring_evidence_digest: scoring_evidence
                .canonical_digest()
                .expect("scoring-evidence digest"),
            scoring_evidence,
            signatures: Vec::new(),
        };
        let digest = envelope.signing_digest().expect("signing digest");
        let keys = signing_keys();
        envelope.signatures = [0_usize, 1]
            .into_iter()
            .map(|index| ReputationSnapshotSignatureV1 {
                signer_id: format!("council-{}", index + 1),
                signature: keys[index].sign(&digest).to_bytes(),
            })
            .collect();
        (policy, envelope)
    }

    #[test]
    fn threshold_snapshot_verifies_against_external_policy() {
        let (policy, envelope) = signed_snapshot();
        envelope
            .validate_structure()
            .expect("intrinsically valid envelope");
        envelope
            .verify(&policy, GENERATED_AT + 10)
            .expect("threshold signatures must verify");
    }

    #[test]
    fn intrinsic_validation_rejects_inert_policy_empty_and_malformed_signatures() {
        let (_, envelope) = signed_snapshot();

        let mut inert_policy = envelope.clone();
        inert_policy.policy_digest = [0; 32];
        assert_eq!(
            inert_policy.validate_structure(),
            Err(SignedReputationSnapshotError::InvalidPolicyDigest)
        );

        let mut empty = envelope.clone();
        empty.signatures.clear();
        assert_eq!(
            empty.validate_structure(),
            Err(SignedReputationSnapshotError::EmptySnapshotSignatures)
        );

        let mut malformed = envelope;
        malformed.signatures[0].signature = [0; ed25519_dalek::SIGNATURE_LENGTH];
        assert!(matches!(
            malformed.validate_structure(),
            Err(SignedReputationSnapshotError::InvalidSignature { .. })
        ));
    }

    #[test]
    fn bounded_canonical_decoders_accept_verified_policy_and_envelope() {
        let (policy, envelope) = signed_snapshot();
        let policy_bytes = policy.canonical_bytes().expect("encode policy");
        let envelope_bytes = envelope.canonical_bytes().expect("encode signed snapshot");

        assert_eq!(
            decode_reputation_trust_policy(&policy_bytes).expect("decode canonical policy"),
            policy
        );
        assert_eq!(
            decode_signed_reputation_snapshot(&envelope_bytes)
                .expect("decode canonical signed snapshot"),
            envelope
        );
        assert_eq!(
            decode_and_verify_signed_reputation_snapshot(
                &envelope_bytes,
                &policy,
                GENERATED_AT + 10,
            )
            .expect("decode and verify signed snapshot"),
            envelope
        );
    }

    #[test]
    fn signed_structure_and_decoder_reject_too_many_degradation_flags() {
        let (_, mut envelope) = signed_snapshot();
        let provider_id = envelope.snapshot.providers[0].provider_id.clone();
        envelope.snapshot.providers[0].degradation_flags = vec![
            ReputationDegradationFlagV1::ReserveWarning,
            ReputationDegradationFlagV1::ReserveGrace,
            ReputationDegradationFlagV1::ReserveDelinquent,
            ReputationDegradationFlagV1::ReserveDefault,
            ReputationDegradationFlagV1::ProofSuccessBelow90,
            ReputationDegradationFlagV1::ProofSuccessBelow80,
        ];
        let expected_error = || {
            SignedReputationSnapshotError::Snapshot(
                ReputationValidationError::TooManyDegradationFlags {
                    provider_id: provider_id.clone(),
                    count: 6,
                    max: MAX_REPUTATION_DEGRADATION_FLAGS,
                },
            )
        };

        assert_eq!(envelope.validate_structure(), Err(expected_error()));
        let bytes = norito::to_bytes(&envelope).expect("encode structurally invalid envelope");
        assert_eq!(
            decode_signed_reputation_snapshot(&bytes),
            Err(expected_error())
        );
    }

    #[test]
    fn bounded_canonical_decoders_reject_oversize_trailing_and_compressed_inputs() {
        let policy = policy();
        let mut trailing = policy.canonical_bytes().expect("encode policy");
        trailing.push(0);
        assert!(matches!(
            decode_reputation_trust_policy(&trailing),
            Err(SignedReputationSnapshotError::Decoding { .. })
        ));

        let compressed =
            norito::to_compressed_bytes(&policy, Some(norito::CompressionConfig::default()))
                .expect("compress policy");
        assert!(matches!(
            decode_reputation_trust_policy(&compressed),
            Err(SignedReputationSnapshotError::NonCanonicalEncoding { .. })
        ));

        let oversized = vec![0_u8; MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES + 1];
        assert_eq!(
            decode_reputation_trust_policy(&oversized),
            Err(SignedReputationSnapshotError::EncodedPayloadTooLarge {
                payload: "reputation trust policy",
                len: MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES + 1,
                max: MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES,
            })
        );
    }

    #[test]
    fn bounded_encoder_rejects_exact_oversize_before_serialization() {
        struct MustNotSerialize;

        impl norito::NoritoSerialize for MustNotSerialize {
            fn serialize(
                &self,
                _writer: &mut norito::core::Encoder<'_>,
            ) -> Result<(), norito::core::Error> {
                panic!("oversized value must be rejected before serialization")
            }

            fn encoded_len_exact(&self) -> Option<usize> {
                Some(17)
            }
        }

        assert_eq!(
            encode_canonical_bounded("test payload", &MustNotSerialize, 16),
            Err(SignedReputationSnapshotError::EncodedPayloadTooLarge {
                payload: "test payload",
                len: 17,
                max: 16,
            })
        );
    }

    #[test]
    fn signed_snapshot_size_preflight_accepts_boundary_and_rejects_one_over() {
        let (_, envelope) = signed_snapshot();
        let exact = norito::core::encoded_payload_len(&envelope)
            .expect("signed snapshot canonical length must be countable");

        assert_eq!(
            preflight_canonical_encoded_len("signed reputation snapshot", &envelope, exact),
            Ok(exact)
        );
        assert_eq!(
            preflight_canonical_encoded_len(
                "signed reputation snapshot",
                &envelope,
                exact.saturating_sub(1),
            ),
            Err(SignedReputationSnapshotError::EncodedPayloadTooLarge {
                payload: "signed reputation snapshot",
                len: exact,
                max: exact.saturating_sub(1),
            })
        );
    }

    #[test]
    fn snapshot_tampering_and_policy_substitution_fail() {
        let (policy, envelope) = signed_snapshot();

        let mut tampered = envelope.clone();
        tampered.snapshot.providers[0].score_bps -= 1;
        assert!(matches!(
            tampered.verify(&policy, GENERATED_AT + 10),
            Err(SignedReputationSnapshotError::Snapshot(_))
        ));

        let mut substituted_policy = policy.clone();
        substituted_policy.policy_id[0] ^= 1;
        assert_eq!(
            envelope.verify(&substituted_policy, GENERATED_AT + 10),
            Err(SignedReputationSnapshotError::PolicyDigestMismatch)
        );
    }

    #[test]
    fn scoring_evidence_digest_and_exact_replay_are_mandatory() {
        let (policy, envelope) = signed_snapshot();

        let mut digest_tampered = envelope.clone();
        digest_tampered.scoring_evidence.provider_inputs[0]
            .metrics
            .por_success_bps -= 1;
        assert_eq!(
            digest_tampered.verify(&policy, GENERATED_AT + 10),
            Err(SignedReputationSnapshotError::ScoringEvidenceDigestMismatch)
        );

        let mut replay_tampered = digest_tampered;
        replay_tampered.scoring_evidence_digest = replay_tampered
            .scoring_evidence
            .canonical_digest()
            .expect("tampered evidence digest");
        assert_eq!(
            replay_tampered.verify(&policy, GENERATED_AT + 10),
            Err(SignedReputationSnapshotError::ScoringReplayMismatch)
        );
    }

    #[test]
    fn scoring_evidence_rejects_duplicate_and_unsorted_providers() {
        let mut duplicate = scoring_evidence();
        duplicate.provider_inputs[1] = duplicate.provider_inputs[0].clone();
        assert_eq!(
            duplicate.validate(),
            Err(SignedReputationSnapshotError::DuplicateScoringProvider {
                provider_id: "provider-a".to_string(),
            })
        );

        let mut unsorted = scoring_evidence();
        unsorted.provider_inputs.swap(0, 1);
        assert_eq!(
            unsorted.validate(),
            Err(SignedReputationSnapshotError::ScoringProvidersNotSorted)
        );
    }

    #[test]
    fn signature_quorum_duplicates_order_and_unknown_signers_fail_closed() {
        let (policy, envelope) = signed_snapshot();

        let mut below_quorum = envelope.clone();
        below_quorum.signatures.pop();
        assert_eq!(
            below_quorum.verify(&policy, GENERATED_AT + 10),
            Err(SignedReputationSnapshotError::SignatureQuorumNotMet {
                found: 1,
                required: 2,
            })
        );

        let mut duplicate = envelope.clone();
        duplicate.signatures[1] = duplicate.signatures[0].clone();
        assert_eq!(
            duplicate.verify(&policy, GENERATED_AT + 10),
            Err(SignedReputationSnapshotError::DuplicateSnapshotSigner {
                signer_id: "council-1".to_string(),
            })
        );

        let mut out_of_order = envelope.clone();
        out_of_order.signatures.swap(0, 1);
        assert_eq!(
            out_of_order.verify(&policy, GENERATED_AT + 10),
            Err(SignedReputationSnapshotError::SnapshotSignaturesNotSorted)
        );

        let mut unknown = envelope;
        unknown.signatures[1].signer_id = "council-9".to_string();
        assert_eq!(
            unknown.verify(&policy, GENERATED_AT + 10),
            Err(SignedReputationSnapshotError::UntrustedSnapshotSigner {
                signer_id: "council-9".to_string(),
            })
        );
    }

    #[test]
    fn malformed_and_wrong_signatures_fail_closed() {
        let (policy, envelope) = signed_snapshot();

        let mut inert = envelope.clone();
        inert.signatures[0].signature = [0; ed25519_dalek::SIGNATURE_LENGTH];
        assert!(matches!(
            inert.verify(&policy, GENERATED_AT + 10),
            Err(SignedReputationSnapshotError::InvalidSignature { .. })
        ));

        let mut wrong = envelope;
        wrong.signatures[0].signature[63] ^= 1;
        assert!(matches!(
            wrong.verify(&policy, GENERATED_AT + 10),
            Err(SignedReputationSnapshotError::SignatureVerification { .. })
                | Err(SignedReputationSnapshotError::InvalidSignature { .. })
        ));
    }

    #[test]
    fn freshness_and_policy_windows_fail_closed_at_boundaries() {
        let (policy, envelope) = signed_snapshot();
        assert_eq!(
            envelope.verify(&policy, GENERATED_AT + 601),
            Err(SignedReputationSnapshotError::SnapshotTooOld)
        );
        assert_eq!(
            envelope.verify(&policy, policy.valid_until_unix),
            Err(SignedReputationSnapshotError::PolicyInactiveAtAdmission)
        );

        let mut future = envelope;
        future.snapshot = snapshot([0x12; 16], GENERATED_AT + 31, None);
        let digest = future.signing_digest().expect("future signing digest");
        let keys = signing_keys();
        for (index, signature) in future.signatures.iter_mut().enumerate() {
            signature.signature = keys[index].sign(&digest).to_bytes();
        }
        assert_eq!(
            future.verify(&policy, GENERATED_AT),
            Err(SignedReputationSnapshotError::SnapshotFromFuture)
        );
    }

    #[test]
    fn policy_rejects_weak_duplicate_and_noncanonical_signers() {
        let mut weak = policy();
        weak.signers[0].public_key = [0; ed25519_dalek::PUBLIC_KEY_LENGTH];
        assert!(matches!(
            weak.validate(),
            Err(SignedReputationSnapshotError::InvalidPublicKey { .. })
        ));

        let mut duplicate_key = policy();
        duplicate_key.signers[1].public_key = duplicate_key.signers[0].public_key;
        assert_eq!(
            duplicate_key.validate(),
            Err(SignedReputationSnapshotError::DuplicateTrustedPublicKey)
        );

        let mut duplicate_id = policy();
        duplicate_id.signers[1].signer_id = duplicate_id.signers[0].signer_id.clone();
        assert_eq!(
            duplicate_id.validate(),
            Err(SignedReputationSnapshotError::DuplicateTrustedSigner {
                signer_id: "council-1".to_string(),
            })
        );

        let mut unsorted = policy();
        unsorted.signers.swap(0, 1);
        assert_eq!(
            unsorted.validate(),
            Err(SignedReputationSnapshotError::TrustedSignersNotSorted)
        );
    }

    #[test]
    fn policy_rejects_invalid_threshold_and_freshness_limits() {
        let mut zero_threshold = policy();
        zero_threshold.min_signatures = 0;
        assert_eq!(
            zero_threshold.validate(),
            Err(SignedReputationSnapshotError::InvalidSignatureThreshold {
                threshold: 0,
                signer_count: 3,
            })
        );

        let mut excessive_threshold = policy();
        excessive_threshold.min_signatures = 4;
        assert_eq!(
            excessive_threshold.validate(),
            Err(SignedReputationSnapshotError::InvalidSignatureThreshold {
                threshold: 4,
                signer_count: 3,
            })
        );

        let mut age = policy();
        age.max_snapshot_age_secs = 0;
        assert!(matches!(
            age.validate(),
            Err(SignedReputationSnapshotError::InvalidMaximumSnapshotAge { .. })
        ));

        let mut skew = policy();
        skew.max_future_skew_secs = MAX_REPUTATION_FUTURE_SKEW_SECS + 1;
        assert!(matches!(
            skew.validate(),
            Err(SignedReputationSnapshotError::InvalidMaximumFutureSkew { .. })
        ));
    }

    #[test]
    fn revoked_signers_cannot_contribute_to_threshold() {
        let (mut revoked_policy, envelope) = signed_snapshot();
        revoked_policy.revoked_signer_ids = vec!["council-2".to_string()];
        revoked_policy.min_signatures = 2;
        let policy_digest = revoked_policy
            .canonical_digest()
            .expect("revoked policy digest");
        let mut resigned = envelope;
        resigned.policy_digest = policy_digest;
        let digest = resigned.signing_digest().expect("revoked signing digest");
        let keys = signing_keys();
        for (index, signature) in resigned.signatures.iter_mut().enumerate() {
            signature.signature = keys[index].sign(&digest).to_bytes();
        }

        assert_eq!(
            resigned.verify(&revoked_policy, GENERATED_AT + 10),
            Err(SignedReputationSnapshotError::RevokedSnapshotSigner {
                signer_id: "council-2".to_string(),
            })
        );

        let mut unknown = policy();
        unknown.revoked_signer_ids = vec!["council-9".to_string()];
        assert_eq!(
            unknown.validate(),
            Err(SignedReputationSnapshotError::UnknownRevokedSigner {
                signer_id: "council-9".to_string(),
            })
        );
    }

    #[test]
    fn snapshot_transition_requires_exact_head_and_monotonic_time() {
        let first = snapshot([0x21; 16], GENERATED_AT, None);
        validate_reputation_snapshot_transition(None, &first).expect("initial head");

        let second = snapshot([0x22; 16], GENERATED_AT + 1, Some(first.snapshot_id));
        validate_reputation_snapshot_transition(Some(&first), &second).expect("next head");

        let wrong_head = snapshot([0x23; 16], GENERATED_AT + 2, Some([0x99; 16]));
        assert_eq!(
            validate_reputation_snapshot_transition(Some(&first), &wrong_head),
            Err(SignedReputationSnapshotError::SnapshotPredecessorMismatch)
        );

        let stale = snapshot([0x24; 16], GENERATED_AT, Some(first.snapshot_id));
        assert_eq!(
            validate_reputation_snapshot_transition(Some(&first), &stale),
            Err(SignedReputationSnapshotError::SnapshotTimeDidNotAdvance)
        );

        assert_eq!(
            validate_reputation_snapshot_transition(None, &second),
            Err(SignedReputationSnapshotError::UnexpectedInitialPredecessor)
        );
    }
}
