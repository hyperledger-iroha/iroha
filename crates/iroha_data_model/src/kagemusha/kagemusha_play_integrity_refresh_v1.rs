//! Periodic Play Integrity refresh is distinct from original credential issuance and money.
//!
//! A native owner reserves the exact Core-signed challenge under its current original key
//! and journal. The issuer verifies Google and enrolled-key possession once per retained
//! attempt. This module authenticates the resulting lease; it does not create a wallet,
//! consume a replay slot, select a clock, renew a financial epoch or authorize spending.

use super::*;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Exact refresh challenge domain, including NUL.
pub const KAGEMUSHA_PLAY_INTEGRITY_REFRESH_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:play-integrity-refresh-challenge\0";
/// Exact unsigned fixed refresh challenge body width.
pub const KAGEMUSHA_PLAY_INTEGRITY_REFRESH_CHALLENGE_BODY_BYTES_V1: usize = 450;
/// Fixed unsigned body plus original Core Ed signature; no old challenge fallback.
pub const KAGEMUSHA_PLAY_INTEGRITY_REFRESH_TRANSPORT_BYTES_V1: usize = 514;
/// Exact issuer lease signing domain, including NUL.
pub const KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:play-integrity-refresh-lease\0";
/// Exact issuer unsigned lease body width.
pub const KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_BODY_BYTES_V1: usize = 402;
/// Maximum complete canonical periodic Integrity lease original, including issuer admission
/// and actual platform possession. This names the existing enforced first-release bound.
pub const KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_MAX_BYTES_V1: usize = 4096;
const REQUEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:play-integrity-refresh-request\0";
const POSSESSION_DOMAIN: &[u8] = b"iroha:kagemusha:v1:play-integrity-refresh-possession\0";
const ORIGINAL_DOMAIN: &[u8] = b"iroha:kagemusha:v1:play-integrity-refresh-original\0";

/// Public refresh request selected and retained by an actual native credential owner.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaPlayIntegrityRefreshChallengeV1")]
pub struct KagemushaPlayIntegrityRefreshChallengeV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Exact model digest of the complete unchanged original credential.
    pub credential_digest: [u8; 32],
    /// Actual enrolled SEC1 SHA-256.
    pub attested_key_id: [u8; 32],
    /// Independently selected original wallet account binding.
    pub account_binding: [u8; 32],
    /// Original authenticated network.
    pub network_id: [u8; 32],
    /// Original authenticated financial lane.
    pub lane_id: [u8; 32],
    /// Actual same current admitting release.
    pub release_id: [u8; 32],
    /// Actual same enrolled profile.
    pub hardware_profile_id: [u8; 32],
    /// Actual same qualified suite.
    pub suite_id: [u8; 32],
    /// Actual original ordinary trust-policy digest.
    pub trust_policy_digest: [u8; 32],
    /// Actual app-authority policy digest.
    pub app_authority_policy_digest: [u8; 32],
    /// Actual separately selected Google policy original SHA.
    pub play_integrity_policy_digest: [u8; 32],
    /// Fresh native nonce durably reserved before exposure.
    pub nonce: [u8; 32],
    /// Full original initial enrollment challenge digest.
    pub original_enrollment_challenge_digest: [u8; 32],
    /// Actual signed policy epoch, never renewed by refresh.
    pub policy_epoch: u64,
    /// Actual signed financial epoch, never renewed by refresh.
    pub hardware_epoch: u64,
    /// Trusted native inclusive issue time.
    pub issued_at_ms: u64,
    /// Trusted native exclusive challenge expiry, at most 120 seconds.
    pub expires_at_ms: u64,
}
impl KagemushaPlayIntegrityRefreshChallengeV1 {
    fn body(&self) -> Result<Vec<u8>, String> {
        let fields = [
            self.credential_digest,
            self.attested_key_id,
            self.account_binding,
            self.network_id,
            self.lane_id,
            self.release_id,
            self.hardware_profile_id,
            self.suite_id,
            self.trust_policy_digest,
            self.app_authority_policy_digest,
            self.play_integrity_policy_digest,
            self.nonce,
            self.original_enrollment_challenge_digest,
        ];
        if self.version != 1
            || fields.contains(&[0; 32])
            || self.policy_epoch == 0
            || self.hardware_epoch == 0
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms > 120_000
        {
            return Err("Integrity refresh challenge shape differs".into());
        }
        let mut bytes = self.version.to_le_bytes().to_vec();
        for field in fields {
            bytes.extend_from_slice(&field);
        }
        for value in [
            self.policy_epoch,
            self.hardware_epoch,
            self.issued_at_ms,
            self.expires_at_ms,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        Ok(bytes)
    }
    /// Exact domain + LE64(450) + unsigned body; no Norito or phone-time reconstruction.
    /// # Errors
    /// Rejects absent selectors or invalid original interval.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        Ok(message(
            KAGEMUSHA_PLAY_INTEGRITY_REFRESH_CHALLENGE_DOMAIN_V1,
            &self.body()?,
        ))
    }
    /// Attempt ID is the SHA of the exact full Core signing message.
    /// # Errors
    /// Rejects invalid challenge shape.
    pub fn attempt_id(&self) -> Result<[u8; 32], String> {
        Ok(Sha256::digest(self.canonical_signing_bytes()?).into())
    }
    /// Google's independent requestHash binds this exact attempt and enrolled key.
    /// # Errors
    /// Rejects invalid challenge shape.
    pub fn request_hash(&self) -> Result<[u8; 32], String> {
        let mut hash = Sha256::new();
        hash.update(REQUEST_DOMAIN);
        hash.update(self.canonical_signing_bytes()?);
        hash.update(self.attested_key_id);
        Ok(hash.finalize().into())
    }
    /// Actual enrolled Android key signs this separate refresh possession message.
    /// # Errors
    /// Rejects invalid challenge shape.
    pub fn possession_signing_bytes(&self) -> Result<Vec<u8>, String> {
        Ok(message(POSSESSION_DOMAIN, &self.canonical_signing_bytes()?))
    }
    fn select(
        &self,
        credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
        release: &KagemushaAuthenticatedReleaseV1,
        trust: &KagemushaOrdinaryAppTrustPolicyV1,
        authority: &KagemushaAppAttestationAuthorityPolicyV1,
        now: u64,
    ) -> Result<(), String> {
        self.body()?;
        let c = credential.subject();
        let profile = release
            .enabled_profile(c.hardware_profile_id)
            .ok_or("Integrity enrolled profile absent")?;
        trust.validate_for_profile(&profile.hardware_profile, authority)?;
        let policy = trust
            .play_integrity_policy
            .ok_or("Integrity refresh is not governed")?;
        if c.platform_class != KagemushaHardwarePlatformClassV1::AndroidKeyMint
            || c.play_integrity.is_none()
            || self.credential_digest != credential.digest()
            || self.attested_key_id != c.attested_key_id
            || self.account_binding != c.account_binding
            || self.network_id != c.network_id
            || self.network_id != *release.network_id().as_bytes()
            || self.lane_id != c.lane_id
            || self.release_id != c.release_id
            || self.release_id != release.release_id()
            || self.hardware_profile_id != c.hardware_profile_id
            || self.suite_id != c.suite_id
            || self.suite_id != profile.suite_id
            || self.trust_policy_digest != c.trust_policy_digest
            || self.trust_policy_digest != trust.canonical_digest()?
            || self.app_authority_policy_digest != c.app_authority_policy_digest
            || self.app_authority_policy_digest != authority.canonical_digest()?
            || self.play_integrity_policy_digest != policy.policy_digest
            || self.policy_epoch != c.policy_epoch
            || self.policy_epoch != profile.policy_epoch
            || self.hardware_epoch != c.hardware_epoch
            || self.original_enrollment_challenge_digest != c.enrollment_challenge_digest
            || self.issued_at_ms < c.issued_at_ms
            || self.expires_at_ms > c.expires_at_ms
            || now < self.issued_at_ms
            || now >= self.expires_at_ms
        {
            return Err(
                "Integrity refresh does not select the original credential/current policy".into(),
            );
        }
        Ok(())
    }
}

/// Original Core signature over the retained refresh challenge.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::KagemushaSignedPlayIntegrityRefreshChallengeV1"
)]
pub struct KagemushaSignedPlayIntegrityRefreshChallengeV1 {
    /// Complete original native-selected challenge.
    pub challenge: KagemushaPlayIntegrityRefreshChallengeV1,
    /// Original FI/Core Ed signature, independent of app-authority Ed.
    pub signature: Signature,
}
impl KagemushaSignedPlayIntegrityRefreshChallengeV1 {
    /// Verify the original Core signature and the independently selected credential/policy.
    /// Native callers separately retain the nonce, current owner and exact original attempt.
    /// # Errors
    /// Rejects a substituted original, scope, epoch, key or trusted interval.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate(
        &self,
        expected: &KagemushaPlayIntegrityRefreshChallengeV1,
        credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
        release: &KagemushaAuthenticatedReleaseV1,
        trust: &KagemushaOrdinaryAppTrustPolicyV1,
        authority: &KagemushaAppAttestationAuthorityPolicyV1,
        core_key: &PublicKey,
        now: u64,
    ) -> Result<(), String> {
        if self.challenge != *expected || core_key.algorithm() != Algorithm::Ed25519 {
            return Err("Integrity refresh original/Core key differs".into());
        }
        self.challenge
            .select(credential, release, trust, authority, now)?;
        self.signature
            .verify(core_key, &self.challenge.canonical_signing_bytes()?)
            .map_err(|_| "Integrity refresh Core signature rejected".to_owned())
    }
    /// Exact fixed 514-byte transport; encoding grants no native ownership.
    /// # Errors
    /// Rejects malformed challenge or signature width.
    pub fn to_transport_bytes(&self) -> Result<Vec<u8>, String> {
        let mut body = self.challenge.body()?;
        if self.signature.payload().len() != 64 {
            return Err("Integrity Core signature width differs".into());
        }
        body.extend_from_slice(self.signature.payload());
        Ok(body)
    }
    /// Decode the sole exact fixed transport before independent Core-key authentication.
    /// # Errors
    /// Rejects another length, shape or trailing original.
    pub fn from_transport_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 514 {
            return Err("Integrity refresh transport width differs".into());
        }
        let mut reader = Reader {
            bytes: &bytes[..450],
            offset: 0,
        };
        let challenge = KagemushaPlayIntegrityRefreshChallengeV1 {
            version: u16::from_le_bytes(reader.take()?),
            credential_digest: reader.take()?,
            attested_key_id: reader.take()?,
            account_binding: reader.take()?,
            network_id: reader.take()?,
            lane_id: reader.take()?,
            release_id: reader.take()?,
            hardware_profile_id: reader.take()?,
            suite_id: reader.take()?,
            trust_policy_digest: reader.take()?,
            app_authority_policy_digest: reader.take()?,
            play_integrity_policy_digest: reader.take()?,
            nonce: reader.take()?,
            original_enrollment_challenge_digest: reader.take()?,
            policy_epoch: u64::from_le_bytes(reader.take()?),
            hardware_epoch: u64::from_le_bytes(reader.take()?),
            issued_at_ms: u64::from_le_bytes(reader.take()?),
            expires_at_ms: u64::from_le_bytes(reader.take()?),
        };
        let value = Self {
            challenge,
            signature: Signature::from_bytes(&bytes[450..]),
        };
        if value.to_transport_bytes()? != bytes {
            return Err("Integrity original transport differs".into());
        }
        Ok(value)
    }
}

/// Separately signed current Google verification lease; the enrolled credential is unchanged.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaPlayIntegrityRefreshLeaseSubjectV1")]
pub struct KagemushaPlayIntegrityRefreshLeaseSubjectV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Complete unchanged original credential digest.
    pub credential_digest: [u8; 32],
    /// SHA of the original complete Core refresh signing message.
    pub challenge_digest: [u8; 32],
    /// Actual enrolled SEC1 SHA.
    pub attested_key_id: [u8; 32],
    /// Actual unchanged release.
    pub release_id: [u8; 32],
    /// Actual unchanged profile.
    pub hardware_profile_id: [u8; 32],
    /// Exact original trust-policy digest.
    pub trust_policy_digest: [u8; 32],
    /// Exact original app-authority policy digest.
    pub app_authority_policy_digest: [u8; 32],
    /// Original independent Google verdict binding for this refresh.
    pub binding: KagemushaPlayIntegrityBindingV1,
    /// SHA of the exact original Android DER possession proof.
    pub possession_original_digest: [u8; 32],
    /// Original native policy epoch.
    pub policy_epoch: u64,
    /// Original native financial epoch.
    pub hardware_epoch: u64,
    /// Inclusive trusted issuer lease time.
    pub issued_at_ms: u64,
    /// Exclusive lease expiry, bounded by refresh policy and credential lifetime.
    pub expires_at_ms: u64,
}
impl KagemushaPlayIntegrityRefreshLeaseSubjectV1 {
    /// Parse the sole exact fixed issuer body before any app-authority signing.
    /// # Errors
    /// Rejects another length, malformed interval or noncanonical fixed body.
    pub fn from_signing_body(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 402 {
            return Err("Integrity lease body width differs".into());
        }
        let mut r = Reader { bytes, offset: 0 };
        let version = u16::from_le_bytes(r.take()?);
        let credential_digest = r.take()?;
        let challenge_digest = r.take()?;
        let attested_key_id = r.take()?;
        let release_id = r.take()?;
        let hardware_profile_id = r.take()?;
        let trust_policy_digest = r.take()?;
        let app_authority_policy_digest = r.take()?;
        let request_hash = r.take()?;
        let evidence_digest = r.take()?;
        let policy_digest = r.take()?;
        let possession_original_digest = r.take()?;
        let policy_epoch = u64::from_le_bytes(r.take()?);
        let hardware_epoch = u64::from_le_bytes(r.take()?);
        let verified_at_ms = u64::from_le_bytes(r.take()?);
        let refresh_before_ms = u64::from_le_bytes(r.take()?);
        let issued_at_ms = u64::from_le_bytes(r.take()?);
        let expires_at_ms = u64::from_le_bytes(r.take()?);
        let s = Self {
            version,
            credential_digest,
            challenge_digest,
            attested_key_id,
            release_id,
            hardware_profile_id,
            trust_policy_digest,
            app_authority_policy_digest,
            binding: KagemushaPlayIntegrityBindingV1 {
                request_hash,
                evidence_digest,
                policy_digest,
                verified_at_ms,
                refresh_before_ms,
            },
            possession_original_digest,
            policy_epoch,
            hardware_epoch,
            issued_at_ms,
            expires_at_ms,
        };
        let message = s.canonical_signing_bytes()?;
        if message.get(KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_DOMAIN_V1.len() + 8..) != Some(bytes)
        {
            return Err("Integrity lease body roundtrip differs".into());
        }
        Ok(s)
    }
    /// Exact domain + LE64(402) + eleven raw32 + six LE64 after LE16 version.
    /// # Errors
    /// Rejects another version, missing originals or invalid interval.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        let fields = [
            self.credential_digest,
            self.challenge_digest,
            self.attested_key_id,
            self.release_id,
            self.hardware_profile_id,
            self.trust_policy_digest,
            self.app_authority_policy_digest,
            self.binding.request_hash,
            self.binding.evidence_digest,
            self.binding.policy_digest,
            self.possession_original_digest,
        ];
        if self.version != 1
            || fields.contains(&[0; 32])
            || self.policy_epoch == 0
            || self.hardware_epoch == 0
            || self.binding.verified_at_ms == 0
            || self.binding.refresh_before_ms <= self.binding.verified_at_ms
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
        {
            return Err("Integrity refresh lease shape differs".into());
        }
        let mut body = self.version.to_le_bytes().to_vec();
        for field in fields {
            body.extend_from_slice(&field);
        }
        for value in [
            self.policy_epoch,
            self.hardware_epoch,
            self.binding.verified_at_ms,
            self.binding.refresh_before_ms,
            self.issued_at_ms,
            self.expires_at_ms,
        ] {
            body.extend_from_slice(&value.to_le_bytes());
        }
        Ok(message(
            KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_DOMAIN_V1,
            &body,
        ))
    }
}

/// Full original periodic lease and enrolled-key proof, independent of monetary authority.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaPlayIntegrityRefreshLeaseV1")]
pub struct KagemushaPlayIntegrityRefreshLeaseV1 {
    /// Original fixed issuer subject.
    pub subject: KagemushaPlayIntegrityRefreshLeaseSubjectV1,
    /// Genuine governed app-authority Ed signature over the exact lease signing message.
    pub signature: Signature,
    /// Full exact Android possession DER, retained independently of Google evidence.
    pub app_possession: KagemushaAppOperationApprovalEvidenceV1,
    /// Mandatory governed P256 admission over the complete canonical Ed-only lease original.
    pub circuit_admission: super::KagemushaOrdinaryIssuerCircuitAdmissionV1,
}
#[derive(Clone, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::KagemushaPlayIntegrityRefreshLeaseEdOriginalV1"
)]
struct LeaseEdOriginal {
    subject: KagemushaPlayIntegrityRefreshLeaseSubjectV1,
    signature: Signature,
    app_possession: KagemushaAppOperationApprovalEvidenceV1,
}
/// Genuine current periodic lease; no constructor, decoder or financial capability.
pub struct KagemushaVerifiedPlayIntegrityRefreshLeaseV1 {
    subject: KagemushaPlayIntegrityRefreshLeaseSubjectV1,
    original: Vec<u8>,
    digest: [u8; 32],
    authenticated_at_ms: u64,
    circuit_admission: super::KagemushaVerifiedOrdinaryIssuerCircuitAdmissionV1,
}
impl KagemushaVerifiedPlayIntegrityRefreshLeaseV1 {
    /// Exact retained native admission instant, distinct from the issuer's signed issue time.
    #[must_use]
    pub const fn authenticated_at_ms(&self) -> u64 {
        self.authenticated_at_ms
    }

    /// Borrow the genuine independent governed P256 issuer admission.
    #[must_use]
    pub const fn circuit_admission(
        &self,
    ) -> &super::KagemushaVerifiedOrdinaryIssuerCircuitAdmissionV1 {
        &self.circuit_admission
    }

    /// Borrow the exact verified original issuer subject.
    #[must_use]
    pub const fn subject(&self) -> &KagemushaPlayIntegrityRefreshLeaseSubjectV1 {
        &self.subject
    }
    /// Borrow the complete canonical signed original with platform possession.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Exact model-domain digest of this original lease.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Recheck exact original current time without renewing its nonce or expiry.
    /// # Errors
    /// Rejects time regression or expired lease/verdict.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<(), String> {
        if now < self.authenticated_at_ms
            || now < self.subject.issued_at_ms
            || now >= self.subject.expires_at_ms
            || now >= self.subject.binding.refresh_before_ms
        {
            return Err("Integrity refresh lease expired or time regressed".into());
        }
        Ok(())
    }
}
impl KagemushaPlayIntegrityRefreshLeaseV1 {
    /// Sole complete canonical Ed-only lease original, including original platform possession.
    /// # Errors
    /// Rejects malformed original fields or another Ed signature width.
    pub fn ed_only_bytes_for(
        subject: &KagemushaPlayIntegrityRefreshLeaseSubjectV1,
        signature: &Signature,
        app_possession: &KagemushaAppOperationApprovalEvidenceV1,
    ) -> Result<Vec<u8>, String> {
        subject.canonical_signing_bytes()?;
        if signature.payload().len() != 64 {
            return Err("Integrity Ed signature width differs".into());
        }
        match app_possession {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der }
                if (8..=72).contains(&signature_der.len()) => {}
            _ => return Err("Integrity Ed original possession shape differs".into()),
        }
        norito::encode_canonical(&LeaseEdOriginal {
            subject: *subject,
            signature: signature.clone(),
            app_possession: app_possession.clone(),
        })
        .map_err(|e| e.to_string())
    }
    /// Data-only Ed-original encoding, excluding its P256 admission.
    /// # Errors
    /// Rejects malformed original fields.
    pub fn ed_only_canonical_bytes(&self) -> Result<Vec<u8>, String> {
        Self::ed_only_bytes_for(&self.subject, &self.signature, &self.app_possession)
    }
    /// Exact expected issuer admission subject, never a signer or current authority.
    /// # Errors
    /// Rejects malformed original fields.
    pub fn circuit_admission_subject_for(
        subject: &KagemushaPlayIntegrityRefreshLeaseSubjectV1,
        signature: &Signature,
        app_possession: &KagemushaAppOperationApprovalEvidenceV1,
    ) -> Result<super::KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1, String> {
        Ok(super::KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1 {
            version: 1,
            purpose: 2,
            release_id: subject.release_id,
            hardware_profile_id: subject.hardware_profile_id,
            ed_original_sha256: Sha256::digest(Self::ed_only_bytes_for(
                subject,
                signature,
                app_possession,
            )?)
            .into(),
        })
    }

    /// Canonical original bytes only; no verification or current capability is returned.
    /// # Errors
    /// Rejects malformed, missing or oversized original fields.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.subject.canonical_signing_bytes()?;
        if self.signature.payload().len() != 64 {
            return Err("Integrity issuer signature width differs".into());
        }
        if self.circuit_admission.subject
            != Self::circuit_admission_subject_for(
                &self.subject,
                &self.signature,
                &self.app_possession,
            )?
        {
            return Err("Integrity issuer admission original differs".into());
        }
        self.circuit_admission.to_transport_bytes()?;
        let original = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if original.len() > KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_MAX_BYTES_V1 {
            return Err("Integrity lease original oversized".into());
        }
        Ok(original)
    }
    /// Digest the complete canonical original with the same domain as native verified leases.
    /// This data identity does not verify signatures, current policy or lease ownership.
    /// # Errors
    /// Rejects malformed or oversized original fields.
    pub fn canonical_digest(&self) -> Result<[u8; 32], String> {
        Ok(digest_original(&self.canonical_bytes()?))
    }
    /// Authenticate real Core/issuer/platform signatures against actual selected original custody.
    /// The native caller still owns freshness, original nonce reservation and replay consumption.
    /// # Errors
    /// Rejects any key, policy, epoch, original challenge, Google binding or lifetime substitution.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate(
        &self,
        credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
        release: &KagemushaAuthenticatedReleaseV1,
        trust: &KagemushaOrdinaryAppTrustPolicyV1,
        authority: &KagemushaAppAttestationAuthorityPolicyV1,
        expected: &KagemushaSignedPlayIntegrityRefreshChallengeV1,
        core_key: &PublicKey,
        now: u64,
    ) -> Result<KagemushaVerifiedPlayIntegrityRefreshLeaseV1, String> {
        let c = &expected.challenge;
        // The short challenge authorizes issuance. A retained, valid issuer lease remains
        // independently verifiable after that challenge expires, without issuing it again.
        c.select(
            credential,
            release,
            trust,
            authority,
            self.subject.issued_at_ms,
        )?;
        if core_key.algorithm() != Algorithm::Ed25519 {
            return Err("Integrity preparation Core key is not Ed".into());
        }
        expected
            .signature
            .verify(core_key, &c.canonical_signing_bytes()?)
            .map_err(|_| "Integrity preparation Core signature rejected")?;
        let s = &self.subject;
        let original = self.canonical_bytes()?;
        let policy = trust
            .play_integrity_policy
            .ok_or("Integrity policy absent")?;
        let der = match &self.app_possession {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                signature_der
            }
            _ => return Err("Integrity refresh is Android only".into()),
        };
        if s.credential_digest != c.credential_digest
            || s.challenge_digest != c.attempt_id()?
            || s.attested_key_id != c.attested_key_id
            || s.release_id != c.release_id
            || s.hardware_profile_id != c.hardware_profile_id
            || s.trust_policy_digest != c.trust_policy_digest
            || s.app_authority_policy_digest != c.app_authority_policy_digest
            || s.binding.request_hash != c.request_hash()?
            || s.binding.policy_digest != c.play_integrity_policy_digest
            || s.possession_original_digest != <[u8; 32]>::from(Sha256::digest(der))
            || s.policy_epoch != c.policy_epoch
            || s.hardware_epoch != c.hardware_epoch
            || s.binding.verified_at_ms < c.issued_at_ms
            || s.binding.verified_at_ms > s.issued_at_ms
            || s.issued_at_ms - s.binding.verified_at_ms > policy.maximum_evidence_age_ms
            || s.binding.refresh_before_ms - s.binding.verified_at_ms
                > policy.maximum_refresh_interval_ms
            || s.issued_at_ms < c.issued_at_ms
            || s.issued_at_ms >= c.expires_at_ms
            || s.expires_at_ms > s.binding.refresh_before_ms
            || s.expires_at_ms > credential.subject().expires_at_ms
            || now < s.issued_at_ms
            || now >= s.expires_at_ms
        {
            return Err("Integrity refresh lease differs from exact original attempt".into());
        }
        self.app_possession.authenticate_signature(
            KagemushaHardwarePlatformClassV1::AndroidKeyMint,
            &credential.subject().app_public_key,
            credential.subject().app_signing_identity_digest,
            credential.subject().app_release_digest,
            None,
            &c.possession_signing_bytes()?,
        )?;
        self.signature
            .verify(&authority.authority_key, &s.canonical_signing_bytes()?)
            .map_err(|_| "Integrity lease issuer signature rejected")?;
        let digest = digest_original(&original);
        let circuit_admission = self.circuit_admission.authenticate(
            &Self::circuit_admission_subject_for(s, &self.signature, &self.app_possession)?,
            release,
        )?;
        let token = KagemushaVerifiedPlayIntegrityRefreshLeaseV1 {
            circuit_admission,
            subject: *s,
            original,
            digest,
            authenticated_at_ms: now,
        };
        token.recheck_at_trusted_time(now)?;
        Ok(token)
    }
}
fn digest_original(original: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(ORIGINAL_DOMAIN);
    hash.update((original.len() as u64).to_le_bytes());
    hash.update(original);
    hash.finalize().into()
}
fn message(domain: &[u8], body: &[u8]) -> Vec<u8> {
    let mut message = domain.to_vec();
    message.extend_from_slice(&(body.len() as u64).to_le_bytes());
    message.extend_from_slice(body);
    message
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or("Integrity body overflow")?;
        let v = self
            .bytes
            .get(self.offset..end)
            .ok_or("Integrity body truncated")?
            .try_into()
            .map_err(|_| "Integrity field width differs")?;
        self.offset = end;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;
    use iroha_crypto::{Algorithm, KeyPair};
    use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

    fn lease(
        f: &KagemushaOrdinaryRetailEnrollmentFixtureV1,
        credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    ) -> (
        KagemushaSignedPlayIntegrityRefreshChallengeV1,
        KagemushaPlayIntegrityRefreshLeaseV1,
        KeyPair,
    ) {
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let s = credential.subject();
        let c = KagemushaPlayIntegrityRefreshChallengeV1 {
            version: 1,
            credential_digest: credential.digest(),
            attested_key_id: s.attested_key_id,
            account_binding: s.account_binding,
            network_id: s.network_id,
            lane_id: s.lane_id,
            release_id: s.release_id,
            hardware_profile_id: s.hardware_profile_id,
            suite_id: s.suite_id,
            trust_policy_digest: s.trust_policy_digest,
            app_authority_policy_digest: s.app_authority_policy_digest,
            play_integrity_policy_digest: f.trust.play_integrity_policy.unwrap().policy_digest,
            nonce: [59; 32],
            original_enrollment_challenge_digest: s.enrollment_challenge_digest,
            policy_epoch: s.policy_epoch,
            hardware_epoch: s.hardware_epoch,
            issued_at_ms: 1300,
            expires_at_ms: 2000,
        };
        let signed = KagemushaSignedPlayIntegrityRefreshChallengeV1 {
            signature: Signature::new(issuer.private_key(), &c.canonical_signing_bytes().unwrap()),
            challenge: c,
        };
        let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let signature: P256Signature = key.sign(&c.possession_signing_bytes().unwrap());
        let der = signature.to_der().as_bytes().to_vec();
        let subject = KagemushaPlayIntegrityRefreshLeaseSubjectV1 {
            version: 1,
            credential_digest: signed.challenge.credential_digest,
            challenge_digest: c.attempt_id().unwrap(),
            attested_key_id: c.attested_key_id,
            release_id: c.release_id,
            hardware_profile_id: c.hardware_profile_id,
            trust_policy_digest: c.trust_policy_digest,
            app_authority_policy_digest: c.app_authority_policy_digest,
            binding: KagemushaPlayIntegrityBindingV1 {
                request_hash: c.request_hash().unwrap(),
                evidence_digest: [60; 32],
                policy_digest: c.play_integrity_policy_digest,
                verified_at_ms: 1400,
                refresh_before_ms: 2400,
            },
            possession_original_digest: Sha256::digest(&der).into(),
            policy_epoch: c.policy_epoch,
            hardware_epoch: c.hardware_epoch,
            issued_at_ms: 1400,
            expires_at_ms: 2400,
        };
        let signature = Signature::new(
            issuer.private_key(),
            &subject.canonical_signing_bytes().unwrap(),
        );
        let app_possession =
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der: der };
        let circuit_admission =
            crate::testing::ordinary_app_enrollment::ordinary_test_issuer_admission_v1(
                KagemushaPlayIntegrityRefreshLeaseV1::circuit_admission_subject_for(
                    &subject,
                    &signature,
                    &app_possession,
                )
                .unwrap(),
            );
        let lease = KagemushaPlayIntegrityRefreshLeaseV1 {
            subject,
            signature,
            app_possession,
            circuit_admission,
        };

        (signed, lease, issuer)
    }
    #[test]
    fn integrity_canonical_stream_segments_match_sole_encoder_at_every_der_width() {
        let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity();
        let admitted = f.verify(300).unwrap();
        let (_, original, _) = lease(&f, admitted.app_credential());
        for complete in [false, true] {
            let grammar = if complete {
                original.original_canonical_stream_grammar().unwrap()
            } else {
                original.ed_only_canonical_stream_grammar().unwrap()
            };
            assert_eq!(grammar.lengths.len(), 65);
            for variant in grammar.lengths {
                let mut template = original.clone();
                let der: Vec<u8> = (0..variant.der_length)
                    .map(|i| u8::try_from(i).unwrap().wrapping_mul(17))
                    .collect();
                template.app_possession =
                    KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                        signature_der: der.clone(),
                    };
                template.circuit_admission.subject =
                    KagemushaPlayIntegrityRefreshLeaseV1::circuit_admission_subject_for(
                        &template.subject,
                        &template.signature,
                        &template.app_possession,
                    )
                    .unwrap();
                let expected = if complete {
                    let raw = template.canonical_bytes().unwrap();
                    let mut bytes = ORIGINAL_DOMAIN.to_vec();
                    bytes.extend_from_slice(&(raw.len() as u64).to_le_bytes());
                    bytes.extend_from_slice(&raw);
                    bytes
                } else {
                    template.ed_only_canonical_bytes().unwrap()
                };
                let mut stream = variant.prefix.clone();
                for byte in &der[..der.len() - 1] {
                    let mut unit = grammar.repeated_der_byte_unit.clone();
                    unit[0] = Some(*byte);
                    stream.extend(unit);
                }
                stream.push(Some(*der.last().unwrap()));
                stream.extend(variant.suffix);
                assert_eq!(stream.len(), expected.len());
                for (i, byte) in stream.iter_mut().enumerate() {
                    if byte.is_none() {
                        *byte = Some(expected[i]);
                    }
                }
                assert_eq!(
                    stream.into_iter().map(Option::unwrap).collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(variant.layout.possession_signature_bytes.len(), der.len());
                for (position, byte) in variant.layout.possession_signature_bytes.iter().zip(&der) {
                    assert_eq!(expected[*position], *byte);
                }
            }
        }
    }
    #[test]
    fn integrity_issuer_admission_layout_binds_exact_ed_and_platform_originals() {
        let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity();
        let enrolled = f.verify(300).unwrap();
        let (c, mut lease, issuer) = lease(&f, enrolled.app_credential());
        let token = lease
            .authenticate(
                enrolled.app_credential(),
                &f.release,
                &f.trust,
                &f.app_authority,
                &c,
                issuer.public_key(),
                1500,
            )
            .unwrap();
        assert_eq!(lease.canonical_digest().unwrap(), token.digest());
        assert_eq!(
            token.circuit_admission().public_key(),
            &f.release
                .enabled_profile(lease.subject.hardware_profile_id)
                .unwrap()
                .hardware_profile
                .governance_credential_public_key
        );
        let ed = lease.ed_only_canonical_bytes().unwrap();
        let layout = lease.ed_only_preimage_layout().unwrap();
        assert_eq!(
            lease.circuit_admission.subject.ed_original_sha256,
            <[u8; 32]>::from(Sha256::digest(&ed))
        );
        for (i, pinned) in layout.bytes.iter().enumerate() {
            if let Some(value) = pinned {
                assert_eq!(ed[i], *value);
            }
        }
        let der = match &lease.app_possession {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                signature_der
            }
            _ => unreachable!(),
        };
        for (i, p) in layout.possession_signature_bytes.iter().enumerate() {
            assert_eq!(ed[*p], der[i]);
            assert_eq!(layout.bytes[*p], None);
        }
        let full = lease.original_preimage_layout().unwrap();
        let raw = lease.canonical_bytes().unwrap();
        let prelude = ORIGINAL_DOMAIN.len() + 8;
        for (i, p) in full
            .issuer_admission_layout
            .unwrap()
            .signature_bytes
            .iter()
            .enumerate()
        {
            assert_eq!(
                raw[*p - prelude],
                lease.circuit_admission.signature.as_raw_bytes()[i]
            );
            assert_eq!(full.bytes[*p], None);
        }
        assert!(norito::decode_from_bytes::<KagemushaPlayIntegrityRefreshLeaseV1>(&ed).is_err());
        lease.subject.binding.evidence_digest = [88; 32];
        lease.signature = Signature::new(
            issuer.private_key(),
            &lease.subject.canonical_signing_bytes().unwrap(),
        );
        assert!(
            lease
                .authenticate(
                    enrolled.app_credential(),
                    &f.release,
                    &f.trust,
                    &f.app_authority,
                    &c,
                    issuer.public_key(),
                    1500
                )
                .is_err()
        );
    }
    #[test]
    fn ordinary_integrity_periodic_refresh_after_initial_deadline_preserves_credential() {
        let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity();
        let enrolled = f.verify(300).unwrap();
        let credential = enrolled.app_credential();
        let original = credential.original().to_vec();
        let (c, lease, issuer) = lease(&f, credential);
        assert!(credential.recheck_at_trusted_time(1500).is_err());
        let verified = lease
            .authenticate(
                credential,
                &f.release,
                &f.trust,
                &f.app_authority,
                &c,
                issuer.public_key(),
                1500,
            )
            .unwrap();
        credential
            .recheck_with_integrity_lease(&verified, 1500)
            .unwrap();
        assert!(enrolled.recheck_at_trusted_time(1500).is_err());
        enrolled
            .recheck_with_integrity_lease(&verified, 1500)
            .unwrap();
        assert!(
            enrolled
                .recheck_with_integrity_lease(&verified, 299)
                .is_err()
        );
        assert!(
            enrolled
                .recheck_with_integrity_lease(&verified, 9000)
                .is_err()
        );
        let restored_after_challenge = lease
            .authenticate(
                credential,
                &f.release,
                &f.trust,
                &f.app_authority,
                &c,
                issuer.public_key(),
                2200,
            )
            .unwrap();
        credential
            .recheck_with_integrity_lease(&restored_after_challenge, 2200)
            .unwrap();
        assert_eq!(credential.original(), original);
        assert!(
            credential
                .recheck_with_integrity_lease(&verified, 2400)
                .is_err()
        );
        assert!(verified.recheck_at_trusted_time(1499).is_err());
        let transport = c.to_transport_bytes().unwrap();
        assert_eq!(transport.len(), 514);
        assert_eq!(
            KagemushaSignedPlayIntegrityRefreshChallengeV1::from_transport_bytes(&transport)
                .unwrap(),
            c
        );
        let body = lease.subject.canonical_signing_bytes().unwrap();
        let start = KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_DOMAIN_V1.len() + 8;
        assert_eq!(
            KagemushaPlayIntegrityRefreshLeaseSubjectV1::from_signing_body(&body[start..]).unwrap(),
            lease.subject
        );
    }
    #[test]
    fn ordinary_integrity_refresh_rejects_resigned_epoch_key_policy_and_other_original() {
        let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity();
        let enrolled = f.verify(300).unwrap();
        let credential = enrolled.app_credential();
        let (c, lease, issuer) = lease(&f, credential);
        for field in 0..5 {
            let mut changed = c.clone();
            match field {
                0 => changed.challenge.hardware_epoch += 1,
                1 => changed.challenge.attested_key_id[0] ^= 1,
                2 => changed.challenge.trust_policy_digest[0] ^= 1,
                3 => changed.challenge.credential_digest[0] ^= 1,
                _ => changed.challenge.original_enrollment_challenge_digest[0] ^= 1,
            }
            changed.signature = Signature::new(
                issuer.private_key(),
                &changed.challenge.canonical_signing_bytes().unwrap(),
            );
            assert!(
                lease
                    .authenticate(
                        credential,
                        &f.release,
                        &f.trust,
                        &f.app_authority,
                        &changed,
                        issuer.public_key(),
                        1500
                    )
                    .is_err()
            );
        }
        let mut changed = lease.clone();
        changed.subject.binding.policy_digest[0] ^= 1;
        changed.signature = Signature::new(
            issuer.private_key(),
            &changed.subject.canonical_signing_bytes().unwrap(),
        );
        assert!(
            changed
                .authenticate(
                    credential,
                    &f.release,
                    &f.trust,
                    &f.app_authority,
                    &c,
                    issuer.public_key(),
                    1500
                )
                .is_err()
        );
        let mut changed = lease;
        let KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } =
            &mut changed.app_possession
        else {
            unreachable!()
        };
        signature_der[5] ^= 1;
        assert!(
            changed
                .authenticate(
                    credential,
                    &f.release,
                    &f.trust,
                    &f.app_authority,
                    &c,
                    issuer.public_key(),
                    1500
                )
                .is_err()
        );
    }
    #[test]
    fn ordinary_approval_retains_actual_integrity_deadline_and_separate_verified_refresh() {
        use crate::kagemusha::*;
        use iroha_crypto::{Hash, HashOf};
        let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity();
        let enrolled = f.verify(300).unwrap();
        let credential = enrolled.app_credential();
        let c = credential.subject();
        let (signed, lease, issuer) = lease(&f, credential);
        let verified = lease
            .authenticate(
                credential,
                &f.release,
                &f.trust,
                &f.app_authority,
                &signed,
                issuer.public_key(),
                1500,
            )
            .unwrap();
        let subject = KagemushaHardwareTransitionSelectionV1 {
            version: 1,
            release_id: c.release_id,
            provider_policy_root: f.release.provider_policy_root(),
            app_policy_digest: credential.static_binding_digest(),
            credential_id: credential.digest(),
            network_id: crate::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::prehashed(c.network_id),
            )),
            lane_commitment: c.lane_id,
            hardware_profile_id: c.hardware_profile_id,
            policy_epoch: c.policy_epoch,
            hardware_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(c).unwrap(),
            hardware_epoch_generation: c.hardware_epoch,
            operation_kind: KagemushaOperationKindV1::Bootstrap,
            transition_statement_digest: [71; 32],
            candidate_envelope_digest: [0; 32],
            terminal_body_commitment: [0; 32],
            secure_index_before: 0,
            secure_index_after: 0,
        };
        let make = |issued, expires| {
            let challenge = KagemushaAppOperationApprovalChallengeV1 {
                version: 1,
                purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
                operation_id: [72; 32],
                nonce: [73; 32],
                account_binding: c.account_binding,
                authority_policy_digest: c.app_authority_policy_digest,
                attested_key_id: c.attested_key_id,
                enrollment_digest: credential.digest(),
                subject_signing_digest: Sha256::digest(subject.canonical_signing_bytes().unwrap())
                    .into(),
                normalized_guard_digest: [74; 32],
                issued_at_ms: issued,
                expires_at_ms: expires,
                subject,
            };
            let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
            let signature: P256Signature = key.sign(&challenge.canonical_signing_bytes().unwrap());
            KagemushaAppOperationApprovalV1 {
                challenge,
                evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                    signature_der: signature.to_der().as_bytes().to_vec(),
                },
            }
        };
        let original = make(1100, 1300);
        let original_token = original
            .authenticate(&original.challenge, credential, None, 1100)
            .unwrap();
        original_token.recheck_at_trusted_time(1199).unwrap();
        assert!(original_token.recheck_at_trusted_time(1099).is_err());
        assert!(original_token.recheck_at_trusted_time(1200).is_err());
        let after_refresh = make(1500, 2500);
        assert!(
            after_refresh
                .authenticate(&after_refresh.challenge, credential, None, 1500)
                .is_err()
        );
        let token = after_refresh
            .authenticate_with_integrity_lease(
                &after_refresh.challenge,
                credential,
                &verified,
                None,
                1500,
            )
            .unwrap();
        token.recheck_at_trusted_time(2399).unwrap();
        assert!(token.recheck_at_trusted_time(1499).is_err());
        assert!(token.recheck_at_trusted_time(2400).is_err());
        let mut substitution = after_refresh.clone();
        substitution.challenge.nonce[0] ^= 1;
        assert!(
            substitution
                .authenticate_with_integrity_lease(
                    &after_refresh.challenge,
                    credential,
                    &verified,
                    None,
                    1500
                )
                .is_err()
        );
        let mut signature = after_refresh.clone();
        let KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } =
            &mut signature.evidence
        else {
            unreachable!()
        };
        signature_der[5] ^= 1;
        assert!(
            signature
                .authenticate_with_integrity_lease(
                    &after_refresh.challenge,
                    credential,
                    &verified,
                    None,
                    1500
                )
                .is_err()
        );
    }
}

/// Encoder-owned raw positions in the complete original lease or its Ed-only frame.
/// All framing and enum tags are pinned; values and CRC positions remain witness cells.
#[derive(Debug, Clone)]
pub struct KagemushaPlayIntegrityRefreshLeaseOriginalLayoutV1 {
    /// Original digest preimage (or bare Ed-only frame) with exact fixed framing bytes.
    pub bytes: Vec<Option<u8>>,
    /// Complete canonical archive range.
    pub original: core::ops::Range<usize>,
    /// Raw LE16 version positions.
    pub version_bytes: [usize; 2],
    /// Eleven raw32 subject fields in lease signing-body order.
    pub fixed_digest_bytes: [[usize; 32]; 11],
    /// Six raw LE64 fields in lease signing-body order.
    pub scalar_bytes: [[usize; 8]; 6],
    /// Original issuer Ed signature raw64 positions.
    pub signature_bytes: [usize; 64],
    /// Original platform DER bytes, retaining actual variable length and enum framing.
    pub possession_signature_bytes: Vec<usize>,
    /// Mandatory issuer counter raw fields in the complete original; absent for Ed-only layout.
    pub issuer_admission_layout:
        Option<super::KagemushaOrdinaryIssuerCircuitAdmissionOriginalLayoutV1>,
}
/// Encoder-derived segments for one selected canonical stream at a bounded DER length.
/// These are data-only codec metadata; selecting a segment authenticates no issuer or lease.
#[derive(Debug, Clone)]
pub struct KagemushaPlayIntegrityRefreshLeaseStreamLengthV1 {
    /// Actual DER byte count, in the closed 8..=72 codec bound.
    pub der_length: usize,
    /// Exact prefix before the first raw DER byte, including encoder-produced length framing.
    pub prefix: Vec<Option<u8>>,
    /// Exact suffix immediately after the final raw DER byte, including any outer field framing.
    pub suffix: Vec<Option<u8>>,
    /// Semantic positions in this complete stream, including issuer admission when present.
    pub layout: KagemushaPlayIntegrityRefreshLeaseOriginalLayoutV1,
    /// Header payload-length LE64 positions relative to the prefix segment.
    pub header_payload_length_bytes: [usize; 8],
    /// Header CRC64-XZ positions relative to the prefix segment; computed over `archive_payload`.
    pub header_crc_bytes: [usize; 8],
    /// Canonical frame payload range in the assembled stream, excluding header and digest domain.
    pub archive_payload: core::ops::Range<usize>,
    /// Optional full-original archive-length LE64 prefix, before the canonical frame.
    pub original_length_bytes: Option<[usize; 8]>,
}
/// Relative coordinate for a semantic byte outside the active DER units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KagemushaPlayIntegrityRefreshLeaseStreamPositionV1 {
    /// A byte in the selected prefix segment.
    Prefix(usize),
    /// A byte in the selected suffix segment.
    Suffix(usize),
}
impl KagemushaPlayIntegrityRefreshLeaseStreamLengthV1 {
    /// Convert an encoder-owned semantic map position to its selected segment coordinate.
    /// # Errors
    /// Rejects DER positions (which use the active DER units) or out-of-stream positions.
    pub fn relative_semantic_position(
        &self,
        position: usize,
    ) -> Result<KagemushaPlayIntegrityRefreshLeaseStreamPositionV1, String> {
        if position < self.prefix.len() {
            Ok(KagemushaPlayIntegrityRefreshLeaseStreamPositionV1::Prefix(
                position,
            ))
        } else {
            let suffix_start = self.layout.bytes.len() - self.suffix.len();
            if position < suffix_start || position >= self.layout.bytes.len() {
                return Err("lease position is outside a semantic segment".into());
            }
            Ok(KagemushaPlayIntegrityRefreshLeaseStreamPositionV1::Suffix(
                position - suffix_start,
            ))
        }
    }
}
/// A single canonical bounded lease stream assembled from encoder-owned segments.
/// Use one length-selected prefix, `der_length - 1` repeated byte units, the final DER byte,
/// then the selected suffix. Hash and CRC that one active stream; these are not 65 proof copies.
#[derive(Debug, Clone)]
pub struct KagemushaPlayIntegrityRefreshLeaseStreamGrammarV1 {
    /// A raw DER byte (`None`) followed by the actual framing up to the next DER byte.
    pub repeated_der_byte_unit: Vec<Option<u8>>,
    /// Raw DER byte position within the repeated unit; derived from the encoder.
    pub der_byte_in_unit: usize,
    /// Maximum active prefix size over every supported width; fixes circuit capacity.
    pub maximum_prefix_bytes: usize,
    /// Maximum active suffix size over every supported width; fixes circuit capacity.
    pub maximum_suffix_bytes: usize,
    /// Maximum assembled canonical stream size over every supported width.
    pub maximum_stream_bytes: usize,
    /// Exact length-indexed framing and semantic maps for every supported DER width.
    pub lengths: Vec<KagemushaPlayIntegrityRefreshLeaseStreamLengthV1>,
}
impl KagemushaPlayIntegrityRefreshLeaseV1 {
    /// Exact bare canonical Ed-only frame layout for the issuer admission SHA256 relation.
    /// # Errors
    /// Rejects malformed originals or another declared canonical field layout.
    pub fn ed_only_preimage_layout(
        &self,
    ) -> Result<KagemushaPlayIntegrityRefreshLeaseOriginalLayoutV1, String> {
        lease_original_layout(
            &LeaseEdOriginal {
                subject: self.subject,
                signature: self.signature.clone(),
                app_possession: self.app_possession.clone(),
            },
            &self.subject,
            &self.signature,
            &self.app_possession,
            None,
            false,
        )
    }
    /// Exact complete original lease digest layout, including mandatory governed countersignature.
    /// # Errors
    /// Rejects malformed originals or another canonical field layout.
    pub fn original_preimage_layout(
        &self,
    ) -> Result<KagemushaPlayIntegrityRefreshLeaseOriginalLayoutV1, String> {
        self.canonical_bytes()?;
        lease_original_layout(
            self,
            &self.subject,
            &self.signature,
            &self.app_possession,
            Some(&self.circuit_admission),
            true,
        )
    }
    /// Derive one bounded canonical stream grammar from the sole Ed-only lease encoder.
    /// Every DER width is validated; no hand-written Norito length or element framing is used.
    /// # Errors
    /// Rejects a codec topology that cannot be represented by one repeated raw-byte unit.
    pub fn ed_only_canonical_stream_grammar(
        &self,
    ) -> Result<KagemushaPlayIntegrityRefreshLeaseStreamGrammarV1, String> {
        self.canonical_stream_grammar(false)
    }
    /// Derive the complete original digest stream, with its mandatory issuer admission.
    /// The returned selected stream includes the exact original domain and archive length prefix.
    /// # Errors
    /// Rejects malformed fields or a codec topology that changes the repeated DER-byte unit.
    pub fn original_canonical_stream_grammar(
        &self,
    ) -> Result<KagemushaPlayIntegrityRefreshLeaseStreamGrammarV1, String> {
        self.canonical_stream_grammar(true)
    }
    fn canonical_stream_grammar(
        &self,
        complete: bool,
    ) -> Result<KagemushaPlayIntegrityRefreshLeaseStreamGrammarV1, String> {
        let mut lengths = Vec::with_capacity(65);
        let mut common_unit: Option<Vec<Option<u8>>> = None;
        for length in 8..=72 {
            let mut template = self.clone();
            // Deliberately inert bytes supply codec width only. No signature/authentication is
            // performed or implied; all original evidence cells remain unassigned in the layout.
            template.app_possession = KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: vec![0x5a; length],
            };
            template.circuit_admission.subject = Self::circuit_admission_subject_for(
                &template.subject,
                &template.signature,
                &template.app_possession,
            )?;
            let layout = if complete {
                template.original_preimage_layout()?
            } else {
                template.ed_only_preimage_layout()?
            };
            let positions = &layout.possession_signature_bytes;
            if positions.len() != length {
                return Err("lease DER stream width differs".into());
            }
            let first = positions[0];
            let stride = positions[1]
                .checked_sub(first)
                .ok_or("lease DER ordering differs")?;
            if stride == 0
                || positions
                    .iter()
                    .enumerate()
                    .any(|(i, p)| *p != first + i * stride)
            {
                return Err("lease DER byte framing is not uniform".into());
            }
            let unit = layout.bytes[first..first + stride].to_vec();
            if unit.first() != Some(&None) || unit.iter().skip(1).any(Option::is_none) {
                return Err("lease DER framing contains another semantic byte".into());
            }
            if common_unit
                .as_ref()
                .is_some_and(|original| *original != unit)
            {
                return Err("lease DER byte framing changes with width".into());
            }
            let last = *positions.last().ok_or("lease DER stream absent")?;
            let prefix = layout.bytes[..first].to_vec();
            let suffix = layout.bytes[last + 1..].to_vec();
            let mut reconstructed = prefix.clone();
            for _ in 0..length - 1 {
                reconstructed.extend_from_slice(&unit);
            }
            reconstructed.push(None);
            reconstructed.extend_from_slice(&suffix);
            if reconstructed != layout.bytes {
                return Err("lease canonical stream reconstruction differs".into());
            }
            common_unit = Some(unit);
            let frame_start = layout.original.start;
            let frame = if complete {
                template.canonical_bytes()?
            } else {
                template.ed_only_canonical_bytes()?
            };
            let header = norito::core::Header::read(frame.as_slice()).map_err(|e| e.to_string())?;
            if frame[23..31] != header.length.to_le_bytes()
                || frame[31..39] != header.checksum.to_le_bytes()
                || frame_start + norito::core::Header::SIZE > prefix.len()
            {
                return Err("lease stream header framing differs".into());
            }
            let archive_payload = frame_start + norito::core::Header::SIZE..layout.original.end;
            if usize::try_from(header.length).ok() != Some(archive_payload.len()) {
                return Err("lease stream header payload width differs".into());
            }
            lengths.push(KagemushaPlayIntegrityRefreshLeaseStreamLengthV1 {
                der_length: length,
                prefix,
                suffix,
                header_payload_length_bytes: core::array::from_fn(|i| frame_start + 23 + i),
                header_crc_bytes: core::array::from_fn(|i| frame_start + 31 + i),
                archive_payload,
                original_length_bytes: complete
                    .then(|| core::array::from_fn(|i| ORIGINAL_DOMAIN.len() + i)),
                layout,
            });
        }
        Ok(KagemushaPlayIntegrityRefreshLeaseStreamGrammarV1 {
            repeated_der_byte_unit: common_unit.ok_or("lease DER stream grammar absent")?,
            der_byte_in_unit: 0,
            maximum_prefix_bytes: lengths
                .iter()
                .map(|v| v.prefix.len())
                .max()
                .ok_or("lease stream absent")?,
            maximum_suffix_bytes: lengths
                .iter()
                .map(|v| v.suffix.len())
                .max()
                .ok_or("lease stream absent")?,
            maximum_stream_bytes: lengths
                .iter()
                .map(|v| v.layout.bytes.len())
                .max()
                .ok_or("lease stream absent")?,
            lengths,
        })
    }
}
fn lease_original_layout<T: Encode + norito::NoritoSchema>(
    encoded: &T,
    subject: &KagemushaPlayIntegrityRefreshLeaseSubjectV1,
    signature: &Signature,
    evidence: &KagemushaAppOperationApprovalEvidenceV1,
    admission: Option<&super::KagemushaOrdinaryIssuerCircuitAdmissionV1>,
    digest_domain: bool,
) -> Result<KagemushaPlayIntegrityRefreshLeaseOriginalLayoutV1, String> {
    use super::kagemusha_ordinary_app_enrollment_v1::{
        layout_field_payload, layout_fixed_bytes_field, sole_changed_raw_position,
    };
    subject.canonical_signing_bytes()?;
    if signature.payload().len() != 64 {
        return Err("lease Ed signature width differs".into());
    }
    let frame = norito::encode_canonical(encoded).map_err(|e| e.to_string())?;
    let flags = frame[39];
    let payload = layout_field_payload(encoded, flags)?;
    let root = frame
        .len()
        .checked_sub(payload.len())
        .ok_or("lease canonical root differs")?;
    if root < norito::core::Header::SIZE || frame.get(root..) != Some(payload.as_slice()) {
        return Err("lease canonical root differs".into());
    }
    let prelude = if digest_domain {
        ORIGINAL_DOMAIN.len() + 8
    } else {
        0
    };
    let mut raw = Vec::new();
    if digest_domain {
        raw.extend_from_slice(ORIGINAL_DOMAIN);
        raw.extend_from_slice(&(frame.len() as u64).to_le_bytes());
    }
    raw.extend_from_slice(&frame);
    let mut bytes: Vec<Option<u8>> = raw.into_iter().map(Some).collect();
    bytes[prelude + 31..prelude + 39].fill(None);
    let mut cursor = root;
    let s = crate::isi::read_aos_field(&frame, &mut cursor, flags).map_err(|e| e.to_string())?;
    let subject_start = prelude + cursor - s.len();
    if s != layout_field_payload(subject, flags)? {
        return Err("lease subject layout differs".into());
    }
    let mut sub = 0;
    let mut fields = Vec::new();
    for _ in 0..14 {
        let f = crate::isi::read_aos_field(s, &mut sub, flags).map_err(|e| e.to_string())?;
        fields.push((f, subject_start + sub - f.len()));
    }
    if sub != s.len() || fields[0].0 != subject.version.to_le_bytes() {
        return Err("lease subject fields differ".into());
    }
    let version_bytes = core::array::from_fn(|i| fields[0].1 + i);
    let mut binding = 0;
    let mut pi_fields = Vec::new();
    for _ in 0..5 {
        let f = crate::isi::read_aos_field(fields[8].0, &mut binding, flags)
            .map_err(|e| e.to_string())?;
        pi_fields.push((f, fields[8].1 + binding - f.len()));
    }
    if binding != fields[8].0.len() {
        return Err("lease Integrity fields differ".into());
    }
    let digests = [
        subject.credential_digest,
        subject.challenge_digest,
        subject.attested_key_id,
        subject.release_id,
        subject.hardware_profile_id,
        subject.trust_policy_digest,
        subject.app_authority_policy_digest,
        subject.binding.request_hash,
        subject.binding.evidence_digest,
        subject.binding.policy_digest,
        subject.possession_original_digest,
    ];
    let scalar_values = [
        subject.policy_epoch,
        subject.hardware_epoch,
        subject.binding.verified_at_ms,
        subject.binding.refresh_before_ms,
        subject.issued_at_ms,
        subject.expires_at_ms,
    ];
    let mut fixed_digest_bytes = [[0usize; 32]; 11];
    for (index, (value, positions)) in digests.iter().zip(&mut fixed_digest_bytes).enumerate() {
        let (field, start) = if index < 7 {
            fields[index + 1]
        } else if index < 10 {
            pi_fields[index - 7]
        } else {
            fields[9]
        };
        if field != layout_fixed_bytes_field(value, flags)? {
            return Err("lease selector encoder differs".into());
        }
        for (index, position) in positions.iter_mut().enumerate() {
            let mut changed = *value;
            changed[index] ^= 1;
            *position = start
                + sole_changed_raw_position(
                    field,
                    &layout_fixed_bytes_field(&changed, flags)?,
                    changed[index],
                )?;
        }
    }
    let mut scalar_bytes = [[0usize; 8]; 6];
    for (index, (value, positions)) in scalar_values.iter().zip(&mut scalar_bytes).enumerate() {
        let (field, start) = match index {
            0 => fields[10],
            1 => fields[11],
            2 => pi_fields[3],
            3 => pi_fields[4],
            4 => fields[12],
            _ => fields[13],
        };
        if field != value.to_le_bytes() {
            return Err("lease scalar layout differs".into());
        }
        *positions = core::array::from_fn(|i| start + i);
    }
    let sig = crate::isi::read_aos_field(&frame, &mut cursor, flags).map_err(|e| e.to_string())?;
    let sig_start = prelude + cursor - sig.len();
    if sig != layout_field_payload(signature, flags)? {
        return Err("lease original Ed layout differs".into());
    }
    let mut signature_bytes = [0usize; 64];
    for (index, p) in signature_bytes.iter_mut().enumerate() {
        let mut changed = signature.payload().to_vec();
        changed[index] ^= 1;
        let enc = layout_field_payload(&Signature::from_bytes(&changed), flags)?;
        *p = sig_start + sole_changed_raw_position(sig, &enc, changed[index])?;
    }
    let original_evidence =
        crate::isi::read_aos_field(&frame, &mut cursor, flags).map_err(|e| e.to_string())?;
    let evidence_start = prelude + cursor - original_evidence.len();
    if original_evidence != layout_field_payload(evidence, flags)? {
        return Err("lease possession encoder differs".into());
    }
    let der = match evidence {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => signature_der,
        _ => return Err("Integrity lease possession must be Android".into()),
    };
    let mut possession_signature_bytes = Vec::new();
    for i in 0..der.len() {
        let mut changed = der.clone();
        changed[i] ^= 1;
        let enc = layout_field_payload(
            &KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: changed.clone(),
            },
            flags,
        )?;
        possession_signature_bytes
            .push(evidence_start + sole_changed_raw_position(original_evidence, &enc, changed[i])?);
    }
    let issuer_admission_layout = if let Some(admission) = admission {
        let a =
            crate::isi::read_aos_field(&frame, &mut cursor, flags).map_err(|e| e.to_string())?;
        let start = prelude + cursor - a.len();
        if a != layout_field_payload(admission, flags)? {
            return Err("lease issuer admission encoder differs".into());
        }
        let mut layout = admission.original_payload_layout(flags)?;
        for p in layout
            .version_bytes
            .iter_mut()
            .chain(core::iter::once(&mut layout.purpose_byte))
            .chain(layout.fixed_digest_bytes.iter_mut().flatten())
            .chain(layout.signature_bytes.iter_mut())
        {
            *p += start;
            bytes[*p] = None;
        }
        Some(layout)
    } else {
        None
    };
    if cursor != frame.len() {
        return Err("lease trailing fields".into());
    }
    for p in version_bytes
        .iter()
        .chain(fixed_digest_bytes.iter().flatten())
        .chain(scalar_bytes.iter().flatten())
        .chain(&signature_bytes)
        .chain(&possession_signature_bytes)
    {
        bytes[*p] = None;
    }
    Ok(KagemushaPlayIntegrityRefreshLeaseOriginalLayoutV1 {
        bytes,
        original: prelude..prelude + frame.len(),
        version_bytes,
        fixed_digest_bytes,
        scalar_bytes,
        signature_bytes,
        possession_signature_bytes,
        issuer_admission_layout,
    })
}
