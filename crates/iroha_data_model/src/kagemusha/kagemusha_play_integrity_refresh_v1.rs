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
        message(
            KAGEMUSHA_PLAY_INTEGRITY_REFRESH_CHALLENGE_DOMAIN_V1,
            &self.body()?,
        )
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
        message(POSSESSION_DOMAIN, &self.canonical_signing_bytes()?)
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
        message(KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_DOMAIN_V1, &body)
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
}
/// Genuine current periodic lease; no constructor, decoder or financial capability.
pub struct KagemushaVerifiedPlayIntegrityRefreshLeaseV1 {
    subject: KagemushaPlayIntegrityRefreshLeaseSubjectV1,
    original: Vec<u8>,
    digest: [u8; 32],
    authenticated_at_ms: u64,
}
impl KagemushaVerifiedPlayIntegrityRefreshLeaseV1 {
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
    /// Canonical original bytes only; no verification or current capability is returned.
    /// # Errors
    /// Rejects malformed, missing or oversized original fields.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.subject.canonical_signing_bytes()?;
        if self.signature.payload().len() != 64 {
            return Err("Integrity issuer signature width differs".into());
        }
        let original = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if original.len() > 4096 {
            return Err("Integrity lease original oversized".into());
        }
        Ok(original)
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
            None,
            &c.possession_signing_bytes()?,
        )?;
        self.signature
            .verify(&authority.authority_key, &s.canonical_signing_bytes()?)
            .map_err(|_| "Integrity lease issuer signature rejected")?;
        let mut hash = Sha256::new();
        hash.update(ORIGINAL_DOMAIN);
        hash.update((original.len() as u64).to_le_bytes());
        hash.update(&original);
        let token = KagemushaVerifiedPlayIntegrityRefreshLeaseV1 {
            subject: *s,
            original,
            digest: hash.finalize().into(),
            authenticated_at_ms: now,
        };
        token.recheck_at_trusted_time(now)?;
        Ok(token)
    }
}
fn message(domain: &[u8], body: &[u8]) -> Result<Vec<u8>, String> {
    let mut message = domain.to_vec();
    message.extend_from_slice(&(body.len() as u64).to_le_bytes());
    message.extend_from_slice(body);
    Ok(message)
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
        let lease = KagemushaPlayIntegrityRefreshLeaseV1 {
            signature: Signature::new(
                issuer.private_key(),
                &subject.canonical_signing_bytes().unwrap(),
            ),
            subject,
            app_possession: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: der,
            },
        };
        (signed, lease, issuer)
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
            };
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
