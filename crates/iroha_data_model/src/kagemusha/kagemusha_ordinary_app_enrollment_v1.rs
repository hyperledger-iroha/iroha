//! Governed ordinary-app credentials with separate platform and financial keys.
//!
//! The attestation service authenticates raw platform evidence before signing.
//! Native admission independently checks the signed release, exact policy original,
//! original challenge and expected key. These credentials establish identity and
//! approval scope; they do not assert platform rollback resistance or grant funds.

use super::{
    KagemushaAppAttestationAuthorityPolicyV1, KagemushaAppOperationApprovalChallengeV1,
    KagemushaAuthenticatedReleaseV1, KagemushaDevicePublicKeyV1, KagemushaHardwarePlatformClassV1,
    KagemushaHardwareProfileV1, kagemusha_device_key_reference_v1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Canonical public policy and signed credential archive ceiling.
pub const KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1: usize = 16 * 1024;
/// Exact ordinary credential issuer signing domain, including NUL.
pub const KAGEMUSHA_ORDINARY_APP_CREDENTIAL_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-app-credential\0";
/// Fixed credential body width; all optional slots are present and zero when absent.
pub const KAGEMUSHA_ORDINARY_APP_CREDENTIAL_BODY_BYTES_V1: usize =
    2 + 2 + 18 * 32 + 65 + 4 * 8 + 4 + 1 + 3 * 32 + 2 * 8;
/// Exact native enrollment challenge signing domain, including NUL.
pub const KAGEMUSHA_ORDINARY_APP_ENROLLMENT_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-app-enrollment-challenge\0";
const POLICY_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-app-trust-policy\0";
const CREDENTIAL_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-app-credential-original\0";
const STATIC_BINDING_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-app-static-binding\0";
const EVIDENCE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-app-enrollment-evidence\0";
const INTEGRITY_REQUEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:play-integrity-enrollment\0";

/// Model-owned absolute ranges in the sole ordinary issuer signing message.
#[derive(Debug, Clone, Copy)]
pub struct KagemushaOrdinaryAppCredentialSigningLayoutV1;
impl KagemushaOrdinaryAppCredentialSigningLayoutV1 {
    /// Exact signing domain range, including NUL.
    pub const DOMAIN: core::ops::Range<usize> =
        0..KAGEMUSHA_ORDINARY_APP_CREDENTIAL_DOMAIN_V1.len();
    /// Exact LE64 fixed body-length range.
    pub const BODY_LENGTH: core::ops::Range<usize> = Self::DOMAIN.end..Self::DOMAIN.end + 8;
    /// Exact complete unsigned body range.
    pub const BODY: core::ops::Range<usize> = Self::BODY_LENGTH.end
        ..Self::BODY_LENGTH.end + KAGEMUSHA_ORDINARY_APP_CREDENTIAL_BODY_BYTES_V1;
    /// Complete signature message width.
    pub const TOTAL_BYTES: usize = Self::BODY.end;
    /// LE16 first-release version.
    pub const VERSION: core::ops::Range<usize> = Self::BODY.start..Self::BODY.start + 2;
    /// One-byte platform tag (Android1, Apple2).
    pub const PLATFORM_CLASS: core::ops::Range<usize> = Self::BODY.start + 2..Self::BODY.start + 3;
    /// One-byte security tag (TEE1, StrongBox2, AppAttest3).
    pub const SECURITY_LEVEL: core::ops::Range<usize> = Self::BODY.start + 3..Self::BODY.start + 4;
    /// Exact raw32 `enrollment_id` slot.
    pub const ENROLLMENT_ID: core::ops::Range<usize> = Self::BODY.start + 4..Self::BODY.start + 36;
    /// Exact raw32 `client_nonce` slot.
    pub const CLIENT_NONCE: core::ops::Range<usize> = Self::BODY.start + 36..Self::BODY.start + 68;
    /// Exact raw32 `server_nonce` slot.
    pub const SERVER_NONCE: core::ops::Range<usize> = Self::BODY.start + 68..Self::BODY.start + 100;
    /// Exact raw32 `account_binding` slot.
    pub const ACCOUNT_BINDING: core::ops::Range<usize> =
        Self::BODY.start + 100..Self::BODY.start + 132;
    /// Exact raw32 `network_id` slot.
    pub const NETWORK_ID: core::ops::Range<usize> = Self::BODY.start + 132..Self::BODY.start + 164;
    /// Exact raw32 `lane_id` slot.
    pub const LANE_ID: core::ops::Range<usize> = Self::BODY.start + 164..Self::BODY.start + 196;
    /// Exact raw32 `release_id` slot.
    pub const RELEASE_ID: core::ops::Range<usize> = Self::BODY.start + 196..Self::BODY.start + 228;
    /// Exact raw32 `hardware_profile_id` slot.
    pub const HARDWARE_PROFILE_ID: core::ops::Range<usize> =
        Self::BODY.start + 228..Self::BODY.start + 260;
    /// Exact raw32 `suite_id` slot.
    pub const SUITE_ID: core::ops::Range<usize> = Self::BODY.start + 260..Self::BODY.start + 292;
    /// Exact raw32 `trust_policy_digest` slot.
    pub const TRUST_POLICY_DIGEST: core::ops::Range<usize> =
        Self::BODY.start + 292..Self::BODY.start + 324;
    /// Exact raw32 `app_authority_policy_digest` slot.
    pub const APP_AUTHORITY_POLICY_DIGEST: core::ops::Range<usize> =
        Self::BODY.start + 324..Self::BODY.start + 356;
    /// Exact raw32 `app_signing_identity_digest` slot.
    pub const APP_SIGNING_IDENTITY_DIGEST: core::ops::Range<usize> =
        Self::BODY.start + 356..Self::BODY.start + 388;
    /// Exact raw32 `app_release_digest` slot.
    pub const APP_RELEASE_DIGEST: core::ops::Range<usize> =
        Self::BODY.start + 388..Self::BODY.start + 420;
    /// Exact raw32 `attested_key_id` slot.
    pub const ATTESTED_KEY_ID: core::ops::Range<usize> =
        Self::BODY.start + 420..Self::BODY.start + 452;
    /// Exact raw32 `app_key_reference` slot.
    pub const APP_KEY_REFERENCE: core::ops::Range<usize> =
        Self::BODY.start + 452..Self::BODY.start + 484;
    /// Exact raw32 `financial_authority_commitment` slot.
    pub const FINANCIAL_AUTHORITY_COMMITMENT: core::ops::Range<usize> =
        Self::BODY.start + 484..Self::BODY.start + 516;
    /// Exact raw32 `platform_evidence_digest` slot.
    pub const PLATFORM_EVIDENCE_DIGEST: core::ops::Range<usize> =
        Self::BODY.start + 516..Self::BODY.start + 548;
    /// Exact raw32 `enrollment_challenge_digest` slot.
    pub const ENROLLMENT_CHALLENGE_DIGEST: core::ops::Range<usize> =
        Self::BODY.start + 548..Self::BODY.start + 580;
    /// Exact `app_public_key` slot; optional Integrity slots are zero when absent.
    pub const APP_PUBLIC_KEY: core::ops::Range<usize> =
        Self::BODY.start + 580..Self::BODY.start + 645;
    /// Exact `policy_epoch` slot; optional Integrity slots are zero when absent.
    pub const POLICY_EPOCH: core::ops::Range<usize> =
        Self::BODY.start + 645..Self::BODY.start + 653;
    /// Exact `hardware_epoch` slot; optional Integrity slots are zero when absent.
    pub const HARDWARE_EPOCH: core::ops::Range<usize> =
        Self::BODY.start + 653..Self::BODY.start + 661;
    /// Exact `issued_at_ms` slot; optional Integrity slots are zero when absent.
    pub const ISSUED_AT_MS: core::ops::Range<usize> =
        Self::BODY.start + 661..Self::BODY.start + 669;
    /// Exact `expires_at_ms` slot; optional Integrity slots are zero when absent.
    pub const EXPIRES_AT_MS: core::ops::Range<usize> =
        Self::BODY.start + 669..Self::BODY.start + 677;
    /// Exact `app_attest_counter_floor` slot; optional Integrity slots are zero when absent.
    pub const APP_ATTEST_COUNTER_FLOOR: core::ops::Range<usize> =
        Self::BODY.start + 677..Self::BODY.start + 681;
    /// Exact `play_integrity_tag` slot; optional Integrity slots are zero when absent.
    pub const PLAY_INTEGRITY_TAG: core::ops::Range<usize> =
        Self::BODY.start + 681..Self::BODY.start + 682;
    /// Exact `play_integrity_request_hash` slot; optional Integrity slots are zero when absent.
    pub const PLAY_INTEGRITY_REQUEST_HASH: core::ops::Range<usize> =
        Self::BODY.start + 682..Self::BODY.start + 714;
    /// Exact `play_integrity_evidence_digest` slot; optional Integrity slots are zero when absent.
    pub const PLAY_INTEGRITY_EVIDENCE_DIGEST: core::ops::Range<usize> =
        Self::BODY.start + 714..Self::BODY.start + 746;
    /// Exact `play_integrity_policy_digest` slot; optional Integrity slots are zero when absent.
    pub const PLAY_INTEGRITY_POLICY_DIGEST: core::ops::Range<usize> =
        Self::BODY.start + 746..Self::BODY.start + 778;
    /// Exact `play_integrity_verified_at_ms` slot; optional Integrity slots are zero when absent.
    pub const PLAY_INTEGRITY_VERIFIED_AT_MS: core::ops::Range<usize> =
        Self::BODY.start + 778..Self::BODY.start + 786;
    /// Exact `play_integrity_refresh_before_ms` slot; optional Integrity slots are zero when absent.
    pub const PLAY_INTEGRITY_REFRESH_BEFORE_MS: core::ops::Range<usize> =
        Self::BODY.start + 786..Self::BODY.start + 794;
}

/// Encoder-owned ranges and framing for the exact complete original credential digest.
/// Ranges address the domain + LE64 canonical archive length + complete canonical archive.
/// Values and CRC are None; Some bytes pin all authoritative framing/prefixes and option tags.
/// This data-only description grants no credential or signature authority.
#[derive(Debug, Clone)]
pub struct KagemushaOrdinaryAppCredentialOriginalLayoutV1 {
    /// Complete digest preimage template, including original Ed signature and CRC positions.
    pub bytes: Vec<Option<u8>>,
    /// Complete canonical original archive range within the digest preimage.
    pub original: core::ops::Range<usize>,
    /// All 28 encoded subject field ranges in the actual declared model order.
    /// Platform/security fields are their actual Norito enum representation, not signing tags.
    pub subject_fields: [core::ops::Range<usize>; 28],
    /// Complete encoded signature field; its vector framing is pinned in `bytes`.
    pub signature: core::ops::Range<usize>,
    /// Absolute positions of all 64 raw Ed25519 signature bytes in the original encoder.
    pub signature_bytes: [usize; 64],
    /// Actual LE16 version bytes validated against the model encoder.
    pub version_bytes: [usize; 2],
    /// Actual encoded platform discriminant bytes, not the one-byte signing-body tag.
    pub platform_class_bytes: Vec<usize>,
    /// Actual encoded security discriminant bytes; TEE/StrongBox can vary under one policy.
    pub security_level_bytes: Vec<usize>,
    /// Raw byte positions for the 18 raw32 subject selectors in issuer signing-body order.
    /// This includes account/scope, actual app key references and the financial commitment.
    pub fixed_digest_bytes: [[usize; 32]; 18],
    /// Actual 65 uncompressed SEC1 bytes, independently of the financial commitment.
    pub app_public_key_bytes: [usize; 65],
    /// Raw little-endian byte positions for policy/hardware epoch, issued/expiry and Apple floor.
    pub scalar_bytes: [Vec<usize>; 5],
    /// Five encoded Integrity binding fields when present; None pins the absent option bytes.
    pub play_integrity_fields: Option<[core::ops::Range<usize>; 5]>,
    /// Actual raw positions for the three Integrity digests and two little-endian timestamps.
    pub play_integrity_bytes: Option<[Vec<usize>; 5]>,
}

/// Platform security level established by the independent raw verifier.
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
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(
    tag = "level",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaAppKeySecurityLevelV1")]
pub enum KagemushaAppKeySecurityLevelV1 {
    /// Attested Android trusted execution environment, accepted only by exact policy.
    TrustedExecutionEnvironment,
    /// Attested Android StrongBox key; no usage-count or rollback property is implied.
    StrongBox,
    /// Apple App Attest key under its exact application/environment policy.
    AppleAppAttest,
}
impl KagemushaAppKeySecurityLevelV1 {
    /// Fixed one-byte issuer-body discriminant.
    #[must_use]
    pub const fn signing_tag(self) -> u8 {
        match self {
            Self::TrustedExecutionEnvironment => 1,
            Self::StrongBox => 2,
            Self::AppleAppAttest => 3,
        }
    }
}

/// Governed separate Play Integrity enrollment and refresh requirement.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaPlayIntegrityPolicyV1")]
pub struct KagemushaPlayIntegrityPolicyV1 {
    /// Digest of the exact server verification policy and Google project/app selection.
    pub policy_digest: [u8; 32],
    /// Maximum age at independent server verification.
    pub maximum_evidence_age_ms: u64,
    /// Maximum interval to the next policy refresh.
    pub maximum_refresh_interval_ms: u64,
    /// Require Google's recognized application verdict.
    pub require_play_recognized: bool,
    /// Require the applicable licensed application verdict.
    pub require_licensed: bool,
    /// Required device verdict: 1 device integrity, 2 strong integrity.
    pub minimum_device_integrity: u8,
}

/// Exact public policy original pinned by the enabled profile's policy digest.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryAppTrustPolicyV1")]
pub struct KagemushaOrdinaryAppTrustPolicyV1 {
    /// Sole first-release policy version.
    pub version: u16,
    /// Existing governed app-authority policy; its Ed key signs the credential.
    pub app_authority_policy_digest: [u8; 32],
    /// Exact ordinary platform class; OEM compact credentials use another format.
    pub platform_class: KagemushaHardwarePlatformClassV1,
    /// Sorted unique accepted Android levels; Apple requires an empty list.
    pub allowed_android_security_levels: Vec<KagemushaAppKeySecurityLevelV1>,
    /// Separate enrollment/refresh policy; explicit None is permitted only when governed.
    pub play_integrity_policy: Option<KagemushaPlayIntegrityPolicyV1>,
    /// Maximum credential lifetime, also bounded by the original app authority/profile.
    pub maximum_credential_lifetime_ms: u64,
}
impl KagemushaOrdinaryAppTrustPolicyV1 {
    /// Check the closed ordinary platform/policy shape.
    /// # Errors
    /// Rejects unsupported classes, unsorted levels, incomplete Integrity policy or lifetime.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.app_authority_policy_digest == [0; 32]
            || self.maximum_credential_lifetime_ms == 0
        {
            return Err("ordinary app trust policy incomplete".into());
        }
        match self.platform_class {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint => {
                if self.allowed_android_security_levels.is_empty()
                    || self.allowed_android_security_levels.len() > 2
                    || self
                        .allowed_android_security_levels
                        .iter()
                        .any(|level| *level == KagemushaAppKeySecurityLevelV1::AppleAppAttest)
                    || !self
                        .allowed_android_security_levels
                        .windows(2)
                        .all(|p| p[0] < p[1])
                {
                    return Err("ordinary Android security levels invalid".into());
                }
            }
            KagemushaHardwarePlatformClassV1::AppleAppAttest => {
                if !self.allowed_android_security_levels.is_empty()
                    || self.play_integrity_policy.is_some()
                {
                    return Err("ordinary Apple policy contains Android selectors".into());
                }
            }
            _ => return Err("ordinary app policy is not an ordinary platform".into()),
        }
        if let Some(policy) = self.play_integrity_policy {
            if policy.policy_digest == [0; 32]
                || policy.maximum_evidence_age_ms == 0
                || policy.maximum_refresh_interval_ms == 0
                || !matches!(policy.minimum_device_integrity, 1 | 2)
            {
                return Err("Play Integrity policy incomplete".into());
            }
        }
        Ok(())
    }

    /// Hash the exact bounded canonical policy original under its fixed domain.
    /// # Errors
    /// Rejects invalid shape or a codec/bound failure.
    pub fn canonical_digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        Ok(digest_original(POLICY_DOMAIN, &bounded_encode(self)?))
    }

    /// Match the policy to independently selected enabled-profile and app-authority originals.
    /// # Errors
    /// Rejects any policy, class, issuer, app or lifetime substitution.
    pub fn validate_for_profile(
        &self,
        profile: &KagemushaHardwareProfileV1,
        authority: &KagemushaAppAttestationAuthorityPolicyV1,
    ) -> Result<(), String> {
        self.validate()?;
        if profile.platform_class != self.platform_class
            || authority.platform_class != self.platform_class
            || profile.firmware_policy_digest != self.canonical_digest()?
            || profile.app_attestation_authority_policy_digest != self.app_authority_policy_digest
            || self.app_authority_policy_digest != authority.canonical_digest()?
            || self.maximum_credential_lifetime_ms > authority.maximum_lifetime_ms
        {
            return Err("ordinary app policy differs from governed originals".into());
        }
        Ok(())
    }
}

/// Pre-key enrollment subject reserved by the native owner and signed by the Core issuer.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryAppEnrollmentChallengeV1")]
pub struct KagemushaOrdinaryAppEnrollmentChallengeV1 {
    /// Sole first-release challenge version.
    pub version: u16,
    /// Exact original ordinary platform class.
    pub platform_class: KagemushaHardwarePlatformClassV1,
    /// Original native enrollment attempt identity.
    pub enrollment_id: [u8; 32],
    /// Original client challenge nonce.
    pub client_nonce: [u8; 32],
    /// Durably reserved server challenge nonce.
    pub server_nonce: [u8; 32],
    /// Model-owned account binding of the independently held wallet account.
    pub account_binding: [u8; 32],
    /// Exact genesis-derived network bytes.
    pub network_id: [u8; 32],
    /// Original selected native lane identity.
    pub lane_id: [u8; 32],
    /// Actual independently admitted monetary release.
    pub release_id: [u8; 32],
    /// Actual independently selected enabled profile.
    pub hardware_profile_id: [u8; 32],
    /// Actual enabled proof suite.
    pub suite_id: [u8; 32],
    /// Exact accepted ordinary trust-policy original digest.
    pub trust_policy_digest: [u8; 32],
    /// Exact accepted app-authority policy digest.
    pub app_authority_policy_digest: [u8; 32],
    /// Commitment to the separate native financial authority secret; never the app key scalar.
    pub financial_authority_commitment: [u8; 32],
    /// Original Core enrollment issuer policy digest.
    pub issuer_policy_digest: [u8; 32],
    /// Exact governed profile policy epoch.
    pub policy_epoch: u64,
    /// Actual original native financial enrollment epoch; not a platform counter.
    pub hardware_epoch: u64,
    /// Original native inclusive issue time.
    pub issued_at_ms: u64,
    /// Original native exclusive expiry, never renewed by phone time.
    pub expires_at_ms: u64,
}
impl KagemushaOrdinaryAppEnrollmentChallengeV1 {
    /// Return the sole 451-byte body under its fixed signing domain and length.
    /// # Errors
    /// Rejects unsupported class, missing identities/nonces or invalid original interval.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        let fields = [
            self.enrollment_id,
            self.client_nonce,
            self.server_nonce,
            self.account_binding,
            self.network_id,
            self.lane_id,
            self.release_id,
            self.hardware_profile_id,
            self.suite_id,
            self.trust_policy_digest,
            self.app_authority_policy_digest,
            self.financial_authority_commitment,
            self.issuer_policy_digest,
        ];
        nonzero(&fields)?;
        if self.version != 1
            || self.client_nonce == self.server_nonce
            || self.policy_epoch == 0
            || self.hardware_epoch == 0
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > super::KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1
        {
            return Err("ordinary enrollment challenge invalid".into());
        }
        let mut bytes = KAGEMUSHA_ORDINARY_APP_ENROLLMENT_CHALLENGE_DOMAIN_V1.to_vec();
        bytes.extend_from_slice(&451u64.to_le_bytes());
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.push(platform_tag(self.platform_class)?);
        for field in fields {
            bytes.extend_from_slice(&field);
        }
        bytes.extend_from_slice(&self.policy_epoch.to_le_bytes());
        bytes.extend_from_slice(&self.hardware_epoch.to_le_bytes());
        bytes.extend_from_slice(&self.issued_at_ms.to_le_bytes());
        bytes.extend_from_slice(&self.expires_at_ms.to_le_bytes());
        Ok(bytes)
    }

    /// Exact platform enrollment challenge hash, consumed by KeyMint or App Attest.
    /// # Errors
    /// Rejects invalid challenge shape.
    pub fn attestation_challenge(&self) -> Result<[u8; 32], String> {
        Ok(Sha256::digest(self.canonical_signing_bytes()?).into())
    }

    /// Separate Play Integrity request hash, binding the original preparation and generated key.
    /// # Errors
    /// Rejects invalid challenge or missing actual attested key identity.
    pub fn play_integrity_request_hash(
        &self,
        attested_key_id: [u8; 32],
    ) -> Result<[u8; 32], String> {
        nonzero(&[attested_key_id])?;
        let mut hash = Sha256::new();
        hash.update(INTEGRITY_REQUEST_DOMAIN);
        hash.update(self.canonical_signing_bytes()?);
        hash.update(attested_key_id);
        Ok(hash.finalize().into())
    }
}

/// Original signed enrollment preparation; decoding never admits its signer or scope.
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
    name = "iroha_data_model::kagemusha::KagemushaSignedOrdinaryAppEnrollmentChallengeV1"
)]
pub struct KagemushaSignedOrdinaryAppEnrollmentChallengeV1 {
    /// Complete original native subject.
    pub challenge: KagemushaOrdinaryAppEnrollmentChallengeV1,
    /// Signature under the independently selected original Core issuer key.
    pub signature: Signature,
}
impl KagemushaSignedOrdinaryAppEnrollmentChallengeV1 {
    /// Encode the exact public 515-byte preparation: fixed body451 followed by Ed signature64.
    /// # Errors
    /// Rejects an invalid subject or a non-Ed-width original signature.
    pub fn to_transport_bytes(&self) -> Result<Vec<u8>, String> {
        let message = self.challenge.canonical_signing_bytes()?;
        if self.signature.payload().len() != 64 {
            return Err("ordinary preparation signature width differs".into());
        }
        let mut bytes =
            message[KAGEMUSHA_ORDINARY_APP_ENROLLMENT_CHALLENGE_DOMAIN_V1.len() + 8..].to_vec();
        bytes.extend_from_slice(self.signature.payload());
        Ok(bytes)
    }
    /// Decode only the exact first-release public preparation transport.
    /// # Errors
    /// Rejects invalid tags, shape, width or noncanonical transport. Signer admission is separate.
    pub fn from_transport_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 515 {
            return Err("ordinary preparation transport width differs".into());
        }
        let mut reader = BodyReader::new(&bytes[..451]);
        let version = u16::from_le_bytes(reader.read()?);
        let platform_class = decode_platform(reader.read::<1>()?[0])?;
        let enrollment_id = reader.read()?;
        let client_nonce = reader.read()?;
        let server_nonce = reader.read()?;
        let account_binding = reader.read()?;
        let network_id = reader.read()?;
        let lane_id = reader.read()?;
        let release_id = reader.read()?;
        let hardware_profile_id = reader.read()?;
        let suite_id = reader.read()?;
        let trust_policy_digest = reader.read()?;
        let app_authority_policy_digest = reader.read()?;
        let financial_authority_commitment = reader.read()?;
        let issuer_policy_digest = reader.read()?;
        let policy_epoch = u64::from_le_bytes(reader.read()?);
        let hardware_epoch = u64::from_le_bytes(reader.read()?);
        let issued_at_ms = u64::from_le_bytes(reader.read()?);
        let expires_at_ms = u64::from_le_bytes(reader.read()?);
        let parsed = Self {
            challenge: KagemushaOrdinaryAppEnrollmentChallengeV1 {
                version,
                platform_class,
                enrollment_id,
                client_nonce,
                server_nonce,
                account_binding,
                network_id,
                lane_id,
                release_id,
                hardware_profile_id,
                suite_id,
                trust_policy_digest,
                app_authority_policy_digest,
                financial_authority_commitment,
                issuer_policy_digest,
                policy_epoch,
                hardware_epoch,
                issued_at_ms,
                expires_at_ms,
            },
            signature: Signature::from_bytes(&bytes[451..]),
        };
        if parsed.to_transport_bytes()? != bytes {
            return Err("ordinary preparation transport differs".into());
        }
        Ok(parsed)
    }

    /// Authenticate the exact preparation under independent Core issuer and retained subject.
    /// # Errors
    /// Rejects scope/signature/interval substitutions; no spending permission is returned.
    pub fn authenticate(
        &self,
        issuer_key: &PublicKey,
        expected: &KagemushaOrdinaryAppEnrollmentChallengeV1,
        trusted_now_ms: u64,
    ) -> Result<(), String> {
        if self.challenge != *expected
            || issuer_key.algorithm() != Algorithm::Ed25519
            || trusted_now_ms < expected.issued_at_ms
            || trusted_now_ms >= expected.expires_at_ms
        {
            return Err("ordinary enrollment preparation scope or time differs".into());
        }
        self.signature
            .verify(issuer_key, &self.challenge.canonical_signing_bytes()?)
            .map_err(|_| "ordinary enrollment preparation signature rejected".into())
    }
}

/// Exact E371 app-key possession message from original C, point and raw-attestation digest.
/// The attested key remains nonexportable; its signature does not reveal a financial secret.
/// The retired C-plus-key-only possession domain is not accepted.
/// # Errors
/// Rejects another preparation shape, absent key or missing raw-attestation digest.
pub fn kagemusha_ordinary_app_enrollment_possession_message_v1(
    challenge: &KagemushaOrdinaryAppEnrollmentChallengeV1,
    key: &KagemushaDevicePublicKeyV1,
    raw_platform_evidence_digest: [u8; 32],
) -> Result<Vec<u8>, String> {
    super::KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
        challenge,
        key,
        raw_platform_evidence_digest,
    )?
    .canonical_signing_bytes()
}

/// Commit complete bounded attestation and original possession evidence in their actual order.
/// This selector provides no attestation, issuer or wallet authority by itself.
/// # Errors
/// Rejects empty originals or role bounds before constructing the preimage.
pub fn kagemusha_ordinary_app_enrollment_evidence_digest_v1(
    raw_attestation: &[u8],
    raw_possession: &[u8],
) -> Result<[u8; 32], String> {
    if raw_attestation.is_empty()
        || raw_attestation.len() > 128 * 1024
        || raw_possession.is_empty()
        || raw_possession.len() > super::KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1
    {
        return Err("ordinary enrollment evidence bound differs".into());
    }
    let mut hash = Sha256::new();
    hash.update(EVIDENCE_DOMAIN);
    hash.update((raw_attestation.len() as u64).to_le_bytes());
    hash.update(raw_attestation);
    hash.update((raw_possession.len() as u64).to_le_bytes());
    hash.update(raw_possession);
    Ok(hash.finalize().into())
}

/// Original independent Google verdict binding, kept separate from KeyMint evidence.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaPlayIntegrityBindingV1")]
pub struct KagemushaPlayIntegrityBindingV1 {
    /// Exact original preparation plus actual generated-key request hash.
    pub request_hash: [u8; 32],
    /// SHA-256 of the independently decoded original Google evidence.
    pub evidence_digest: [u8; 32],
    /// Actual selected separate Google verification policy.
    pub policy_digest: [u8; 32],
    /// Native authority verification time, not a phone clock claim.
    pub verified_at_ms: u64,
    /// Exclusive refresh deadline under that original policy.
    pub refresh_before_ms: u64,
}

/// Complete ordinary credential issuer body; its two key roles never share a private scalar.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryAppCredentialSubjectV1")]
pub struct KagemushaOrdinaryAppCredentialSubjectV1 {
    /// Sole first-release credential version.
    pub version: u16,
    /// Actual platform class.
    pub platform_class: KagemushaHardwarePlatformClassV1,
    /// Actual raw-verifier security level, constrained by the governed trust policy.
    pub security_level: KagemushaAppKeySecurityLevelV1,
    /// Original native enrollment attempt.
    pub enrollment_id: [u8; 32],
    /// Original client nonce.
    pub client_nonce: [u8; 32],
    /// Original reserved server nonce.
    pub server_nonce: [u8; 32],
    /// Independently held wallet account binding.
    pub account_binding: [u8; 32],
    /// Exact original network.
    pub network_id: [u8; 32],
    /// Exact native lane.
    pub lane_id: [u8; 32],
    /// Actual admitted release.
    pub release_id: [u8; 32],
    /// Actual selected profile.
    pub hardware_profile_id: [u8; 32],
    /// Actual selected proof suite.
    pub suite_id: [u8; 32],
    /// Actual accepted ordinary trust policy.
    pub trust_policy_digest: [u8; 32],
    /// Actual accepted app authority policy.
    pub app_authority_policy_digest: [u8; 32],
    /// Actual verified app identity; RP hash on Apple.
    pub app_signing_identity_digest: [u8; 32],
    /// Governance-approved app distribution policy, not a measured binary hash.
    pub app_release_digest: [u8; 32],
    /// SHA-256 of the original nonexportable platform key's SEC1 public point.
    pub attested_key_id: [u8; 32],
    /// Model-derived key reference of that same platform key.
    pub app_key_reference: [u8; 32],
    /// Separate native financial secret commitment; the platform scalar is never a witness.
    pub financial_authority_commitment: [u8; 32],
    /// Model digest of complete original raw attestation and original app-key possession evidence.
    pub platform_evidence_digest: [u8; 32],
    /// Exact original preparation signing-message hash.
    pub enrollment_challenge_digest: [u8; 32],
    /// Actual attested uncompressed SEC1 P-256 key, used only for app approval signatures.
    pub app_public_key: KagemushaDevicePublicKeyV1,
    /// Actual governed policy epoch.
    pub policy_epoch: u64,
    /// Native financial enrollment epoch, distinct from an App Attest counter.
    pub hardware_epoch: u64,
    /// Inclusive independent issuer time.
    pub issued_at_ms: u64,
    /// Exclusive independent issuer credential expiry.
    pub expires_at_ms: u64,
    /// Original enrolled Apple assertion counter floor; zero for Android.
    pub app_attest_counter_floor: u32,
    /// Explicit separate Google evidence binding, absent only if the actual policy permits.
    pub play_integrity: Option<KagemushaPlayIntegrityBindingV1>,
}
impl KagemushaOrdinaryAppCredentialSubjectV1 {
    /// Decode the exact fixed unsigned issuer body and require its canonical roundtrip.
    /// This is a data-only codec, not enrollment admission.
    /// # Errors
    /// Rejects another width, tag, reserved optional slot, key or signing shape.
    pub fn from_signing_body(body: &[u8]) -> Result<Self, String> {
        if body.len() != KAGEMUSHA_ORDINARY_APP_CREDENTIAL_BODY_BYTES_V1 {
            return Err("ordinary credential body width differs".into());
        }
        let mut reader = BodyReader::new(body);
        let version = u16::from_le_bytes(reader.read()?);
        let platform_class = decode_platform(reader.read::<1>()?[0])?;
        let security_level = match reader.read::<1>()?[0] {
            1 => KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment,
            2 => KagemushaAppKeySecurityLevelV1::StrongBox,
            3 => KagemushaAppKeySecurityLevelV1::AppleAppAttest,
            _ => return Err("ordinary app key level unknown".into()),
        };
        let enrollment_id = reader.read()?;
        let client_nonce = reader.read()?;
        let server_nonce = reader.read()?;
        let account_binding = reader.read()?;
        let network_id = reader.read()?;
        let lane_id = reader.read()?;
        let release_id = reader.read()?;
        let hardware_profile_id = reader.read()?;
        let suite_id = reader.read()?;
        let trust_policy_digest = reader.read()?;
        let app_authority_policy_digest = reader.read()?;
        let app_signing_identity_digest = reader.read()?;
        let app_release_digest = reader.read()?;
        let attested_key_id = reader.read()?;
        let app_key_reference = reader.read()?;
        let financial_authority_commitment = reader.read()?;
        let platform_evidence_digest = reader.read()?;
        let enrollment_challenge_digest = reader.read()?;
        let app_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(&reader.read::<65>()?)
            .map_err(|e| e.to_string())?;
        let policy_epoch = u64::from_le_bytes(reader.read()?);
        let hardware_epoch = u64::from_le_bytes(reader.read()?);
        let issued_at_ms = u64::from_le_bytes(reader.read()?);
        let expires_at_ms = u64::from_le_bytes(reader.read()?);
        let app_attest_counter_floor = u32::from_le_bytes(reader.read()?);
        let slot: [u8; 113] = reader.read()?;
        let play_integrity = match slot[0] {
            0 if slot == [0; 113] => None,
            1 => {
                let mut pi = BodyReader::new(&slot[1..]);
                Some(KagemushaPlayIntegrityBindingV1 {
                    request_hash: pi.read()?,
                    evidence_digest: pi.read()?,
                    policy_digest: pi.read()?,
                    verified_at_ms: u64::from_le_bytes(pi.read()?),
                    refresh_before_ms: u64::from_le_bytes(pi.read()?),
                })
            }
            _ => return Err("ordinary Integrity slot invalid".into()),
        };
        let subject = Self {
            version,
            platform_class,
            security_level,
            enrollment_id,
            client_nonce,
            server_nonce,
            account_binding,
            network_id,
            lane_id,
            release_id,
            hardware_profile_id,
            suite_id,
            trust_policy_digest,
            app_authority_policy_digest,
            app_signing_identity_digest,
            app_release_digest,
            attested_key_id,
            app_key_reference,
            financial_authority_commitment,
            platform_evidence_digest,
            enrollment_challenge_digest,
            app_public_key,
            policy_epoch,
            hardware_epoch,
            issued_at_ms,
            expires_at_ms,
            app_attest_counter_floor,
            play_integrity,
        };
        let canonical = subject.canonical_signing_bytes()?;
        if canonical[KAGEMUSHA_ORDINARY_APP_CREDENTIAL_DOMAIN_V1.len() + 8..] != *body {
            return Err("ordinary credential body is not canonical".into());
        }
        Ok(subject)
    }

    /// Return the sole fixed 794-byte issuer body with domain and LE64 length prefix.
    /// # Errors
    /// Rejects invalid key identities, class, fixed fields or credential interval.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        let fields = [
            self.enrollment_id,
            self.client_nonce,
            self.server_nonce,
            self.account_binding,
            self.network_id,
            self.lane_id,
            self.release_id,
            self.hardware_profile_id,
            self.suite_id,
            self.trust_policy_digest,
            self.app_authority_policy_digest,
            self.app_signing_identity_digest,
            self.app_release_digest,
            self.attested_key_id,
            self.app_key_reference,
            self.financial_authority_commitment,
            self.platform_evidence_digest,
            self.enrollment_challenge_digest,
        ];
        nonzero(&fields)?;
        self.app_public_key.validate().map_err(|e| e.to_string())?;
        if self.version != 1
            || self.client_nonce == self.server_nonce
            || self.policy_epoch == 0
            || self.hardware_epoch == 0
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.attested_key_id
                != <[u8; 32]>::from(Sha256::digest(self.app_public_key.as_sec1_bytes()))
            || self.app_key_reference != kagemusha_device_key_reference_v1(&self.app_public_key)
        {
            return Err("ordinary credential subject invalid".into());
        }
        let class = platform_tag(self.platform_class)?;
        let mut bytes = KAGEMUSHA_ORDINARY_APP_CREDENTIAL_DOMAIN_V1.to_vec();
        bytes.extend_from_slice(
            &(KAGEMUSHA_ORDINARY_APP_CREDENTIAL_BODY_BYTES_V1 as u64).to_le_bytes(),
        );
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.push(class);
        bytes.push(self.security_level.signing_tag());
        for field in fields {
            bytes.extend_from_slice(&field);
        }
        bytes.extend_from_slice(self.app_public_key.as_sec1_bytes());
        for time in [
            self.policy_epoch,
            self.hardware_epoch,
            self.issued_at_ms,
            self.expires_at_ms,
        ] {
            bytes.extend_from_slice(&time.to_le_bytes());
        }
        bytes.extend_from_slice(&self.app_attest_counter_floor.to_le_bytes());
        if let Some(integrity) = self.play_integrity {
            bytes.push(1);
            for field in [
                integrity.request_hash,
                integrity.evidence_digest,
                integrity.policy_digest,
            ] {
                bytes.extend_from_slice(&field);
            }
            bytes.extend_from_slice(&integrity.verified_at_ms.to_le_bytes());
            bytes.extend_from_slice(&integrity.refresh_before_ms.to_le_bytes());
        } else {
            bytes.extend_from_slice(&[0; 113]);
        }
        if bytes.len()
            != KAGEMUSHA_ORDINARY_APP_CREDENTIAL_DOMAIN_V1.len()
                + 8
                + KAGEMUSHA_ORDINARY_APP_CREDENTIAL_BODY_BYTES_V1
        {
            return Err("ordinary credential fixed issuer layout changed".into());
        }
        Ok(bytes)
    }
    fn encoded_field_values(&self) -> [Vec<u8>; 28] {
        [
            self.version.encode(),
            self.platform_class.encode(),
            self.security_level.encode(),
            self.enrollment_id.encode(),
            self.client_nonce.encode(),
            self.server_nonce.encode(),
            self.account_binding.encode(),
            self.network_id.encode(),
            self.lane_id.encode(),
            self.release_id.encode(),
            self.hardware_profile_id.encode(),
            self.suite_id.encode(),
            self.trust_policy_digest.encode(),
            self.app_authority_policy_digest.encode(),
            self.app_signing_identity_digest.encode(),
            self.app_release_digest.encode(),
            self.attested_key_id.encode(),
            self.app_key_reference.encode(),
            self.financial_authority_commitment.encode(),
            self.platform_evidence_digest.encode(),
            self.enrollment_challenge_digest.encode(),
            self.app_public_key.encode(),
            self.policy_epoch.encode(),
            self.hardware_epoch.encode(),
            self.issued_at_ms.encode(),
            self.expires_at_ms.encode(),
            self.app_attest_counter_floor.encode(),
            self.play_integrity.encode(),
        ]
    }
}

/// New ordinary-app credential, signed under the actual governed Ed app authority.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryAppCredentialV1")]
pub struct KagemushaOrdinaryAppCredentialV1 {
    /// Complete unsigned ordinary issuer body.
    pub subject: KagemushaOrdinaryAppCredentialSubjectV1,
    /// Ed25519 signature over canonical_signing_bytes, not an OEM compact P-256 signature.
    pub signature: Signature,
}

/// Actual checked ordinary credential originals. No decoder, clone or public constructor exists.
pub struct KagemushaVerifiedOrdinaryAppCredentialV1 {
    subject: KagemushaOrdinaryAppCredentialSubjectV1,
    original: Vec<u8>,
    digest: [u8; 32],
    static_binding_digest: [u8; 32],
}
impl KagemushaVerifiedOrdinaryAppCredentialV1 {
    /// Borrow the actual checked credential subject.
    #[must_use]
    pub const fn subject(&self) -> &KagemushaOrdinaryAppCredentialSubjectV1 {
        &self.subject
    }
    /// Borrow the complete canonical original signed credential.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Return the model-owned original credential digest.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Return the exact acyclic ordinary app/key/financial policy binding.
    #[must_use]
    pub const fn static_binding_digest(&self) -> [u8; 32] {
        self.static_binding_digest
    }
    /// Recheck the original credential and optional Integrity refresh interval without renewal.
    /// # Errors
    /// Rejects original expiry or a missing overdue policy refresh.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<(), String> {
        if now < self.subject.issued_at_ms
            || now >= self.subject.expires_at_ms
            || self
                .subject
                .play_integrity
                .is_some_and(|pi| now < pi.verified_at_ms || now >= pi.refresh_before_ms)
        {
            return Err("ordinary credential or Integrity refresh expired".into());
        }
        Ok(())
    }

    /// Recheck a separately admitted periodic Integrity lease without changing the credential.
    /// # Errors
    /// Rejects another original credential, expired credential or expired/regressing lease.
    pub fn recheck_with_integrity_lease(
        &self,
        lease: &super::KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
        now: u64,
    ) -> Result<(), String> {
        if self.subject.platform_class != KagemushaHardwarePlatformClassV1::AndroidKeyMint
            || self.subject.play_integrity.is_none()
            || self.digest != lease.subject().credential_digest
            || self.subject.attested_key_id != lease.subject().attested_key_id
            || now < self.subject.issued_at_ms
            || now >= self.subject.expires_at_ms
        {
            return Err("Integrity lease does not select this original credential".into());
        }
        lease.recheck_at_trusted_time(now)
    }
}
fn sole_changed_raw_position(original: &[u8], changed: &[u8], raw: u8) -> Result<usize, String> {
    if original.len() != changed.len() {
        return Err("ordinary credential raw field width differs".into());
    }
    let mut differences = original
        .iter()
        .zip(changed)
        .enumerate()
        .filter_map(|(offset, (left, right))| (left != right).then_some(offset));
    let position = differences
        .next()
        .ok_or("ordinary credential raw field byte absent")?;
    if differences.next().is_some() || changed[position] != raw {
        return Err("ordinary credential raw field byte layout differs".into());
    }
    Ok(position)
}

impl KagemushaOrdinaryAppCredentialV1 {
    /// Digest of the exact complete canonical original; it provides no issuer admission.
    /// # Errors
    /// Rejects invalid or oversized original archive shape.
    pub fn canonical_digest(&self) -> Result<[u8; 32], String> {
        Ok(digest_original(
            CREDENTIAL_DIGEST_DOMAIN,
            &self.canonical_bytes()?,
        ))
    }
    /// Encode the sole bounded complete original ordinary credential, without granting trust.
    /// # Errors
    /// Rejects invalid signing shape, signature width or canonical archive bounds.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.subject.canonical_signing_bytes()?;
        if self.signature.payload().len() != 64 {
            return Err("ordinary credential Ed signature width differs".into());
        }
        bounded_encode(self)
    }
    /// Decode a bounded exact original before any issuer or owner admission.
    /// # Errors
    /// Rejects empty/oversized/noncanonical archives or malformed fixed issuer shape.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1 {
            return Err("ordinary credential archive bound differs".into());
        }
        let value: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|e| e.to_string())?;
        if value.canonical_bytes()? != bytes {
            return Err("ordinary credential original is not canonical".into());
        }
        Ok(value)
    }

    /// Exact model-owned digest preimage layout derived from the sole canonical encoder.
    /// This validates offsets against separately encoded subject fields and raw issuer signature.
    /// # Errors
    /// Rejects changed frame/schema/field layouts, another signature width or invalid body shape.
    pub fn original_preimage_layout(
        &self,
    ) -> Result<KagemushaOrdinaryAppCredentialOriginalLayoutV1, String> {
        self.subject.canonical_signing_bytes()?;
        if self.signature.payload().len() != 64 {
            return Err("ordinary credential Ed signature width differs".into());
        }
        let frame = bounded_encode(self)?;
        let payload = self.encode();
        let root_offset = frame
            .len()
            .checked_sub(payload.len())
            .ok_or("ordinary credential root layout differs")?;
        if root_offset < norito::core::Header::SIZE
            || frame.get(root_offset..) != Some(payload.as_slice())
        {
            return Err("ordinary credential canonical root layout differs".into());
        }
        let flags = frame[39];
        let _layout_flags = norito::core::DecodeFlagsGuard::enter(flags);
        let mut offset = root_offset;
        let subject_bytes =
            crate::isi::read_aos_field(&frame, &mut offset, flags).map_err(|e| e.to_string())?;
        let subject_start = offset - subject_bytes.len();
        if subject_bytes != self.subject.encode() {
            return Err("ordinary credential subject encoder differs".into());
        }
        let signature_bytes =
            crate::isi::read_aos_field(&frame, &mut offset, flags).map_err(|e| e.to_string())?;
        let signature_start = offset - signature_bytes.len();
        let decoded_signature: Signature =
            crate::isi::decode_aos_canonical_field(signature_bytes, flags)
                .map_err(|error| error.to_string())?;
        if offset != frame.len() || decoded_signature != self.signature {
            return Err("ordinary credential original signature framing differs".into());
        }
        let prelude_len = CREDENTIAL_DIGEST_DOMAIN.len() + 8;
        let signature = prelude_len + signature_start..prelude_len + offset;
        let mut signature_positions = [0_usize; 64];
        for (index, position) in signature_positions.iter_mut().enumerate() {
            let mut changed = self.signature.payload().to_vec();
            changed[index] ^= 1;
            let encoded = {
                let _guard = norito::core::DecodeFlagsGuard::enter(flags);
                Signature::from_bytes(&changed).encode()
            };
            if encoded.len() != signature_bytes.len() {
                return Err("ordinary credential signature byte layout width differs".into());
            }
            let mut differences = encoded
                .iter()
                .zip(signature_bytes)
                .enumerate()
                .filter_map(|(offset, (left, right))| (left != right).then_some(offset));
            let local = differences
                .next()
                .ok_or("ordinary credential signature byte absent")?;
            if differences.next().is_some() || encoded[local] != changed[index] {
                return Err("ordinary credential signature byte layout differs".into());
            }
            *position = prelude_len + signature_start + local;
        }
        let expected_fields = self.subject.encoded_field_values();
        let mut subject_offset = 0;
        let mut ranges: [core::ops::Range<usize>; 28] = core::array::from_fn(|_| 0..0);
        for (index, (range, expected)) in ranges.iter_mut().zip(expected_fields).enumerate() {
            let field = crate::isi::read_aos_field(subject_bytes, &mut subject_offset, flags)
                .map_err(|e| e.to_string())?;
            if field != expected {
                return Err(format!(
                    "ordinary credential declared field {index} layout differs"
                ));
            }
            *range = prelude_len + subject_start + subject_offset - field.len()
                ..prelude_len + subject_start + subject_offset;
        }
        if subject_offset != subject_bytes.len() {
            return Err("ordinary credential subject trailing fields".into());
        }
        if &frame[ranges[0].start - prelude_len..ranges[0].end - prelude_len]
            != self.subject.version.to_le_bytes()
        {
            return Err("ordinary credential version scalar encoder differs".into());
        }
        let version_bytes = core::array::from_fn(|index| ranges[0].start + index);
        let platform_class_bytes: Vec<usize> = ranges[1].clone().collect();
        let security_level_bytes: Vec<usize> = ranges[2].clone().collect();
        let selectors = [
            self.subject.enrollment_id,
            self.subject.client_nonce,
            self.subject.server_nonce,
            self.subject.account_binding,
            self.subject.network_id,
            self.subject.lane_id,
            self.subject.release_id,
            self.subject.hardware_profile_id,
            self.subject.suite_id,
            self.subject.trust_policy_digest,
            self.subject.app_authority_policy_digest,
            self.subject.app_signing_identity_digest,
            self.subject.app_release_digest,
            self.subject.attested_key_id,
            self.subject.app_key_reference,
            self.subject.financial_authority_commitment,
            self.subject.platform_evidence_digest,
            self.subject.enrollment_challenge_digest,
        ];
        let mut fixed_digest_bytes = [[0_usize; 32]; 18];
        for (field_index, (raw, positions)) in
            selectors.iter().zip(&mut fixed_digest_bytes).enumerate()
        {
            let range = &ranges[field_index + 3];
            let encoded_original = &frame[range.start - prelude_len..range.end - prelude_len];
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            if raw.encode() != encoded_original {
                return Err("ordinary credential raw selector encoder differs".into());
            }
            for (byte_index, position) in positions.iter_mut().enumerate() {
                let mut changed = *raw;
                changed[byte_index] ^= 1;
                let encoded_changed = changed.encode();
                *position = range.start
                    + sole_changed_raw_position(
                        encoded_original,
                        &encoded_changed,
                        changed[byte_index],
                    )?;
            }
        }
        let key_range = &ranges[21];
        let raw_key = self.subject.app_public_key.as_sec1_bytes();
        if &frame[key_range.start - prelude_len..key_range.end - prelude_len] != raw_key {
            return Err("ordinary credential raw SEC1 encoder differs".into());
        }
        let app_public_key_bytes = core::array::from_fn(|index| key_range.start + index);
        let scalar_values = [
            self.subject.policy_epoch.to_le_bytes().to_vec(),
            self.subject.hardware_epoch.to_le_bytes().to_vec(),
            self.subject.issued_at_ms.to_le_bytes().to_vec(),
            self.subject.expires_at_ms.to_le_bytes().to_vec(),
            self.subject.app_attest_counter_floor.to_le_bytes().to_vec(),
        ];
        let mut scalar_bytes: [Vec<usize>; 5] = core::array::from_fn(|_| Vec::new());
        for (index, (raw, positions)) in scalar_values.iter().zip(&mut scalar_bytes).enumerate() {
            let range = &ranges[index + 22];
            if &frame[range.start - prelude_len..range.end - prelude_len] != raw {
                return Err("ordinary credential raw scalar encoder differs".into());
            }
            positions.extend((0..raw.len()).map(|byte| range.start + byte));
        }
        let mut preimage = CREDENTIAL_DIGEST_DOMAIN.to_vec();
        preimage.extend_from_slice(&(frame.len() as u64).to_le_bytes());
        preimage.extend_from_slice(&frame);
        let mut template: Vec<Option<u8>> = preimage.into_iter().map(Some).collect();
        template[prelude_len + 31..prelude_len + 39].fill(None);
        for position in version_bytes
            .iter()
            .chain(&platform_class_bytes)
            .chain(&security_level_bytes)
        {
            template[*position] = None;
        }
        for position in fixed_digest_bytes.iter().flatten() {
            template[*position] = None;
        }
        for position in app_public_key_bytes {
            template[position] = None;
        }
        for position in scalar_bytes.iter().flatten() {
            template[*position] = None;
        }
        for position in signature_positions {
            template[position] = None;
        }
        let (pi_fields, pi_bytes) = if let Some(pi) = self.subject.play_integrity {
            let encoded = pi.encode();
            let option = &frame[ranges[27].start - prelude_len..ranges[27].end - prelude_len];
            if !option.ends_with(&encoded) {
                return Err("ordinary credential Integrity option encoder differs".into());
            }
            let start = ranges[27].end - encoded.len();
            let mut cursor = 0;
            let expected = [
                pi.request_hash.encode(),
                pi.evidence_digest.encode(),
                pi.policy_digest.encode(),
                pi.verified_at_ms.encode(),
                pi.refresh_before_ms.encode(),
            ];
            let mut fields: [core::ops::Range<usize>; 5] = core::array::from_fn(|_| 0..0);
            let mut raw_positions: [Vec<usize>; 5] = core::array::from_fn(|_| Vec::new());
            let raw_digests = [pi.request_hash, pi.evidence_digest, pi.policy_digest];
            let raw_scalars = [
                pi.verified_at_ms.to_le_bytes(),
                pi.refresh_before_ms.to_le_bytes(),
            ];
            for (index, (range, expected)) in fields.iter_mut().zip(expected).enumerate() {
                let field = crate::isi::read_aos_field(&encoded, &mut cursor, flags)
                    .map_err(|e| e.to_string())?;
                if field != expected {
                    return Err("ordinary credential Integrity field encoder differs".into());
                }
                *range = start + cursor - field.len()..start + cursor;
                if index < 3 {
                    let raw = raw_digests[index];
                    for byte_index in 0..32 {
                        let mut changed = raw;
                        changed[byte_index] ^= 1;
                        let encoded_changed = {
                            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
                            changed.encode()
                        };
                        raw_positions[index].push(
                            range.start
                                + sole_changed_raw_position(
                                    field,
                                    &encoded_changed,
                                    changed[byte_index],
                                )?,
                        );
                    }
                } else {
                    if field != raw_scalars[index - 3] {
                        return Err("ordinary credential Integrity scalar layout differs".into());
                    }
                    raw_positions[index].extend(range.clone());
                }
                for position in &raw_positions[index] {
                    template[*position] = None;
                }
            }
            if cursor != encoded.len() {
                return Err("ordinary credential Integrity trailing bytes".into());
            }
            (Some(fields), Some(raw_positions))
        } else {
            (None, None)
        };
        Ok(KagemushaOrdinaryAppCredentialOriginalLayoutV1 {
            bytes: template,
            original: prelude_len..prelude_len + frame.len(),
            subject_fields: ranges,
            signature,
            signature_bytes: signature_positions,
            version_bytes,
            platform_class_bytes,
            security_level_bytes,
            fixed_digest_bytes,
            app_public_key_bytes,
            scalar_bytes,
            play_integrity_fields: pi_fields,
            play_integrity_bytes: pi_bytes,
        })
    }

    /// Authenticate actual issuer, policy, release, original preparation and independently held key.
    /// # Errors
    /// Rejects substituted key roles, signer, policy, release, challenge, scope, Integrity or time.
    pub fn authenticate(
        &self,
        release: &KagemushaAuthenticatedReleaseV1,
        trust: &KagemushaOrdinaryAppTrustPolicyV1,
        authority: &KagemushaAppAttestationAuthorityPolicyV1,
        expected: &KagemushaOrdinaryAppEnrollmentChallengeV1,
        expected_key: &KagemushaDevicePublicKeyV1,
        trusted_now_ms: u64,
    ) -> Result<KagemushaVerifiedOrdinaryAppCredentialV1, String> {
        let enabled = release
            .enabled_profile(expected.hardware_profile_id)
            .ok_or("ordinary credential profile unavailable")?;
        if expected.release_id != release.release_id()
            || expected.network_id != *release.network_id().as_bytes()
            || expected.suite_id != enabled.suite_id
            || expected.policy_epoch != enabled.policy_epoch
        {
            return Err("ordinary credential differs from actual release".into());
        }
        self.authenticate_originals(
            &enabled.hardware_profile,
            trust,
            authority,
            expected,
            expected_key,
            trusted_now_ms,
        )
    }

    fn authenticate_originals(
        &self,
        profile: &KagemushaHardwareProfileV1,
        trust: &KagemushaOrdinaryAppTrustPolicyV1,
        authority: &KagemushaAppAttestationAuthorityPolicyV1,
        expected: &KagemushaOrdinaryAppEnrollmentChallengeV1,
        expected_key: &KagemushaDevicePublicKeyV1,
        now: u64,
    ) -> Result<KagemushaVerifiedOrdinaryAppCredentialV1, String> {
        trust.validate_for_profile(profile, authority)?;
        let s = &self.subject;
        let message = s.canonical_signing_bytes()?;
        if s.platform_class != expected.platform_class
            || s.platform_class != profile.platform_class
            || s.enrollment_id != expected.enrollment_id
            || s.client_nonce != expected.client_nonce
            || s.server_nonce != expected.server_nonce
            || s.account_binding != expected.account_binding
            || s.network_id != expected.network_id
            || s.lane_id != expected.lane_id
            || s.release_id != expected.release_id
            || s.hardware_profile_id != expected.hardware_profile_id
            || s.suite_id != expected.suite_id
            || s.policy_epoch != expected.policy_epoch
            || s.policy_epoch != profile.policy_epoch
            || s.hardware_epoch != expected.hardware_epoch
            || s.trust_policy_digest != expected.trust_policy_digest
            || s.trust_policy_digest != trust.canonical_digest()?
            || s.app_authority_policy_digest != expected.app_authority_policy_digest
            || s.app_authority_policy_digest != authority.canonical_digest()?
            || s.financial_authority_commitment != expected.financial_authority_commitment
            || s.enrollment_challenge_digest != expected.attestation_challenge()?
            || s.app_public_key != *expected_key
            || s.app_signing_identity_digest != authority.app_signing_identity_digest
            || s.app_release_digest != authority.app_release_digest
            || s.issued_at_ms < expected.issued_at_ms
            || s.issued_at_ms >= expected.expires_at_ms
            || s.issued_at_ms < profile.valid_from_ms
            || s.expires_at_ms > profile.expires_at_ms
            || s.expires_at_ms - s.issued_at_ms > trust.maximum_credential_lifetime_ms
        {
            return Err("ordinary credential scope differs from original selection".into());
        }
        match s.platform_class {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint => {
                if s.app_attest_counter_floor != 0
                    || !trust
                        .allowed_android_security_levels
                        .contains(&s.security_level)
                {
                    return Err("ordinary Android level or counter invalid".into());
                }
            }
            KagemushaHardwarePlatformClassV1::AppleAppAttest => {
                if s.security_level != KagemushaAppKeySecurityLevelV1::AppleAppAttest
                    || s.play_integrity.is_some()
                {
                    return Err("ordinary Apple platform evidence invalid".into());
                }
            }
            _ => return Err("ordinary credential cannot replace OEM compact credential".into()),
        }
        match (trust.play_integrity_policy, s.play_integrity) {
            (None, None) => (),
            (Some(policy), Some(binding)) => {
                nonzero(&[
                    binding.request_hash,
                    binding.evidence_digest,
                    binding.policy_digest,
                ])?;
                if binding.request_hash
                    != expected.play_integrity_request_hash(s.attested_key_id)?
                    || binding.policy_digest != policy.policy_digest
                    || binding.verified_at_ms < expected.issued_at_ms
                    || binding.verified_at_ms > s.issued_at_ms
                    || s.issued_at_ms - binding.verified_at_ms > policy.maximum_evidence_age_ms
                    || binding.refresh_before_ms <= binding.verified_at_ms
                    || binding.refresh_before_ms > s.expires_at_ms
                    || binding.refresh_before_ms - binding.verified_at_ms
                        > policy.maximum_refresh_interval_ms
                {
                    return Err("ordinary Play Integrity binding differs".into());
                }
            }
            _ => return Err("ordinary Play Integrity required or unsolicited".into()),
        }
        self.signature
            .verify(&authority.authority_key, &message)
            .map_err(|_| "ordinary credential Ed authority signature rejected")?;
        let original = bounded_encode(self)?;
        let mut static_body = Vec::new();
        for field in [
            s.account_binding,
            s.network_id,
            s.lane_id,
            s.release_id,
            s.hardware_profile_id,
            s.suite_id,
            s.trust_policy_digest,
            s.app_authority_policy_digest,
            s.app_signing_identity_digest,
            s.app_release_digest,
            s.attested_key_id,
            s.app_key_reference,
            s.financial_authority_commitment,
        ] {
            static_body.extend_from_slice(&field);
        }
        let checked = KagemushaVerifiedOrdinaryAppCredentialV1 {
            subject: *s,
            digest: digest_original(CREDENTIAL_DIGEST_DOMAIN, &original),
            original,
            static_binding_digest: digest_original(STATIC_BINDING_DOMAIN, &static_body),
        };
        checked.recheck_at_trusted_time(now)?;
        Ok(checked)
    }
}

/// Derive the ordinary native financial epoch identity from the exact signed credential.
/// This is logical native metadata, never a platform monotonicity or rollback claim.
/// # Errors
/// Rejects another signing shape or reserved generation/financial commitment.
pub fn kagemusha_ordinary_financial_epoch_id_v1(
    subject: &KagemushaOrdinaryAppCredentialSubjectV1,
) -> Result<[u8; 32], String> {
    subject.canonical_signing_bytes()?;
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:ordinary-financial-epoch\0");
    hash.update((6u64 * 32 + 8).to_le_bytes());
    for field in [
        subject.enrollment_id,
        subject.network_id,
        subject.lane_id,
        subject.release_id,
        subject.hardware_profile_id,
        subject.financial_authority_commitment,
    ] {
        hash.update(field);
    }
    hash.update(subject.hardware_epoch.to_le_bytes());
    Ok(hash.finalize().into())
}

/// Model-owned ordinary account binding shared with the operation approval challenge.
#[must_use]
pub fn kagemusha_ordinary_app_account_binding_v1(account: &crate::account::AccountId) -> [u8; 32] {
    KagemushaAppOperationApprovalChallengeV1::account_binding(account)
}

fn platform_tag(class: KagemushaHardwarePlatformClassV1) -> Result<u8, String> {
    match class {
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => Ok(1),
        KagemushaHardwarePlatformClassV1::AppleAppAttest => Ok(2),
        _ => Err("ordinary app platform unsupported".into()),
    }
}
fn nonzero(fields: &[[u8; 32]]) -> Result<(), String> {
    if fields.contains(&[0; 32]) {
        Err("ordinary app selector missing".into())
    } else {
        Ok(())
    }
}
fn bounded_encode<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>, String> {
    let bytes = norito::encode_canonical(value).map_err(|e| e.to_string())?;
    if bytes.len() > KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1 {
        return Err("ordinary app archive bound exceeded".into());
    }
    Ok(bytes)
}
fn digest_original(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

struct BodyReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> BodyReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn read<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or("ordinary body length overflow")?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or("ordinary body truncated")?
            .try_into()
            .map_err(|_| "ordinary body field width differs")?;
        self.offset = end;
        Ok(value)
    }
}
fn decode_platform(tag: u8) -> Result<KagemushaHardwarePlatformClassV1, String> {
    match tag {
        1 => Ok(KagemushaHardwarePlatformClassV1::AndroidKeyMint),
        2 => Ok(KagemushaHardwarePlatformClassV1::AppleAppAttest),
        _ => Err("ordinary platform tag unknown".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        id::NetworkId,
        kagemusha::{
            KagemushaAppOperationApprovalEvidenceV1, KagemushaAppOperationApprovalPurposeV1,
            KagemushaAppOperationApprovalV1, KagemushaHardwareTransitionSelectionV1,
            KagemushaOperationKindV1,
        },
    };
    use iroha_crypto::{Hash, HashOf, KeyPair};
    use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

    struct Fixture {
        authority: KagemushaAppAttestationAuthorityPolicyV1,
        trust: KagemushaOrdinaryAppTrustPolicyV1,
        profile: KagemushaHardwareProfileV1,
        preparation: KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
        certificate: KagemushaOrdinaryAppCredentialV1,
        issuer: KeyPair,
        app: SigningKey,
    }
    fn fixture(apple: bool) -> Fixture {
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let app = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            app.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let class = if apple {
            KagemushaHardwarePlatformClassV1::AppleAppAttest
        } else {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint
        };
        let authority = KagemushaAppAttestationAuthorityPolicyV1 {
            authority_key: issuer.public_key().clone(),
            platform_class: class,
            app_signing_identity_digest: [2; 32],
            app_release_digest: [3; 32],
            maximum_lifetime_ms: 10000,
        };
        let trust = KagemushaOrdinaryAppTrustPolicyV1 {
            version: 1,
            app_authority_policy_digest: authority.canonical_digest().unwrap(),
            platform_class: class,
            allowed_android_security_levels: if apple {
                vec![]
            } else {
                vec![
                    KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment,
                    KagemushaAppKeySecurityLevelV1::StrongBox,
                ]
            },
            play_integrity_policy: None,
            maximum_credential_lifetime_ms: 10000,
        };
        let profile = KagemushaHardwareProfileV1 {
            version: 1,
            protocol_version: 1,
            hardware_profile_id: [0; 32],
            provider_id: [4; 32],
            platform_class: class,
            product_class_digest: [5; 32],
            firmware_policy_digest: trust.canonical_digest().unwrap(),
            enrollment_attestation_verifier_digest: [6; 32],
            attestation_trust_roots_digest: [7; 32],
            allowed_suite_commitment: [8; 32],
            policy_epoch: 1,
            governance_credential_public_key: key,
            capability_mask: class.required_guarantees(),
            qualification_report_digest: [9; 32],
            valid_from_ms: 1,
            expires_at_ms: 20000,
            app_attestation_authority_policy_digest: authority.canonical_digest().unwrap(),
        }
        .seal_hardware_profile_id()
        .unwrap();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([11; 32])));
        let challenge = KagemushaOrdinaryAppEnrollmentChallengeV1 {
            version: 1,
            platform_class: class,
            enrollment_id: [12; 32],
            client_nonce: [13; 32],
            server_nonce: [14; 32],
            account_binding: [15; 32],
            network_id: *network.as_bytes(),
            lane_id: [16; 32],
            release_id: [17; 32],
            hardware_profile_id: profile.hardware_profile_id,
            suite_id: [18; 32],
            trust_policy_digest: trust.canonical_digest().unwrap(),
            app_authority_policy_digest: authority.canonical_digest().unwrap(),
            financial_authority_commitment: [19; 32],
            issuer_policy_digest: [20; 32],
            policy_epoch: 1,
            hardware_epoch: 1,
            issued_at_ms: 100,
            expires_at_ms: 2000,
        };
        let preparation = KagemushaSignedOrdinaryAppEnrollmentChallengeV1 {
            challenge,
            signature: Signature::try_new(
                issuer.private_key(),
                &challenge.canonical_signing_bytes().unwrap(),
            )
            .unwrap(),
        };
        let subject = KagemushaOrdinaryAppCredentialSubjectV1 {
            version: 1,
            platform_class: class,
            security_level: if apple {
                KagemushaAppKeySecurityLevelV1::AppleAppAttest
            } else {
                KagemushaAppKeySecurityLevelV1::StrongBox
            },
            enrollment_id: challenge.enrollment_id,
            client_nonce: challenge.client_nonce,
            server_nonce: challenge.server_nonce,
            account_binding: challenge.account_binding,
            network_id: challenge.network_id,
            lane_id: challenge.lane_id,
            release_id: challenge.release_id,
            hardware_profile_id: challenge.hardware_profile_id,
            suite_id: challenge.suite_id,
            trust_policy_digest: challenge.trust_policy_digest,
            app_authority_policy_digest: challenge.app_authority_policy_digest,
            app_signing_identity_digest: authority.app_signing_identity_digest,
            app_release_digest: authority.app_release_digest,
            attested_key_id: Sha256::digest(key.as_sec1_bytes()).into(),
            app_key_reference: kagemusha_device_key_reference_v1(&key),
            financial_authority_commitment: challenge.financial_authority_commitment,
            platform_evidence_digest: [21; 32],
            enrollment_challenge_digest: challenge.attestation_challenge().unwrap(),
            app_public_key: key,
            policy_epoch: 1,
            hardware_epoch: 1,
            issued_at_ms: 200,
            expires_at_ms: 10200,
            app_attest_counter_floor: if apple { 5 } else { 0 },
            play_integrity: None,
        };
        let certificate = KagemushaOrdinaryAppCredentialV1 {
            subject,
            signature: Signature::try_new(
                issuer.private_key(),
                &subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap(),
        };
        Fixture {
            authority,
            trust,
            profile,
            preparation,
            certificate,
            issuer,
            app,
        }
    }
    fn admit(f: &Fixture, now: u64) -> Result<KagemushaVerifiedOrdinaryAppCredentialV1, String> {
        f.certificate.authenticate_originals(
            &f.profile,
            &f.trust,
            &f.authority,
            &f.preparation.challenge,
            &f.certificate.subject.app_public_key,
            now,
        )
    }
    fn resign(f: &mut Fixture) {
        f.certificate.signature = Signature::try_new(
            f.issuer.private_key(),
            &f.certificate.subject.canonical_signing_bytes().unwrap(),
        )
        .unwrap();
    }
    fn approval(
        f: &Fixture,
        credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    ) -> KagemushaAppOperationApprovalV1 {
        let c = credential.subject();
        let subject = KagemushaHardwareTransitionSelectionV1 {
            version: 1,
            release_id: c.release_id,
            provider_policy_root: [22; 32],
            app_policy_digest: credential.static_binding_digest(),
            credential_id: credential.digest(),
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::prehashed(c.network_id),
            )),
            lane_commitment: c.lane_id,
            hardware_profile_id: c.hardware_profile_id,
            policy_epoch: c.policy_epoch,
            hardware_epoch_id: [23; 32],
            hardware_epoch_generation: 1,
            operation_kind: KagemushaOperationKindV1::SendSplit,
            transition_statement_digest: [24; 32],
            candidate_envelope_digest: [25; 32],
            terminal_body_commitment: [26; 32],
            secure_index_before: 70,
            secure_index_after: 71,
        };
        let challenge = KagemushaAppOperationApprovalChallengeV1 {
            version: 1,
            purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
            operation_id: [27; 32],
            nonce: [28; 32],
            account_binding: c.account_binding,
            authority_policy_digest: c.app_authority_policy_digest,
            attested_key_id: c.attested_key_id,
            enrollment_digest: credential.digest(),
            subject_signing_digest: Sha256::digest(subject.canonical_signing_bytes().unwrap())
                .into(),
            normalized_guard_digest: [29; 32],
            issued_at_ms: 300,
            expires_at_ms: 900,
            subject,
        };
        let signature: P256Signature = f.app.sign(&challenge.canonical_signing_bytes().unwrap());
        KagemushaAppOperationApprovalV1 {
            challenge,
            evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: signature.to_der().as_bytes().to_vec(),
            },
        }
    }

    #[test]
    fn ordinary_exact_fixed_codecs_and_ed_original_roundtrip() {
        let f = fixture(false);
        let bytes = f.preparation.to_transport_bytes().unwrap();
        assert_eq!(bytes.len(), 515);
        let parsed =
            KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&bytes).unwrap();
        assert_eq!(parsed, f.preparation);
        parsed
            .authenticate(f.issuer.public_key(), &f.preparation.challenge, 200)
            .unwrap();
        let mut trailing = bytes;
        trailing.push(0);
        assert!(
            KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&trailing)
                .is_err()
        );
        let message = f.certificate.subject.canonical_signing_bytes().unwrap();
        assert_eq!(
            message.len(),
            KAGEMUSHA_ORDINARY_APP_CREDENTIAL_DOMAIN_V1.len() + 8 + 794
        );
        let body = &message[KAGEMUSHA_ORDINARY_APP_CREDENTIAL_DOMAIN_V1.len() + 8..];
        assert_eq!(
            KagemushaOrdinaryAppCredentialSubjectV1::from_signing_body(body).unwrap(),
            f.certificate.subject
        );
        let original = norito::encode_canonical(&f.certificate).unwrap();
        let decoded: KagemushaOrdinaryAppCredentialV1 =
            norito::decode_from_bytes(&original).unwrap();
        assert_eq!(decoded, f.certificate);
        assert_ne!(admit(&f, 300).unwrap().digest(), [0; 32]);
    }

    #[test]
    fn ordinary_credential_rejects_real_resigned_key_role_and_scope_substitution() {
        for field in 0..5 {
            let mut f = fixture(false);
            match field {
                0 => f.certificate.subject.financial_authority_commitment = [31; 32],
                1 => f.certificate.subject.account_binding = [32; 32],
                2 => f.certificate.subject.trust_policy_digest = [33; 32],
                3 => f.certificate.subject.enrollment_challenge_digest = [34; 32],
                _ => f.certificate.subject.hardware_epoch = 2,
            }
            resign(&mut f);
            assert!(admit(&f, 300).is_err(), "field{field}");
        }
        let mut f = fixture(false);
        f.authority.authority_key = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519)
            .public_key()
            .clone();
        assert!(admit(&f, 300).is_err());
    }

    #[test]
    fn ordinary_policy_permits_only_explicit_tee_and_strongbox_levels() {
        let mut f = fixture(false);
        f.certificate.subject.security_level =
            KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment;
        resign(&mut f);
        assert!(admit(&f, 300).is_ok());
        f.certificate.subject.security_level = KagemushaAppKeySecurityLevelV1::AppleAppAttest;
        resign(&mut f);
        assert!(admit(&f, 300).is_err());
        f.trust.allowed_android_security_levels.reverse();
        assert!(f.trust.validate().is_err());
        f.trust.allowed_android_security_levels = vec![
            KagemushaAppKeySecurityLevelV1::StrongBox,
            KagemushaAppKeySecurityLevelV1::StrongBox,
        ];
        assert!(f.trust.validate().is_err());
    }

    #[test]
    fn ordinary_integrity_refresh_is_separate_and_bound_to_preparation_and_key() {
        let mut f = fixture(false);
        f.trust.play_integrity_policy = Some(KagemushaPlayIntegrityPolicyV1 {
            policy_digest: [35; 32],
            maximum_evidence_age_ms: 100,
            maximum_refresh_interval_ms: 500,
            require_play_recognized: true,
            require_licensed: true,
            minimum_device_integrity: 1,
        });
        f.profile.firmware_policy_digest = f.trust.canonical_digest().unwrap();
        f.profile = f.profile.seal_hardware_profile_id().unwrap();
        f.preparation.challenge.hardware_profile_id = f.profile.hardware_profile_id;
        f.preparation.challenge.trust_policy_digest = f.trust.canonical_digest().unwrap();
        f.certificate.subject.hardware_profile_id = f.profile.hardware_profile_id;
        f.certificate.subject.trust_policy_digest = f.trust.canonical_digest().unwrap();
        f.certificate.subject.enrollment_challenge_digest =
            f.preparation.challenge.attestation_challenge().unwrap();
        f.certificate.subject.play_integrity = Some(KagemushaPlayIntegrityBindingV1 {
            request_hash: f
                .preparation
                .challenge
                .play_integrity_request_hash(f.certificate.subject.attested_key_id)
                .unwrap(),
            evidence_digest: [36; 32],
            policy_digest: [35; 32],
            verified_at_ms: 190,
            refresh_before_ms: 690,
        });
        resign(&mut f);
        let verified = admit(&f, 300).unwrap();
        assert!(verified.recheck_at_trusted_time(689).is_ok());
        assert!(verified.recheck_at_trusted_time(690).is_err());
        f.certificate
            .subject
            .play_integrity
            .as_mut()
            .unwrap()
            .request_hash = [37; 32];
        resign(&mut f);
        assert!(admit(&f, 300).is_err());
        f.certificate.subject.play_integrity = None;
        resign(&mut f);
        assert!(admit(&f, 300).is_err());
    }

    #[test]
    fn ordinary_original_approval_verifies_exact_android_signature_and_refuses_replay_substitution()
    {
        let f = fixture(false);
        let verified = admit(&f, 300).unwrap();
        let original = approval(&f, &verified);
        let approval = original
            .authenticate(&original.challenge, &verified, None, 400)
            .unwrap();
        assert_eq!(approval.app_attest_counter(), None);
        assert_eq!(
            approval.proof_binding_digest(),
            super::super::kagemusha_ordinary_app_approval_proof_binding_digest_v1(&original)
                .unwrap()
        );
        let mut changed_evidence = original.clone();
        let KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } =
            &mut changed_evidence.evidence
        else {
            unreachable!()
        };
        signature_der[5] ^= 1;
        assert_ne!(
            approval.proof_binding_digest(),
            super::super::kagemusha_ordinary_app_approval_proof_binding_digest_v1(
                &changed_evidence
            )
            .unwrap()
        );
        let KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } =
            &mut changed_evidence.evidence
        else {
            unreachable!()
        };
        signature_der.clear();
        assert!(
            super::super::kagemusha_ordinary_app_approval_proof_binding_digest_v1(
                &changed_evidence
            )
            .is_err()
        );

        let mut changed = original.clone();
        changed.challenge.nonce = [40; 32];
        assert!(
            changed
                .authenticate(&changed.challenge, &verified, None, 400)
                .is_err()
        );
        let mut changed = original.clone();
        changed.challenge.account_binding = [41; 32];
        assert!(
            changed
                .authenticate(&changed.challenge, &verified, None, 400)
                .is_err()
        );
        assert!(
            original
                .authenticate(&original.challenge, &verified, None, 900)
                .is_err()
        );
        assert!(
            original
                .authenticate(&original.challenge, &verified, Some(0), 400)
                .is_err()
        );
    }

    #[test]
    fn ordinary_apple_counter_is_separate_from_financial_exact_next_and_allows_skips() {
        let f = fixture(true);
        let verified = admit(&f, 300).unwrap();
        let mut original = approval(&f, &verified);
        let mut auth = vec![2u8; 32];
        auth.push(0x40);
        auth.extend_from_slice(&11u32.to_be_bytes());
        let mut hash = Sha256::new();
        hash.update(&auth);
        hash.update(Sha256::digest(
            original.challenge.canonical_signing_bytes().unwrap(),
        ));
        let nonce: [u8; 32] = hash.finalize().into();
        let signature: P256Signature = f.app.sign(&nonce);
        let der = signature.to_der();
        // Exact two-key canonical definite CBOR, produced only for this known-public fixture.
        let mut raw = vec![0xa2, 0x71];
        raw.extend_from_slice(b"authenticatorData");
        raw.extend_from_slice(&[0x58, 37]);
        raw.extend_from_slice(&auth);
        raw.push(0x69);
        raw.extend_from_slice(b"signature");
        raw.extend_from_slice(&[0x58, der.as_bytes().len() as u8]);
        raw.extend_from_slice(der.as_bytes());
        original.evidence =
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion: raw };
        let approved = original
            .authenticate(&original.challenge, &verified, Some(5), 400)
            .unwrap();
        assert_eq!(approved.app_attest_counter(), Some(11));
        assert_eq!(original.challenge.subject.secure_index_after, 71);
        assert!(
            original
                .authenticate(&original.challenge, &verified, Some(11), 400)
                .is_err()
        );
        assert!(
            original
                .authenticate(&original.challenge, &verified, None, 400)
                .is_err()
        );
    }
}
