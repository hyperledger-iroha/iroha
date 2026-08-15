//! Threshold-governed admission for canonical SoraFS pricing manifests.
use super::{PricingManifestError, PricingManifestV1};
use blake3::Hasher;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use std::cmp::Ordering;
use thiserror::Error;
/// Schema version for [`PricingTrustedSignerV1`].
pub const PRICING_TRUSTED_SIGNER_VERSION_V1: u8 = 1;
/// Schema version for [`PricingTrustPolicyV1`].
pub const PRICING_TRUST_POLICY_VERSION_V1: u8 = 1;
/// Schema version for [`PricingManifestSignatureV1`].
pub const PRICING_MANIFEST_SIGNATURE_VERSION_V1: u8 = 1;
/// Schema version for [`GovernedPricingManifestV1`].
pub const GOVERNED_PRICING_MANIFEST_VERSION_V1: u8 = 1;
/// Schema version for [`GovernedPricingSeriesV1`].
pub const GOVERNED_PRICING_SERIES_VERSION_V1: u8 = 1;
/// Maximum trusted pricing signers.
pub const MAX_PRICING_TRUSTED_SIGNERS: usize = 64;
/// Maximum signature count retained on one pricing manifest.
pub const MAX_PRICING_MANIFEST_SIGNATURES: usize = 64;
/// Maximum signer identifier byte length.
pub const MAX_PRICING_SIGNER_ID_BYTES: usize = 128;
/// Maximum future activation any V1 policy may permit.
pub const MAX_PRICING_FUTURE_ACTIVATION_SECS: u64 = 30 * 24 * 60 * 60;
/// Maximum canonical pricing trust-policy bytes.
pub const MAX_PRICING_TRUST_POLICY_BYTES: usize = 64 * 1024;
/// Maximum canonical governed pricing-manifest bytes.
pub const MAX_GOVERNED_PRICING_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
/// Maximum retained admissions in one durable pricing series checkpoint.
pub const MAX_GOVERNED_PRICING_SERIES_ENTRIES: usize = 1_024;
/// Maximum canonical durable pricing-series checkpoint bytes.
pub const MAX_GOVERNED_PRICING_SERIES_BYTES: usize = 32 * 1024 * 1024;
/// One strong Ed25519 signer authorized by pricing governance.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PricingTrustedSignerV1 {
    /// Schema version.
    pub version: u8,
    /// Stable governance identity.
    pub signer_id: String,
    /// Canonical strong Ed25519 public key.
    pub public_key: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH],
}
impl PricingTrustedSignerV1 {
    /// Validate identifier and strong public key.
    pub fn validate(&self) -> Result<(), GovernedPricingError> {
        if self.version != PRICING_TRUSTED_SIGNER_VERSION_V1 {
            return Err(GovernedPricingError::UnsupportedTrustedSignerVersion {
                found: self.version,
            });
        }
        validate_signer_id(&self.signer_id)?;
        crate::checked_ed25519_verifying_key_from_bytes(&self.public_key).map_err(|reason| {
            GovernedPricingError::InvalidPublicKey {
                signer_id: self.signer_id.clone(),
                reason,
            }
        })?;
        Ok(())
    }
}
/// External trust, currency, threshold, and activation policy for pricing.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PricingTrustPolicyV1 {
    /// Schema version.
    pub version: u8,
    /// Non-zero governance policy id.
    pub policy_id: [u8; 32],
    /// First policy-valid Unix second, inclusive.
    pub valid_from_unix: u64,
    /// Policy expiry Unix second, exclusive.
    pub valid_until_unix: u64,
    /// Exact currency this policy authorizes.
    pub currency: String,
    /// Maximum accepted scheduled activation into the future.
    pub max_future_activation_secs: u64,
    /// Minimum distinct active signatures.
    pub min_signatures: u16,
    /// Trusted signers in strictly increasing signer-id order.
    pub signers: Vec<PricingTrustedSignerV1>,
    /// Explicit revoked signer ids in strictly increasing order.
    pub revoked_signer_ids: Vec<String>,
}
impl PricingTrustPolicyV1 {
    /// Validate trust roots, threshold, currency, validity, and revocations.
    pub fn validate(&self) -> Result<(), GovernedPricingError> {
        if self.version != PRICING_TRUST_POLICY_VERSION_V1 {
            return Err(GovernedPricingError::UnsupportedTrustPolicyVersion {
                found: self.version,
            });
        }
        if crate::inert_bytes(&self.policy_id) {
            return Err(GovernedPricingError::InvalidPolicyId);
        }
        if self.valid_from_unix == 0 || self.valid_until_unix <= self.valid_from_unix {
            return Err(GovernedPricingError::InvalidPolicyValidity);
        }
        super::validate_currency(&self.currency)
            .map_err(GovernedPricingError::InvalidPolicyCurrency)?;
        if self.max_future_activation_secs > MAX_PRICING_FUTURE_ACTIVATION_SECS {
            return Err(GovernedPricingError::InvalidFutureActivationLimit {
                found: self.max_future_activation_secs,
                max: MAX_PRICING_FUTURE_ACTIVATION_SECS,
            });
        }
        if self.signers.is_empty() {
            return Err(GovernedPricingError::EmptyTrustedSignerSet);
        }
        if self.signers.len() > MAX_PRICING_TRUSTED_SIGNERS {
            return Err(GovernedPricingError::ResourceLimitExceeded {
                field: "signers",
                count: self.signers.len(),
                max: MAX_PRICING_TRUSTED_SIGNERS,
            });
        }
        let mut previous_id: Option<&str> = None;
        for (index, signer) in self.signers.iter().enumerate() {
            signer.validate()?;
            if let Some(previous) = previous_id {
                match previous.cmp(signer.signer_id.as_str()) {
                    Ordering::Equal => {
                        return Err(GovernedPricingError::DuplicateTrustedSigner {
                            signer_id: signer.signer_id.clone(),
                        });
                    }
                    Ordering::Greater => {
                        return Err(GovernedPricingError::NonCanonicalOrder { field: "signers" });
                    }
                    Ordering::Less => {}
                }
            }
            if self.signers[..index]
                .iter()
                .any(|previous| previous.public_key == signer.public_key)
            {
                return Err(GovernedPricingError::DuplicateTrustedPublicKey);
            }
            previous_id = Some(&signer.signer_id);
        }
        if self.revoked_signer_ids.len() > self.signers.len() {
            return Err(GovernedPricingError::ResourceLimitExceeded {
                field: "revoked_signer_ids",
                count: self.revoked_signer_ids.len(),
                max: self.signers.len(),
            });
        }
        let mut previous_revocation: Option<&str> = None;
        for signer_id in &self.revoked_signer_ids {
            validate_signer_id(signer_id)?;
            if let Some(previous) = previous_revocation {
                match previous.cmp(signer_id.as_str()) {
                    Ordering::Equal => {
                        return Err(GovernedPricingError::DuplicateRevocation {
                            signer_id: signer_id.clone(),
                        });
                    }
                    Ordering::Greater => {
                        return Err(GovernedPricingError::NonCanonicalOrder {
                            field: "revoked_signer_ids",
                        });
                    }
                    Ordering::Less => {}
                }
            }
            if self.signer(signer_id).is_none() {
                return Err(GovernedPricingError::UnknownRevokedSigner {
                    signer_id: signer_id.clone(),
                });
            }
            previous_revocation = Some(signer_id);
        }
        let active_count = self
            .signers
            .len()
            .checked_sub(self.revoked_signer_ids.len())
            .ok_or(GovernedPricingError::InvalidSignatureThreshold {
                threshold: self.min_signatures,
                active_signers: 0,
            })?;
        let threshold = usize::from(self.min_signatures);
        if threshold == 0 || threshold > active_count {
            return Err(GovernedPricingError::InvalidSignatureThreshold {
                threshold: self.min_signatures,
                active_signers: active_count,
            });
        }
        Ok(())
    }
    /// Return the domain-separated canonical policy digest.
    pub fn canonical_digest(&self) -> Result<[u8; 32], GovernedPricingError> {
        self.validate()?;
        hash_canonical(
            b"sorafs-pricing-trust-policy-v1",
            "pricing trust policy",
            self,
            MAX_PRICING_TRUST_POLICY_BYTES,
        )
    }
    /// Return bounded canonical policy bytes.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, GovernedPricingError> {
        self.validate()?;
        encode_canonical_bounded("pricing trust policy", self, MAX_PRICING_TRUST_POLICY_BYTES)
    }
    fn signer(&self, signer_id: &str) -> Option<&PricingTrustedSignerV1> {
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
/// One threshold signature retained on a governed pricing manifest.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PricingManifestSignatureV1 {
    /// Schema version.
    pub version: u8,
    /// Signer id from the external trust policy.
    pub signer_id: String,
    /// Strong canonical Ed25519 signature.
    pub signature: [u8; ed25519_dalek::SIGNATURE_LENGTH],
}
impl PricingManifestSignatureV1 {
    fn validate_structure(&self) -> Result<(), GovernedPricingError> {
        if self.version != PRICING_MANIFEST_SIGNATURE_VERSION_V1 {
            return Err(GovernedPricingError::UnsupportedManifestSignatureVersion {
                found: self.version,
            });
        }
        validate_signer_id(&self.signer_id)?;
        crate::checked_ed25519_signature_from_bytes(&self.signature).map_err(|reason| {
            GovernedPricingError::InvalidSignature {
                signer_id: self.signer_id.clone(),
                reason,
            }
        })?;
        Ok(())
    }
}
/// Canonical pricing manifest plus threshold governance authorization.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GovernedPricingManifestV1 {
    /// Schema version.
    pub version: u8,
    /// Digest of the external trust policy.
    pub policy_digest: [u8; 32],
    /// Domain-separated id derived from predecessor and canonical manifest bytes.
    pub pricing_id: [u8; 32],
    /// Exact predecessor id for a pricing series.
    #[norito(default)]
    pub previous_pricing_id: Option<[u8; 32]>,
    /// Intrinsic deterministic pricing manifest.
    pub manifest: PricingManifestV1,
    /// Distinct signatures in strictly increasing signer-id order.
    pub signatures: Vec<PricingManifestSignatureV1>,
}
impl GovernedPricingManifestV1 {
    /// Validate policy-independent structure, id binding, and signature encodings.
    pub fn validate_structure(&self) -> Result<(), GovernedPricingError> {
        if self.version != GOVERNED_PRICING_MANIFEST_VERSION_V1 {
            return Err(GovernedPricingError::UnsupportedGovernedManifestVersion {
                found: self.version,
            });
        }
        if crate::inert_bytes(&self.policy_digest) {
            return Err(GovernedPricingError::InvalidPolicyDigest);
        }
        self.manifest.validate()?;
        if let Some(previous) = self.previous_pricing_id
            && crate::inert_bytes(&previous)
        {
            return Err(GovernedPricingError::InvalidPreviousPricingId);
        }
        let expected_id = derive_pricing_id(&self.manifest, self.previous_pricing_id)?;
        if self.pricing_id != expected_id {
            return Err(GovernedPricingError::PricingIdMismatch);
        }
        if self.previous_pricing_id == Some(self.pricing_id) {
            return Err(GovernedPricingError::SelfReferentialPricingManifest);
        }
        if self.signatures.is_empty() {
            return Err(GovernedPricingError::EmptyManifestSignatures);
        }
        if self.signatures.len() > MAX_PRICING_MANIFEST_SIGNATURES {
            return Err(GovernedPricingError::ResourceLimitExceeded {
                field: "signatures",
                count: self.signatures.len(),
                max: MAX_PRICING_MANIFEST_SIGNATURES,
            });
        }
        let mut previous_id: Option<&str> = None;
        for signature in &self.signatures {
            signature.validate_structure()?;
            if let Some(previous) = previous_id {
                match previous.cmp(signature.signer_id.as_str()) {
                    Ordering::Equal => {
                        return Err(GovernedPricingError::DuplicateManifestSigner {
                            signer_id: signature.signer_id.clone(),
                        });
                    }
                    Ordering::Greater => {
                        return Err(GovernedPricingError::NonCanonicalOrder {
                            field: "signatures",
                        });
                    }
                    Ordering::Less => {}
                }
            }
            previous_id = Some(&signature.signer_id);
        }
        Ok(())
    }
    /// Return the exact digest signed by pricing governance.
    pub fn signing_digest(&self) -> Result<[u8; 32], GovernedPricingError> {
        if crate::inert_bytes(&self.policy_digest) {
            return Err(GovernedPricingError::InvalidPolicyDigest);
        }
        let expected_id = derive_pricing_id(&self.manifest, self.previous_pricing_id)?;
        if self.pricing_id != expected_id {
            return Err(GovernedPricingError::PricingIdMismatch);
        }
        let mut hasher = Hasher::new();
        hasher.update(b"sorafs-governed-pricing-manifest-signature-v1");
        hasher.update(&self.policy_digest);
        hasher.update(&self.pricing_id);
        Ok(*hasher.finalize().as_bytes())
    }
    /// Verify policy identity, currency, schedule, threshold, revocations, and signatures.
    pub fn verify(
        &self,
        policy: &PricingTrustPolicyV1,
        admitted_at_unix: u64,
    ) -> Result<(), GovernedPricingError> {
        self.validate_structure()?;
        policy.validate()?;
        if self.policy_digest != policy.canonical_digest()? {
            return Err(GovernedPricingError::PolicyDigestMismatch);
        }
        if self.manifest.currency != policy.currency {
            return Err(GovernedPricingError::CurrencyMismatch {
                expected: policy.currency.clone(),
                found: self.manifest.currency.clone(),
            });
        }
        validate_admission_time(policy, &self.manifest, admitted_at_unix)?;
        if self.signatures.len() > policy.signers.len() {
            return Err(GovernedPricingError::ResourceLimitExceeded {
                field: "signatures",
                count: self.signatures.len(),
                max: policy.signers.len(),
            });
        }
        if self.signatures.len() < usize::from(policy.min_signatures) {
            return Err(GovernedPricingError::SignatureQuorumNotMet {
                found: self.signatures.len(),
                required: policy.min_signatures,
            });
        }
        let digest = self.signing_digest()?;
        for signature in &self.signatures {
            let trusted = policy.signer(&signature.signer_id).ok_or_else(|| {
                GovernedPricingError::UntrustedSigner {
                    signer_id: signature.signer_id.clone(),
                }
            })?;
            if policy.is_revoked(&signature.signer_id) {
                return Err(GovernedPricingError::RevokedSigner {
                    signer_id: signature.signer_id.clone(),
                });
            }
            let parsed = crate::checked_ed25519_signature_from_bytes(&signature.signature)
                .map_err(|reason| GovernedPricingError::InvalidSignature {
                    signer_id: signature.signer_id.clone(),
                    reason,
                })?;
            let key = crate::checked_ed25519_verifying_key_from_bytes(&trusted.public_key)
                .map_err(|reason| GovernedPricingError::InvalidPublicKey {
                    signer_id: signature.signer_id.clone(),
                    reason,
                })?;
            key.verify_strict(&digest, &parsed).map_err(|error| {
                GovernedPricingError::SignatureVerification {
                    signer_id: signature.signer_id.clone(),
                    reason: error.to_string(),
                }
            })?;
        }
        Ok(())
    }
    /// Return bounded canonical governed-manifest bytes.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, GovernedPricingError> {
        self.validate_structure()?;
        encode_canonical_bounded(
            "governed pricing manifest",
            self,
            MAX_GOVERNED_PRICING_MANIFEST_BYTES,
        )
    }
}
/// Derive the canonical pricing id from predecessor and intrinsic manifest bytes.
pub fn derive_pricing_id(
    manifest: &PricingManifestV1,
    previous_pricing_id: Option<[u8; 32]>,
) -> Result<[u8; 32], GovernedPricingError> {
    manifest.validate()?;
    if previous_pricing_id.is_some_and(|previous| crate::inert_bytes(&previous)) {
        return Err(GovernedPricingError::InvalidPreviousPricingId);
    }
    let bytes = encode_canonical_bounded(
        "pricing manifest",
        manifest,
        MAX_GOVERNED_PRICING_MANIFEST_BYTES,
    )?;
    let length = u64::try_from(bytes.len()).map_err(|_| GovernedPricingError::LengthOverflow)?;
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs-pricing-manifest-id-v1");
    match previous_pricing_id {
        Some(previous) => {
            hasher.update(&[1]);
            hasher.update(&previous);
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hasher.update(&length.to_le_bytes());
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}
/// Validate exact predecessor linkage and strictly increasing activation time.
pub fn validate_governed_pricing_transition(
    previous: Option<&GovernedPricingManifestV1>,
    next: &GovernedPricingManifestV1,
) -> Result<(), GovernedPricingError> {
    next.validate_structure()?;
    match previous {
        Some(previous) => {
            previous.validate_structure()?;
            if next.previous_pricing_id != Some(previous.pricing_id) {
                return Err(GovernedPricingError::PreviousPricingMismatch);
            }
            if next.manifest.effective_from_unix <= previous.manifest.effective_from_unix {
                return Err(GovernedPricingError::ActivationDidNotAdvance);
            }
        }
        None if next.previous_pricing_id.is_some() => {
            return Err(GovernedPricingError::UnexpectedInitialPredecessor);
        }
        None => {}
    }
    Ok(())
}
/// One governed pricing manifest and the exact time it passed admission.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GovernedPricingAdmissionV1 {
    /// Unix second at which threshold verification and admission completed.
    admitted_at_unix: u64,
    /// Exact threshold-signed pricing manifest retained for deterministic replay.
    governed: GovernedPricingManifestV1,
}
impl GovernedPricingAdmissionV1 {
    /// Return the exact admission Unix second.
    #[must_use]
    pub const fn admitted_at_unix(&self) -> u64 {
        self.admitted_at_unix
    }
    /// Return the retained threshold-signed manifest.
    #[must_use]
    pub const fn governed(&self) -> &GovernedPricingManifestV1 {
        &self.governed
    }
}
/// Durable, replay-verifiable series of governed pricing activations.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GovernedPricingSeriesV1 {
    /// Schema version.
    version: u8,
    /// Digest of the external trust policy used for every admission.
    policy_digest: [u8; 32],
    /// Admissions in exact predecessor and non-decreasing admission-time order.
    admissions: Vec<GovernedPricingAdmissionV1>,
}
impl GovernedPricingSeriesV1 {
    /// Construct an empty series bound to a validated external policy.
    pub fn new(policy: &PricingTrustPolicyV1) -> Result<Self, GovernedPricingError> {
        policy.validate()?;
        Ok(Self {
            version: GOVERNED_PRICING_SERIES_VERSION_V1,
            policy_digest: policy.canonical_digest()?,
            admissions: Vec::new(),
        })
    }
    /// Replay every retained admission against the external trust policy.
    pub fn validate(&self, policy: &PricingTrustPolicyV1) -> Result<(), GovernedPricingError> {
        if self.version != GOVERNED_PRICING_SERIES_VERSION_V1 {
            return Err(GovernedPricingError::UnsupportedPricingSeriesVersion {
                found: self.version,
            });
        }
        policy.validate()?;
        let policy_digest = policy.canonical_digest()?;
        if self.policy_digest != policy_digest {
            return Err(GovernedPricingError::SeriesPolicyDigestMismatch);
        }
        if self.admissions.len() > MAX_GOVERNED_PRICING_SERIES_ENTRIES {
            return Err(GovernedPricingError::ResourceLimitExceeded {
                field: "pricing_series_admissions",
                count: self.admissions.len(),
                max: MAX_GOVERNED_PRICING_SERIES_ENTRIES,
            });
        }
        let mut previous: Option<&GovernedPricingManifestV1> = None;
        let mut previous_admitted_at = 0_u64;
        for admission in &self.admissions {
            if admission.admitted_at_unix < previous_admitted_at {
                return Err(GovernedPricingError::AdmissionTimeRollback {
                    previous: previous_admitted_at,
                    found: admission.admitted_at_unix,
                });
            }
            admission
                .governed
                .verify(policy, admission.admitted_at_unix)?;
            validate_governed_pricing_transition(previous, &admission.governed)?;
            previous = Some(&admission.governed);
            previous_admitted_at = admission.admitted_at_unix;
        }
        Ok(())
    }
    /// Verify and append one exact-head successor without partial mutation.
    pub fn admit(
        &mut self,
        policy: &PricingTrustPolicyV1,
        governed: GovernedPricingManifestV1,
        admitted_at_unix: u64,
    ) -> Result<(), GovernedPricingError> {
        self.validate(policy)?;
        if self.admissions.len() == MAX_GOVERNED_PRICING_SERIES_ENTRIES {
            return Err(GovernedPricingError::ResourceLimitExceeded {
                field: "pricing_series_admissions",
                count: self.admissions.len().saturating_add(1),
                max: MAX_GOVERNED_PRICING_SERIES_ENTRIES,
            });
        }
        if let Some(previous) = self.admissions.last()
            && admitted_at_unix < previous.admitted_at_unix
        {
            return Err(GovernedPricingError::AdmissionTimeRollback {
                previous: previous.admitted_at_unix,
                found: admitted_at_unix,
            });
        }
        governed.verify(policy, admitted_at_unix)?;
        validate_governed_pricing_transition(
            self.admissions.last().map(|entry| &entry.governed),
            &governed,
        )?;
        self.admissions
            .try_reserve(1)
            .map_err(|_| GovernedPricingError::AllocationFailed {
                context: "pricing series admission",
            })?;
        self.admissions.push(GovernedPricingAdmissionV1 {
            admitted_at_unix,
            governed,
        });
        Ok(())
    }
    /// Return the external policy digest bound into this series.
    #[must_use]
    pub const fn policy_digest(&self) -> &[u8; 32] {
        &self.policy_digest
    }
    /// Return the number of retained governed admissions.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.admissions.len()
    }
    /// Return `true` when no pricing schedule has been admitted.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.admissions.is_empty()
    }
    /// Return the immutable admission inventory for audit/readback.
    #[must_use]
    pub fn admissions(&self) -> &[GovernedPricingAdmissionV1] {
        &self.admissions
    }
    /// Validate and return the exact admitted chain head, including future activations.
    pub fn head(
        &self,
        policy: &PricingTrustPolicyV1,
    ) -> Result<Option<&GovernedPricingManifestV1>, GovernedPricingError> {
        self.validate(policy)?;
        Ok(self.admissions.last().map(|entry| &entry.governed))
    }
    /// Return the most recent pricing manifest effective at `observed_at_unix`.
    pub fn active_at(
        &self,
        policy: &PricingTrustPolicyV1,
        observed_at_unix: u64,
    ) -> Result<Option<&GovernedPricingManifestV1>, GovernedPricingError> {
        if observed_at_unix == 0 {
            return Err(GovernedPricingError::InvalidObservationTime);
        }
        self.validate(policy)?;
        Ok(self
            .admissions
            .iter()
            .rev()
            .find(|entry| entry.governed.manifest.effective_from_unix <= observed_at_unix)
            .map(|entry| &entry.governed))
    }
    /// Return a bounded canonical checkpoint for durable restart recovery.
    pub fn canonical_bytes(
        &self,
        policy: &PricingTrustPolicyV1,
    ) -> Result<Vec<u8>, GovernedPricingError> {
        self.validate(policy)?;
        encode_canonical_bounded(
            "governed pricing series",
            self,
            MAX_GOVERNED_PRICING_SERIES_BYTES,
        )
    }
}
/// Decode one bounded canonical pricing trust policy.
pub fn decode_pricing_trust_policy(
    bytes: &[u8],
) -> Result<PricingTrustPolicyV1, GovernedPricingError> {
    let policy: PricingTrustPolicyV1 = decode_canonical(
        "pricing trust policy",
        bytes,
        MAX_PRICING_TRUST_POLICY_BYTES,
        norito::DecodeLimits::new(
            MAX_PRICING_SIGNER_ID_BYTES,
            MAX_PRICING_TRUST_POLICY_BYTES,
            8_192,
            MAX_PRICING_TRUST_POLICY_BYTES * 4,
            32,
        ),
    )?;
    policy.validate()?;
    Ok(policy)
}
/// Decode one bounded canonical governed pricing manifest.
pub fn decode_governed_pricing_manifest(
    bytes: &[u8],
) -> Result<GovernedPricingManifestV1, GovernedPricingError> {
    let manifest: GovernedPricingManifestV1 = decode_canonical(
        "governed pricing manifest",
        bytes,
        MAX_GOVERNED_PRICING_MANIFEST_BYTES,
        norito::DecodeLimits::new(
            super::MAX_PRICING_NONCE_SAMPLES.max(super::MAX_PRICING_TIERS),
            MAX_GOVERNED_PRICING_MANIFEST_BYTES,
            1_000_000,
            MAX_GOVERNED_PRICING_MANIFEST_BYTES * 4,
            64,
        ),
    )?;
    manifest.validate_structure()?;
    Ok(manifest)
}
/// Decode and fully replay one bounded canonical pricing-series checkpoint.
pub fn decode_governed_pricing_series(
    bytes: &[u8],
    policy: &PricingTrustPolicyV1,
) -> Result<GovernedPricingSeriesV1, GovernedPricingError> {
    let series: GovernedPricingSeriesV1 = decode_canonical(
        "governed pricing series",
        bytes,
        MAX_GOVERNED_PRICING_SERIES_BYTES,
        norito::DecodeLimits::new(
            MAX_GOVERNED_PRICING_MANIFEST_BYTES,
            MAX_GOVERNED_PRICING_SERIES_BYTES,
            2_000_000,
            MAX_GOVERNED_PRICING_SERIES_BYTES * 4,
            96,
        ),
    )?;
    series.validate(policy)?;
    Ok(series)
}
fn validate_admission_time(
    policy: &PricingTrustPolicyV1,
    manifest: &PricingManifestV1,
    admitted_at_unix: u64,
) -> Result<(), GovernedPricingError> {
    if admitted_at_unix == 0 {
        return Err(GovernedPricingError::InvalidAdmissionTime);
    }
    if !(policy.valid_from_unix..policy.valid_until_unix).contains(&admitted_at_unix) {
        return Err(GovernedPricingError::PolicyInactiveAtAdmission);
    }
    if !(policy.valid_from_unix..policy.valid_until_unix).contains(&manifest.effective_from_unix) {
        return Err(GovernedPricingError::PolicyInactiveAtActivation);
    }
    if manifest.effective_from_unix < admitted_at_unix {
        return Err(GovernedPricingError::ActivationBeforeAdmission);
    }
    let latest = admitted_at_unix
        .checked_add(policy.max_future_activation_secs)
        .ok_or(GovernedPricingError::TimeOverflow)?;
    if manifest.effective_from_unix > latest {
        return Err(GovernedPricingError::ActivationTooFarInFuture);
    }
    Ok(())
}
fn validate_signer_id(signer_id: &str) -> Result<(), GovernedPricingError> {
    if signer_id.is_empty()
        || signer_id.len() > MAX_PRICING_SIGNER_ID_BYTES
        || !signer_id.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'-' | b'_' | b':')
        })
    {
        return Err(GovernedPricingError::InvalidSignerId);
    }
    Ok(())
}
fn hash_canonical<T: norito::NoritoSerialize>(
    domain: &[u8],
    payload: &'static str,
    value: &T,
    max_bytes: usize,
) -> Result<[u8; 32], GovernedPricingError> {
    let bytes = encode_canonical_bounded(payload, value, max_bytes)?;
    let length = u64::try_from(bytes.len()).map_err(|_| GovernedPricingError::LengthOverflow)?;
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(&length.to_le_bytes());
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}
fn encode_canonical_bounded<T: norito::NoritoSerialize>(
    payload: &'static str,
    value: &T,
    max_bytes: usize,
) -> Result<Vec<u8>, GovernedPricingError> {
    let exact = norito::core::encoded_frame_len(value)
        .map_err(|error| GovernedPricingError::Encoding(error.to_string()))?;
    if exact > max_bytes {
        return Err(GovernedPricingError::EncodedPayloadTooLarge {
            payload,
            length: exact,
            max: max_bytes,
        });
    }
    let bytes = norito::to_bytes(value)
        .map_err(|error| GovernedPricingError::Encoding(error.to_string()))?;
    if bytes.len() > max_bytes {
        return Err(GovernedPricingError::EncodedPayloadTooLarge {
            payload,
            length: bytes.len(),
            max: max_bytes,
        });
    }
    Ok(bytes)
}
fn decode_canonical<T>(
    payload: &'static str,
    bytes: &[u8],
    max_bytes: usize,
    limits: norito::DecodeLimits,
) -> Result<T, GovernedPricingError>
where
    T: for<'decode> norito::NoritoDeserialize<'decode> + norito::NoritoSerialize,
{
    if bytes.len() > max_bytes {
        return Err(GovernedPricingError::EncodedPayloadTooLarge {
            payload,
            length: bytes.len(),
            max: max_bytes,
        });
    }
    let decoded = norito::decode_from_bytes_with_limits(bytes, limits).map_err(|error| {
        GovernedPricingError::Decoding {
            payload,
            reason: error.to_string(),
        }
    })?;
    let canonical = encode_canonical_bounded(payload, &decoded, max_bytes)?;
    if canonical != bytes {
        return Err(GovernedPricingError::NonCanonicalEncoding { payload });
    }
    Ok(decoded)
}
/// Governed pricing validation failures.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GovernedPricingError {
    /// Trusted signer version is unsupported.
    #[error("unsupported pricing trusted-signer version {found}")]
    UnsupportedTrustedSignerVersion { found: u8 },
    /// Trust policy version is unsupported.
    #[error("unsupported pricing trust-policy version {found}")]
    UnsupportedTrustPolicyVersion { found: u8 },
    /// Signature record version is unsupported.
    #[error("unsupported pricing signature version {found}")]
    UnsupportedManifestSignatureVersion { found: u8 },
    /// Governed manifest version is unsupported.
    #[error("unsupported governed pricing-manifest version {found}")]
    UnsupportedGovernedManifestVersion { found: u8 },
    /// Durable pricing-series version is unsupported.
    #[error("unsupported governed pricing-series version {found}")]
    UnsupportedPricingSeriesVersion { found: u8 },
    /// Signer id is malformed.
    #[error("pricing signer identifier is malformed")]
    InvalidSignerId,
    /// Policy id is inert.
    #[error("pricing policy id must not be zero")]
    InvalidPolicyId,
    /// Policy digest is inert.
    #[error("pricing policy digest must not be zero")]
    InvalidPolicyDigest,
    /// Policy validity is malformed.
    #[error("pricing policy validity interval is invalid")]
    InvalidPolicyValidity,
    /// Policy currency is malformed.
    #[error("invalid pricing policy currency: {0}")]
    InvalidPolicyCurrency(String),
    /// Future activation limit exceeds V1.
    #[error("pricing future activation limit {found}s exceeds {max}s")]
    InvalidFutureActivationLimit { found: u64, max: u64 },
    /// Trusted signer set is empty.
    #[error("pricing policy has no trusted signers")]
    EmptyTrustedSignerSet,
    /// Signature threshold exceeds active signers or is zero.
    #[error("pricing threshold {threshold} is invalid for {active_signers} active signers")]
    InvalidSignatureThreshold {
        threshold: u16,
        active_signers: usize,
    },
    /// Duplicate trusted signer id.
    #[error("duplicate pricing trusted signer `{signer_id}`")]
    DuplicateTrustedSigner { signer_id: String },
    /// Trusted public key is reused.
    #[error("pricing policy reuses a trusted public key")]
    DuplicateTrustedPublicKey,
    /// Duplicate revocation.
    #[error("duplicate pricing signer revocation `{signer_id}`")]
    DuplicateRevocation { signer_id: String },
    /// Unknown revoked signer.
    #[error("pricing policy revokes unknown signer `{signer_id}`")]
    UnknownRevokedSigner { signer_id: String },
    /// Sequence is not canonical.
    #[error("{field} must be in canonical order")]
    NonCanonicalOrder { field: &'static str },
    /// Collection exceeds schema bound.
    #[error("{field} count {count} exceeds maximum {max}")]
    ResourceLimitExceeded {
        field: &'static str,
        count: usize,
        max: usize,
    },
    /// Strong public-key validation failed.
    #[error("invalid pricing public key for `{signer_id}`: {reason}")]
    InvalidPublicKey { signer_id: String, reason: String },
    /// Signature structure is invalid.
    #[error("invalid pricing signature for `{signer_id}`: {reason}")]
    InvalidSignature { signer_id: String, reason: String },
    /// Previous id is inert.
    #[error("previous pricing id must not be zero")]
    InvalidPreviousPricingId,
    /// Pricing id does not bind manifest and predecessor.
    #[error("pricing id does not match canonical manifest and predecessor")]
    PricingIdMismatch,
    /// Pricing manifest points to itself.
    #[error("pricing manifest must not reference itself as predecessor")]
    SelfReferentialPricingManifest,
    /// Signature list is empty.
    #[error("governed pricing manifest has no signatures")]
    EmptyManifestSignatures,
    /// Manifest signer is duplicated.
    #[error("duplicate pricing manifest signer `{signer_id}`")]
    DuplicateManifestSigner { signer_id: String },
    /// External policy digest differs.
    #[error("governed pricing policy digest mismatch")]
    PolicyDigestMismatch,
    /// Durable series is bound to a different external policy.
    #[error("governed pricing series policy digest mismatch")]
    SeriesPolicyDigestMismatch,
    /// Manifest currency differs from policy.
    #[error("pricing currency `{found}` differs from governed `{expected}`")]
    CurrencyMismatch { expected: String, found: String },
    /// Admission time is zero.
    #[error("pricing admission time must not be zero")]
    InvalidAdmissionTime,
    /// Active-price observation time is zero.
    #[error("pricing observation time must not be zero")]
    InvalidObservationTime,
    /// Policy inactive at admission.
    #[error("pricing policy is inactive at admission")]
    PolicyInactiveAtAdmission,
    /// Policy inactive at manifest activation.
    #[error("pricing policy is inactive at manifest activation")]
    PolicyInactiveAtActivation,
    /// Scheduled activation exceeds policy.
    #[error("pricing activation is too far in the future")]
    ActivationTooFarInFuture,
    /// Scheduled activation predates admission and would retroactively reprice usage.
    #[error("pricing activation must not precede admission")]
    ActivationBeforeAdmission,
    /// Time arithmetic overflow.
    #[error("pricing time arithmetic overflow")]
    TimeOverflow,
    /// Signature quorum not met.
    #[error("pricing manifest has {found} signatures; {required} required")]
    SignatureQuorumNotMet { found: usize, required: u16 },
    /// Manifest signer is not trusted.
    #[error("untrusted pricing signer `{signer_id}`")]
    UntrustedSigner { signer_id: String },
    /// Manifest signer is revoked.
    #[error("revoked pricing signer `{signer_id}`")]
    RevokedSigner { signer_id: String },
    /// Cryptographic signature verification failed.
    #[error("pricing signature verification failed for `{signer_id}`: {reason}")]
    SignatureVerification { signer_id: String, reason: String },
    /// Transition does not extend exact head.
    #[error("pricing predecessor does not match retained head")]
    PreviousPricingMismatch,
    /// Activation did not strictly advance.
    #[error("pricing activation time did not advance")]
    ActivationDidNotAdvance,
    /// Initial manifest unexpectedly names predecessor.
    #[error("initial pricing manifest must not name a predecessor")]
    UnexpectedInitialPredecessor,
    /// Durable admission clock moved backwards.
    #[error("pricing admission time moved backwards from {previous} to {found}")]
    AdmissionTimeRollback { previous: u64, found: u64 },
    /// Fallible state growth could not reserve memory.
    #[error("pricing allocation failed while extending {context}")]
    AllocationFailed { context: &'static str },
    /// Exact encoded length unavailable.
    #[error("{payload} does not expose an exact canonical encoded length")]
    EncodedLengthUnavailable { payload: &'static str },
    /// Encoded payload exceeds cap.
    #[error("{payload} encoded length {length} exceeds maximum {max}")]
    EncodedPayloadTooLarge {
        payload: &'static str,
        length: usize,
        max: usize,
    },
    /// Encoded length overflow.
    #[error("pricing encoded length overflow")]
    LengthOverflow,
    /// Bounded decode failure.
    #[error("failed to decode {payload}: {reason}")]
    Decoding {
        payload: &'static str,
        reason: String,
    },
    /// Noncanonical Norito input.
    #[error("{payload} is not canonical Norito")]
    NonCanonicalEncoding { payload: &'static str },
    /// Canonical encoding failure.
    #[error("pricing encoding failed: {0}")]
    Encoding(String),
    /// Intrinsic manifest validation failure.
    #[error(transparent)]
    Manifest(#[from] PricingManifestError),
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::XorQuantity;
    use crate::pricing::{
        BondPolicyV1, CreditPolicyV1, PRICING_MANIFEST_VERSION_V1, PricingMicropaymentPolicyV1,
        PricingTierV1,
    };
    use ed25519_dalek::{Signer, SigningKey};
    const ADMITTED_AT: u64 = 1_800_000_000;
    fn xor(value: &str) -> XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }
    fn keys() -> [SigningKey; 3] {
        [
            SigningKey::from_bytes(&[31; 32]),
            SigningKey::from_bytes(&[32; 32]),
            SigningKey::from_bytes(&[33; 32]),
        ]
    }
    fn policy() -> PricingTrustPolicyV1 {
        let keys = keys();
        PricingTrustPolicyV1 {
            version: PRICING_TRUST_POLICY_VERSION_V1,
            policy_id: [0xB5; 32],
            valid_from_unix: ADMITTED_AT - 1_000,
            valid_until_unix: ADMITTED_AT + 10_000,
            currency: "xor".into(),
            max_future_activation_secs: 600,
            min_signatures: 2,
            signers: keys
                .iter()
                .enumerate()
                .map(|(index, key)| PricingTrustedSignerV1 {
                    version: PRICING_TRUSTED_SIGNER_VERSION_V1,
                    signer_id: format!("council-{}", index + 1),
                    public_key: key.verifying_key().to_bytes(),
                })
                .collect(),
            revoked_signer_ids: Vec::new(),
        }
    }
    fn manifest(effective_from_unix: u64) -> PricingManifestV1 {
        PricingManifestV1 {
            version: PRICING_MANIFEST_VERSION_V1,
            currency: "xor".into(),
            effective_from_unix,
            tiers: vec![
                PricingTierV1 {
                    tier_id: "hot".into(),
                    storage_price_per_gib_hour: xor("0.5"),
                    egress_price_per_gib: xor("0.05"),
                    min_collateral_ratio_bps: Some(15_000),
                    notes: None,
                },
                PricingTierV1 {
                    tier_id: "warm".into(),
                    storage_price_per_gib_hour: xor("0.2"),
                    egress_price_per_gib: xor("0.02"),
                    min_collateral_ratio_bps: None,
                    notes: None,
                },
            ],
            credit_policy: CreditPolicyV1 {
                settlement_window_secs: 86_400,
                auto_top_up_threshold_bps: 2_000,
            },
            bond_policy: BondPolicyV1 {
                collateral_ratio_bps: 30_000,
                new_provider_grace_days: 30,
            },
            micropayment_policy: Some(PricingMicropaymentPolicyV1 {
                payout_probability_bps: 100,
                max_voucher_value: xor("5"),
                notes: None,
            }),
        }
    }
    fn signed_manifest(
        policy: &PricingTrustPolicyV1,
        effective_from_unix: u64,
        previous_pricing_id: Option<[u8; 32]>,
        signer_indices: &[usize],
    ) -> GovernedPricingManifestV1 {
        let manifest = manifest(effective_from_unix);
        let mut governed = GovernedPricingManifestV1 {
            version: GOVERNED_PRICING_MANIFEST_VERSION_V1,
            policy_digest: policy.canonical_digest().expect("policy digest"),
            pricing_id: derive_pricing_id(&manifest, previous_pricing_id).expect("pricing id"),
            previous_pricing_id,
            manifest,
            signatures: Vec::new(),
        };
        let digest = governed.signing_digest().expect("signing digest");
        let keys = keys();
        governed.signatures = signer_indices
            .iter()
            .copied()
            .map(|index| PricingManifestSignatureV1 {
                version: PRICING_MANIFEST_SIGNATURE_VERSION_V1,
                signer_id: format!("council-{}", index + 1),
                signature: keys[index].sign(&digest).to_bytes(),
            })
            .collect();
        governed
    }
    #[test]
    fn threshold_governed_pricing_manifest_verifies() {
        let policy = policy();
        let governed = signed_manifest(&policy, ADMITTED_AT + 100, None, &[0, 1]);
        governed
            .verify(&policy, ADMITTED_AT)
            .expect("threshold governed pricing");
        assert_eq!(
            governed.pricing_id,
            derive_pricing_id(&governed.manifest, None).expect("pricing id replay")
        );
    }
    #[test]
    fn pricing_tamper_policy_currency_and_schedule_substitution_fail() {
        let policy = policy();
        let governed = signed_manifest(&policy, ADMITTED_AT + 100, None, &[0, 1]);
        let mut tampered = governed.clone();
        tampered.manifest.tiers[0].storage_price_per_gib_hour = tampered.manifest.tiers[0]
            .storage_price_per_gib_hour
            .checked_add(&xor("0.001"))
            .expect("tampered storage price remains representable");
        assert!(matches!(
            tampered.verify(&policy, ADMITTED_AT),
            Err(GovernedPricingError::PricingIdMismatch)
        ));
        let mut substituted = policy.clone();
        substituted.policy_id[0] ^= 1;
        assert!(matches!(
            governed.verify(&substituted, ADMITTED_AT),
            Err(GovernedPricingError::PolicyDigestMismatch)
        ));
        let mut other_currency = policy.clone();
        other_currency.currency = "usd".into();
        let mismatched = signed_manifest(&other_currency, ADMITTED_AT + 100, None, &[0, 1]);
        assert!(matches!(
            mismatched.verify(&other_currency, ADMITTED_AT),
            Err(GovernedPricingError::CurrencyMismatch { .. })
        ));
        let future = signed_manifest(
            &policy,
            ADMITTED_AT + policy.max_future_activation_secs + 1,
            None,
            &[0, 1],
        );
        assert!(matches!(
            future.verify(&policy, ADMITTED_AT),
            Err(GovernedPricingError::ActivationTooFarInFuture)
        ));
        let retroactive = signed_manifest(&policy, ADMITTED_AT - 1, None, &[0, 1]);
        assert!(matches!(
            retroactive.verify(&policy, ADMITTED_AT),
            Err(GovernedPricingError::ActivationBeforeAdmission)
        ));
    }
    #[test]
    fn quorum_duplicates_order_unknown_revoked_and_malformed_signatures_fail() {
        let policy = policy();
        let governed = signed_manifest(&policy, ADMITTED_AT, None, &[0, 1]);
        let below = signed_manifest(&policy, ADMITTED_AT, None, &[0]);
        assert!(matches!(
            below.verify(&policy, ADMITTED_AT),
            Err(GovernedPricingError::SignatureQuorumNotMet { .. })
        ));
        let mut duplicate = governed.clone();
        duplicate.signatures[1] = duplicate.signatures[0].clone();
        assert!(matches!(
            duplicate.validate_structure(),
            Err(GovernedPricingError::DuplicateManifestSigner { .. })
        ));
        let mut unsorted = governed.clone();
        unsorted.signatures.swap(0, 1);
        assert!(matches!(
            unsorted.validate_structure(),
            Err(GovernedPricingError::NonCanonicalOrder {
                field: "signatures"
            })
        ));
        let mut unknown = governed.clone();
        unknown.signatures[1].signer_id = "council-9".into();
        assert!(matches!(
            unknown.verify(&policy, ADMITTED_AT),
            Err(GovernedPricingError::UntrustedSigner { .. })
        ));
        let mut revoked_policy = policy.clone();
        revoked_policy.revoked_signer_ids = vec!["council-1".into()];
        revoked_policy.min_signatures = 1;
        let revoked = signed_manifest(&revoked_policy, ADMITTED_AT, None, &[0]);
        assert!(matches!(
            revoked.verify(&revoked_policy, ADMITTED_AT),
            Err(GovernedPricingError::RevokedSigner { .. })
        ));
        let mut malformed = governed;
        malformed.signatures[0].signature = [0; ed25519_dalek::SIGNATURE_LENGTH];
        assert!(matches!(
            malformed.validate_structure(),
            Err(GovernedPricingError::InvalidSignature { .. })
        ));
    }
    #[test]
    fn policy_rejects_weak_duplicate_keys_thresholds_and_revocations() {
        let mut weak = policy();
        weak.signers[0].public_key = [0; ed25519_dalek::PUBLIC_KEY_LENGTH];
        assert!(matches!(
            weak.validate(),
            Err(GovernedPricingError::InvalidPublicKey { .. })
        ));
        let mut duplicate_key = policy();
        duplicate_key.signers[1].public_key = duplicate_key.signers[0].public_key;
        assert!(matches!(
            duplicate_key.validate(),
            Err(GovernedPricingError::DuplicateTrustedPublicKey)
        ));
        let mut threshold = policy();
        threshold.min_signatures = 4;
        assert!(matches!(
            threshold.validate(),
            Err(GovernedPricingError::InvalidSignatureThreshold { .. })
        ));
        let mut unknown_revocation = policy();
        unknown_revocation.revoked_signer_ids = vec!["council-9".into()];
        assert!(matches!(
            unknown_revocation.validate(),
            Err(GovernedPricingError::UnknownRevokedSigner { .. })
        ));
    }
    #[test]
    fn pricing_transition_requires_exact_head_and_monotonic_activation() {
        let policy = policy();
        let first = signed_manifest(&policy, ADMITTED_AT, None, &[0, 1]);
        validate_governed_pricing_transition(None, &first).expect("initial pricing");
        let second = signed_manifest(&policy, ADMITTED_AT + 1, Some(first.pricing_id), &[0, 1]);
        validate_governed_pricing_transition(Some(&first), &second).expect("pricing successor");
        let wrong = signed_manifest(&policy, ADMITTED_AT + 2, Some([0x77; 32]), &[0, 1]);
        assert!(matches!(
            validate_governed_pricing_transition(Some(&first), &wrong),
            Err(GovernedPricingError::PreviousPricingMismatch)
        ));
        let stale = signed_manifest(&policy, ADMITTED_AT, Some(first.pricing_id), &[0, 1]);
        assert!(matches!(
            validate_governed_pricing_transition(Some(&first), &stale),
            Err(GovernedPricingError::ActivationDidNotAdvance)
        ));
        assert!(matches!(
            validate_governed_pricing_transition(None, &second),
            Err(GovernedPricingError::UnexpectedInitialPredecessor)
        ));
    }
    #[test]
    fn pricing_series_admits_exact_chain_and_activates_on_schedule() {
        let policy = policy();
        let first = signed_manifest(&policy, ADMITTED_AT + 100, None, &[0, 1]);
        let second = signed_manifest(&policy, ADMITTED_AT + 200, Some(first.pricing_id), &[0, 1]);
        let mut series = GovernedPricingSeriesV1::new(&policy).expect("empty series");
        series
            .admit(&policy, first.clone(), ADMITTED_AT)
            .expect("first admission");
        series
            .admit(&policy, second.clone(), ADMITTED_AT + 10)
            .expect("second admission");
        assert_eq!(
            series.head(&policy).expect("validated series head"),
            Some(&second)
        );
        assert_eq!(
            series
                .active_at(&policy, ADMITTED_AT + 99)
                .expect("pre-activation query"),
            None
        );
        assert_eq!(
            series
                .active_at(&policy, ADMITTED_AT + 100)
                .expect("first activation query"),
            Some(&first)
        );
        assert_eq!(
            series
                .active_at(&policy, ADMITTED_AT + 200)
                .expect("second activation query"),
            Some(&second)
        );
        assert!(matches!(
            series.active_at(&policy, 0),
            Err(GovernedPricingError::InvalidObservationTime)
        ));
        let checkpoint = series.canonical_bytes(&policy).expect("series checkpoint");
        assert_eq!(
            decode_governed_pricing_series(&checkpoint, &policy).expect("replay checkpoint"),
            series
        );
    }
    #[test]
    fn pricing_series_rejects_replay_branch_clock_rollback_and_policy_substitution_atomically() {
        let policy = policy();
        let first = signed_manifest(&policy, ADMITTED_AT + 100, None, &[0, 1]);
        let mut series = GovernedPricingSeriesV1::new(&policy).expect("empty series");
        series
            .admit(&policy, first.clone(), ADMITTED_AT)
            .expect("first admission");
        let baseline = series.clone();
        assert!(matches!(
            series.admit(&policy, first.clone(), ADMITTED_AT + 1),
            Err(GovernedPricingError::PreviousPricingMismatch)
        ));
        assert_eq!(series, baseline, "replay rejection must be atomic");
        let branch = signed_manifest(&policy, ADMITTED_AT + 200, Some([0x77; 32]), &[0, 1]);
        assert!(matches!(
            series.admit(&policy, branch, ADMITTED_AT + 1),
            Err(GovernedPricingError::PreviousPricingMismatch)
        ));
        assert_eq!(series, baseline, "branch rejection must be atomic");
        let successor =
            signed_manifest(&policy, ADMITTED_AT + 200, Some(first.pricing_id), &[0, 1]);
        assert!(matches!(
            series.admit(&policy, successor, ADMITTED_AT - 1),
            Err(GovernedPricingError::AdmissionTimeRollback { .. })
        ));
        assert_eq!(series, baseline, "clock rollback must be atomic");
        let mut substituted_policy = policy.clone();
        substituted_policy.policy_id[0] ^= 1;
        assert!(matches!(
            series.validate(&substituted_policy),
            Err(GovernedPricingError::SeriesPolicyDigestMismatch)
        ));
    }
    #[test]
    fn pricing_series_replay_rejects_reordered_and_tampered_checkpoint_state() {
        let policy = policy();
        let first = signed_manifest(&policy, ADMITTED_AT + 100, None, &[0, 1]);
        let second = signed_manifest(&policy, ADMITTED_AT + 200, Some(first.pricing_id), &[0, 1]);
        let mut series = GovernedPricingSeriesV1::new(&policy).expect("empty series");
        series
            .admit(&policy, first, ADMITTED_AT)
            .expect("first admission");
        series
            .admit(&policy, second, ADMITTED_AT + 10)
            .expect("second admission");
        let mut reordered = series.clone();
        reordered.admissions.swap(0, 1);
        assert!(reordered.validate(&policy).is_err());
        let mut clock_rollback = series.clone();
        clock_rollback.admissions[1].admitted_at_unix = ADMITTED_AT - 1;
        assert!(matches!(
            clock_rollback.validate(&policy),
            Err(GovernedPricingError::AdmissionTimeRollback { .. })
        ));
        let mut tampered = series;
        tampered.admissions[1].governed.manifest.tiers[0].egress_price_per_gib =
            tampered.admissions[1].governed.manifest.tiers[0]
                .egress_price_per_gib
                .checked_add(&xor("0.001"))
                .expect("tampered egress price remains representable");
        assert!(matches!(
            tampered.validate(&policy),
            Err(GovernedPricingError::PricingIdMismatch)
        ));
    }
    #[test]
    fn canonical_decoders_reject_trailing_compressed_and_oversized_inputs() {
        let policy = policy();
        let governed = signed_manifest(&policy, ADMITTED_AT, None, &[0, 1]);
        let policy_bytes = policy.canonical_bytes().expect("policy bytes");
        assert_eq!(
            decode_pricing_trust_policy(&policy_bytes).expect("decode policy"),
            policy
        );
        let governed_bytes = governed.canonical_bytes().expect("governed bytes");
        assert_eq!(
            decode_governed_pricing_manifest(&governed_bytes).expect("decode governed manifest"),
            governed
        );
        let mut trailing = policy_bytes;
        trailing.push(0);
        assert!(matches!(
            decode_pricing_trust_policy(&trailing),
            Err(GovernedPricingError::Decoding { .. })
        ));
        let compressed =
            norito::to_compressed_bytes(&policy, Some(norito::CompressionConfig::default()))
                .expect("compress policy");
        assert!(matches!(
            decode_pricing_trust_policy(&compressed),
            Err(GovernedPricingError::NonCanonicalEncoding { .. })
        ));
        let oversized = vec![0; MAX_PRICING_TRUST_POLICY_BYTES + 1];
        assert!(matches!(
            decode_pricing_trust_policy(&oversized),
            Err(GovernedPricingError::EncodedPayloadTooLarge { .. })
        ));
    }
}
