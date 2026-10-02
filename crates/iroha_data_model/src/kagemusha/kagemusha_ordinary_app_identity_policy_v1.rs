//! Independently governed ordinary hardware identity policy, separate from monetary release.
//!
//! Threshold admission authenticates policy originals and planned correlation only. It does
//! not validate a monetary artifact catalog, assert hardware one-use/rollback, install a
//! native pending enrollment owner, activate identity or open a spending wallet. Trusted
//! roots must be selected from independent authenticated deployment/genesis originals before
//! reading the response; a caller-selected self-signed root grants no authority.

use super::{
    KagemushaAppAttestationAuthorityPolicyV1, KagemushaHardwarePlatformClassV1,
    KagemushaOrdinaryAppEnrollmentChallengeV1, KagemushaOrdinaryAppTrustPolicyV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, NetworkId};
use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Bound before decoding one policy, root configuration or complete signed policy original.
pub const KAGEMUSHA_ORDINARY_APP_IDENTITY_POLICY_MAX_BYTES_V1: usize = 16 * 1024;
/// Maximum independently selected threshold authorities and approvals.
pub const KAGEMUSHA_ORDINARY_APP_IDENTITY_POLICY_MAX_SIGNERS_V1: usize = 32;
const PROFILE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-app-identity-profile\0";
const POLICY_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-app-identity-policy\0";
const APPROVAL_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-app-identity-policy-approval\0";

/// Ordinary identity namespace and planned financial correlation; not a hardware catalog.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryAppIdentityProfileV1")]
pub struct KagemushaOrdinaryAppIdentityProfileV1 {
    /// Sole first-release policy version.
    pub version: u16,
    /// Domain-separated identity of this exact nonmonetary profile body.
    pub identity_profile_id: [u8; 32],
    /// Independently governed ordinary platform; only Android KeyMint or Apple App Attest.
    pub platform_class: KagemushaHardwarePlatformClassV1,
    /// Planned signed financial release coordinate; no release existence/qualification asserted.
    pub planned_release_id: [u8; 32],
    /// Planned signed monetary profile coordinate; no one-use/rollback/catalog asserted.
    pub planned_hardware_profile_id: [u8; 32],
    /// Planned signed monetary suite coordinate; no proof/key/artifact availability asserted.
    pub planned_suite_id: [u8; 32],
    /// Signed planned financial policy epoch; the ordinary governance identity is its policy digest.
    pub policy_epoch: u64,
    /// Exact original ordinary trust policy, including roots/revocation/distribution/Integrity.
    pub trust_policy_digest: [u8; 32],
    /// Exact original independent raw-attestation issuer/application policy.
    pub app_authority_policy_digest: [u8; 32],
    /// Original platform root set; not inferred from the returned raw certificate.
    pub platform_trust_roots_digest: [u8; 32],
    /// Inclusive policy activation under independently held time authority.
    pub valid_from_ms: u64,
    /// Exclusive policy deadline; retry cannot extend it.
    pub expires_at_ms: u64,
}
impl KagemushaOrdinaryAppIdentityProfileV1 {
    /// Compute the exact profile identity with its ID slot zeroed under a distinct domain.
    /// # Errors
    /// Rejects canonical encoding/resource failure; this does not authenticate the profile.
    pub fn expected_identity_profile_id(&self) -> Result<[u8; 32], String> {
        let mut body = *self;
        body.identity_profile_id = [0; 32];
        Ok(digest(PROFILE_DOMAIN, &bounded_encode(&body)?))
    }
    /// Set the data-only profile identity; no policy or hardware authority is granted.
    /// # Errors
    /// Rejects canonical encoding/resource failure.
    pub fn seal_identity_profile_id(mut self) -> Result<Self, String> {
        self.identity_profile_id = self.expected_identity_profile_id()?;
        Ok(self)
    }
    /// Check ordinary scope only. No hardware guarantee mask or monetary release is queried.
    /// # Errors
    /// Rejects missing/reserved selectors, another platform/version or original interval.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.policy_epoch == 0
            || self.valid_from_ms == 0
            || self.valid_from_ms >= self.expires_at_ms
            || !matches!(
                self.platform_class,
                KagemushaHardwarePlatformClassV1::AndroidKeyMint
                    | KagemushaHardwarePlatformClassV1::AppleAppAttest
            )
            || [
                self.identity_profile_id,
                self.planned_release_id,
                self.planned_hardware_profile_id,
                self.planned_suite_id,
                self.trust_policy_digest,
                self.app_authority_policy_digest,
                self.platform_trust_roots_digest,
            ]
            .contains(&[0; 32])
            || self.identity_profile_id != self.expected_identity_profile_id()?
        {
            return Err("ordinary identity profile incomplete or noncanonical".into());
        }
        Ok(())
    }
}

/// Independently held ordinary governance anchors; never accepted from a policy response.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::KagemushaOrdinaryAppIdentityAuthorityPolicyV1"
)]
pub struct KagemushaOrdinaryAppIdentityAuthorityPolicyV1 {
    /// Sole first-release root format.
    pub version: u16,
    /// Independent deployment/governance authority-set identity.
    pub authority_set_id: [u8; 32],
    /// Exact genesis-header-derived network held independently of the response.
    pub network_id: NetworkId,
    /// Exact accepted identity policy ID pinned by independent governance/configuration.
    pub expected_identity_policy_id: [u8; 32],
    /// Minimum distinct authorized Ed25519 approvals.
    pub threshold: u16,
    /// Strictly sorted unique independently authorized original Ed25519 keys.
    pub authorized_signers: Vec<PublicKey>,
}
impl KagemushaOrdinaryAppIdentityAuthorityPolicyV1 {
    /// Check the exact locally selected anchors. This does not authenticate their installation.
    /// # Errors
    /// Rejects missing pins, non-Ed25519/duplicate/unordered keys or impossible threshold/bounds.
    pub fn validate(&self) -> Result<(), String> {
        let count = self.authorized_signers.len();
        if self.version != 1
            || self.authority_set_id == [0; 32]
            || self.expected_identity_policy_id == [0; 32]
            || self.network_id.as_bytes() == &[0; 32]
            || count == 0
            || count > KAGEMUSHA_ORDINARY_APP_IDENTITY_POLICY_MAX_SIGNERS_V1
            || self.threshold == 0
            || usize::from(self.threshold) > count
            || !self.authorized_signers.windows(2).all(|p| p[0] < p[1])
            || self
                .authorized_signers
                .iter()
                .any(|k| k.algorithm() != Algorithm::Ed25519)
        {
            return Err("ordinary identity authority anchors invalid".into());
        }
        bounded_encode(self)?;
        Ok(())
    }
}

/// Complete ordinary policy original approved by independent governance, not hardware proof.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryAppIdentityPolicyV1")]
pub struct KagemushaOrdinaryAppIdentityPolicyV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Actual independently selected authority set approving this policy.
    pub authority_set_id: [u8; 32],
    /// Exact genesis-derived network, independently checked against root anchors.
    pub network_id: NetworkId,
    /// Ordinary namespace without any financial hardware capability requirement.
    pub profile: KagemushaOrdinaryAppIdentityProfileV1,
    /// Actual roots/revocation/environment/distribution and separate Play Integrity policy.
    pub trust: KagemushaOrdinaryAppTrustPolicyV1,
    /// Actual raw-platform attestation issuer, not the preparation issuer role.
    pub app_authority_key: PublicKey,
    /// Original authority platform; independently compared to the ordinary profile.
    pub app_authority_platform_class: KagemushaHardwarePlatformClassV1,
    /// Exact original application/RP signer digest under the actual attestation authority.
    pub app_signing_identity_digest: [u8; 32],
    /// Exact governance-authorized original app distribution/release digest.
    pub app_release_digest: [u8; 32],
    /// Actual original maximum lifetime of raw-attestation/credential assertions.
    pub app_authority_maximum_lifetime_ms: u64,
    /// Independent original Core preparation issuer key; distinct role from raw attestation.
    pub enrollment_issuer_key: PublicKey,
    /// Independent threshold-pinned P256 issuer admission key; no response or seed derives trust.
    pub enrollment_issuer_p256_key: super::KagemushaDevicePublicKeyV1,
    /// Exact Core enrollment issuer policy bound by C; not inferred from preparation.
    pub enrollment_issuer_policy_digest: [u8; 32],
}
impl KagemushaOrdinaryAppIdentityPolicyV1 {
    /// Reconstruct the existing authority model from all original threshold-signed fields.
    /// No new authority serializer/parser is introduced; its existing digest preimage owns
    /// the complete original key/platform/application/release/lifetime relation.
    #[must_use]
    pub fn app_authority(&self) -> KagemushaAppAttestationAuthorityPolicyV1 {
        KagemushaAppAttestationAuthorityPolicyV1 {
            authority_key: self.app_authority_key.clone(),
            platform_class: self.app_authority_platform_class,
            app_signing_identity_digest: self.app_signing_identity_digest,
            app_release_digest: self.app_release_digest,
            maximum_lifetime_ms: self.app_authority_maximum_lifetime_ms,
        }
    }
    /// Validate complete independently approved policy shape, without granting admission.
    /// # Errors
    /// Rejects any original profile/trust/authority/key/pin or resource mismatch.
    pub fn validate(&self) -> Result<(), String> {
        self.profile.validate()?;
        self.enrollment_issuer_p256_key
            .validate()
            .map_err(|error| error.to_string())?;
        self.trust
            .validate_for_identity_profile(&self.profile, &self.app_authority())?;
        if self.version != 1
            || self.authority_set_id == [0; 32]
            || self.network_id.as_bytes() == &[0; 32]
            || self.enrollment_issuer_key.algorithm() != Algorithm::Ed25519
            || self.enrollment_issuer_key == self.app_authority_key
            || self.enrollment_issuer_policy_digest == [0; 32]
        {
            return Err("ordinary identity policy incomplete".into());
        }
        bounded_encode(self)?;
        Ok(())
    }
    /// Compute the exact domain-separated identity to compare to independently pinned roots.
    /// # Errors
    /// Rejects malformed policy or canonical resource failure.
    pub fn canonical_digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        Ok(digest(POLICY_DOMAIN, &bounded_encode(self)?))
    }
    /// Exact threshold approval bytes; monetary release approvals cannot stand in for these.
    /// # Errors
    /// Rejects malformed policy or canonical resource failure.
    pub fn approval_signing_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let body = bounded_encode(self)?;
        let mut out = APPROVAL_DOMAIN.to_vec();
        out.extend_from_slice(&(body.len() as u64).to_le_bytes());
        out.extend_from_slice(&body);
        Ok(out)
    }
}

/// One original independent governance approval, never a platform or financial signature.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryAppIdentityPolicyApprovalV1")]
pub struct KagemushaOrdinaryAppIdentityPolicyApprovalV1 {
    /// Exact independently authorized signer.
    pub public_key: PublicKey,
    /// Original Ed25519 signature over the complete ordinary policy approval bytes.
    pub signature: Signature,
}
/// Complete bounded policy original and sorted threshold approvals.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaSignedOrdinaryAppIdentityPolicyV1")]
pub struct KagemushaSignedOrdinaryAppIdentityPolicyV1 {
    /// Exact approved policy; decoding does not install it as trusted configuration.
    pub policy: KagemushaOrdinaryAppIdentityPolicyV1,
    /// Strictly ordered unique actual approvals, bounded before signature work.
    pub approvals: Vec<KagemushaOrdinaryAppIdentityPolicyApprovalV1>,
}
/// Opaque threshold-verified ordinary policy originals; no monetary or native owner authority.
/// No constructor, decoder or Clone implementation exists outside checked admission.
pub struct KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1 {
    policy: KagemushaOrdinaryAppIdentityPolicyV1,
    policy_id: [u8; 32],
    original: Vec<u8>,
    authority_original: Vec<u8>,
    app_authority_original: Vec<u8>,
    authenticated_at_ms: u64,
}
impl KagemushaSignedOrdinaryAppIdentityPolicyV1 {
    /// Decode only the sole exact bounded canonical signed policy; no trust is granted.
    /// # Errors
    /// Rejects empty/oversized/noncanonical input before approvals can grant admission.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_ORDINARY_APP_IDENTITY_POLICY_MAX_BYTES_V1 {
            return Err("ordinary signed policy resource bound".into());
        }
        let value: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|e| e.to_string())?;
        value.policy.validate()?;
        if bounded_encode(&value)? != bytes {
            return Err("ordinary signed policy not canonical".into());
        }
        Ok(value)
    }
    /// Authenticate exact independent root pins and threshold originals under trusted time.
    /// Roots must come from installed governance/genesis originals, never this response.
    /// # Errors
    /// Rejects foreign network/policy/set, unknown/duplicate/insufficient/bad signatures or expiry.
    pub fn authenticate(
        &self,
        roots: &KagemushaOrdinaryAppIdentityAuthorityPolicyV1,
        now: u64,
    ) -> Result<KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1, String> {
        roots.validate()?;
        self.policy.validate()?;
        let profile = &self.policy.profile;
        let policy_id = self.policy.canonical_digest()?;
        if self.policy.authority_set_id != roots.authority_set_id
            || self.policy.network_id != roots.network_id
            || policy_id != roots.expected_identity_policy_id
            || now < profile.valid_from_ms
            || now >= profile.expires_at_ms
            || self.approvals.len() < usize::from(roots.threshold)
            || self.approvals.len() > roots.authorized_signers.len()
            || !self
                .approvals
                .windows(2)
                .all(|p| p[0].public_key < p[1].public_key)
        {
            return Err("ordinary identity policy root/threshold/time differs".into());
        }
        let original = bounded_encode(self)?;
        let message = self.policy.approval_signing_bytes()?;
        for approval in &self.approvals {
            if approval.public_key.algorithm() != Algorithm::Ed25519
                || approval.signature.payload().len() != 64
                || roots
                    .authorized_signers
                    .binary_search(&approval.public_key)
                    .is_err()
            {
                return Err("ordinary policy signer unauthorized".into());
            }
            approval
                .signature
                .verify(&approval.public_key, &message)
                .map_err(|_| "ordinary policy approval signature rejected")?;
        }
        Ok(KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1 {
            policy: self.policy.clone(),
            policy_id,
            original,
            authority_original: bounded_encode(roots)?,
            app_authority_original: self
                .policy
                .app_authority()
                .canonical_digest_preimage_v1()?
                .bytes,
            authenticated_at_ms: now,
        })
    }
}
impl KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1 {
    /// Borrow complete authenticated policy originals; immutable and nonmonetary.
    #[must_use]
    pub const fn policy(&self) -> &KagemushaOrdinaryAppIdentityPolicyV1 {
        &self.policy
    }
    /// Exact independently pinned policy identity, not a digest claimed by raw evidence.
    #[must_use]
    pub const fn policy_id(&self) -> [u8; 32] {
        self.policy_id
    }
    /// Original canonical threshold-signed policy, retained through the admission lifetime.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Original independently held authority anchors used by actual threshold admission.
    #[must_use]
    pub fn authority_original(&self) -> &[u8] {
        &self.authority_original
    }
    /// Exact existing authority digest preimage retained from every original signed field.
    #[must_use]
    pub fn app_authority_original(&self) -> &[u8] {
        &self.app_authority_original
    }
    /// Recheck the original interval and reject time regression; no renewal occurs.
    /// # Errors
    /// Rejects before admission/activation or at/after independently governed expiry.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<(), String> {
        if now < self.authenticated_at_ms
            || now < self.policy.profile.valid_from_ms
            || now >= self.policy.profile.expires_at_ms
        {
            return Err("ordinary identity policy expired".into());
        }
        Ok(())
    }
    /// Bind full original C to actual policy originals, without checking monetary qualification.
    /// Account/lane/financial commitment/epoch/nonces still come from the native original C.
    /// # Errors
    /// Rejects substituted issuer, network, planned scope, trust/platform or original interval.
    pub fn validate_challenge(
        &self,
        c: &KagemushaOrdinaryAppEnrollmentChallengeV1,
        now: u64,
    ) -> Result<(), String> {
        self.recheck_at_trusted_time(now)?;
        c.canonical_signing_bytes()?;
        let p = &self.policy.profile;
        if c.network_id != *self.policy.network_id.as_bytes()
            || c.platform_class != p.platform_class
            || c.release_id != p.planned_release_id
            || c.hardware_profile_id != p.planned_hardware_profile_id
            || c.suite_id != p.planned_suite_id
            || c.policy_epoch != p.policy_epoch
            || c.trust_policy_digest != p.trust_policy_digest
            || c.app_authority_policy_digest != p.app_authority_policy_digest
            || c.issuer_policy_digest != self.policy.enrollment_issuer_policy_digest
            || c.issued_at_ms < p.valid_from_ms
            || c.expires_at_ms > p.expires_at_ms
            || c.expires_at_ms - c.issued_at_ms > self.policy.app_authority_maximum_lifetime_ms
        {
            return Err("ordinary C differs from independently admitted identity policy".into());
        }
        Ok(())
    }
}
fn bounded_encode<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>, String> {
    let len = norito::canonical_frame_len(value).map_err(|e| e.to_string())?;
    if len > KAGEMUSHA_ORDINARY_APP_IDENTITY_POLICY_MAX_BYTES_V1 {
        return Err("ordinary policy resource bound".into());
    }
    norito::encode_canonical(value).map_err(|e| e.to_string())
}
fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(domain);
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    h.finalize().into()
}

/// Opaque issuer-authenticated full original C under actual independent identity policy.
/// No public constructor, decoder or Clone exists. This retains original preparation only;
/// native pending reservation/consumption and financial qualification are not performed.
pub struct KagemushaVerifiedOrdinaryAppEnrollmentPreparationV1 {
    challenge: KagemushaOrdinaryAppEnrollmentChallengeV1,
    policy_id: [u8; 32],
    identity_policy_original: Vec<u8>,
    identity_authority_original: Vec<u8>,
    original: Vec<u8>,
    authenticated_at_ms: u64,
}
impl KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1 {
    /// Authenticate actual signed C under the independently admitted Core issuer and scope.
    /// Expected C comes from the original native pending owner, not the returned preparation.
    /// # Errors
    /// Rejects foreign issuer/signature/full C/policy/time; never installs a native owner.
    pub fn authenticate_preparation(
        &self,
        preparation: &super::KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
        expected: &KagemushaOrdinaryAppEnrollmentChallengeV1,
        now: u64,
    ) -> Result<KagemushaVerifiedOrdinaryAppEnrollmentPreparationV1, String> {
        self.validate_challenge(expected, now)?;
        preparation.authenticate(&self.policy.enrollment_issuer_key, expected, now)?;
        Ok(KagemushaVerifiedOrdinaryAppEnrollmentPreparationV1 {
            challenge: *expected,
            policy_id: self.policy_id,
            identity_policy_original: self.original().to_vec(),
            identity_authority_original: self.authority_original().to_vec(),
            original: preparation.to_transport_bytes()?,
            authenticated_at_ms: now,
        })
    }
}
impl KagemushaVerifiedOrdinaryAppEnrollmentPreparationV1 {
    /// Exact issuer-authenticated full original C, not response-selected expected scope.
    #[must_use]
    pub const fn challenge(&self) -> &KagemushaOrdinaryAppEnrollmentChallengeV1 {
        &self.challenge
    }
    /// Complete original515 bytes including actual Core signature.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Original independently admitted identity policy ID.
    #[must_use]
    pub const fn policy_id(&self) -> [u8; 32] {
        self.policy_id
    }
    /// Pending original interval only; retry cannot renew or backdate it.
    /// # Errors
    /// Rejects before authentication/issue or at/after original expiry.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<(), String> {
        if now < self.authenticated_at_ms
            || now < self.challenge.issued_at_ms
            || now >= self.challenge.expires_at_ms
        {
            return Err("checked ordinary preparation expired".into());
        }
        Ok(())
    }
    /// Match previously authenticated full C to the exact still-current policy original.
    /// This preserves historical checked C for current signed credential readback, and does
    /// not renew pending C or authorize new E after its original expiry.
    /// # Errors
    /// Rejects policy substitutions, time regression or original policy/C scope mismatch.
    pub fn require_policy_original(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        now: u64,
    ) -> Result<(), String> {
        if self.policy_id != policy.policy_id
            || self.identity_policy_original != policy.original()
            || self.identity_authority_original != policy.authority_original()
            || now < self.authenticated_at_ms
        {
            return Err("checked ordinary preparation policy differs".into());
        }
        policy.validate_challenge(&self.challenge, now)
    }
}

/// Complete checked historical enrollment originals under an independently admitted current
/// ordinary policy. This result carries no pending preparation/raw/E owner or renewed lease.
/// It cannot be decoded, cloned, or used to prepare another enrollment possession statement.
pub struct KagemushaVerifiedHistoricalOrdinaryEnrollmentV1 {
    credential: super::KagemushaVerifiedOrdinaryAppCredentialV1,
    preparation_original: Vec<u8>,
    raw_admission_original: Vec<u8>,
    platform_original: Vec<u8>,
    possession_original: Vec<u8>,
    authenticated_at_ms: u64,
}
impl KagemushaVerifiedHistoricalOrdinaryEnrollmentV1 {
    /// Borrow the current issuer-authenticated ordinary credential. This is not a native
    /// identity/wallet owner; fresh native key possession and actual durable commit remain due.
    #[must_use]
    pub const fn credential(&self) -> &super::KagemushaVerifiedOrdinaryAppCredentialV1 {
        &self.credential
    }
    /// Exact original signed C515, retained without exposing a pending owner.
    #[must_use]
    pub fn preparation_original(&self) -> &[u8] {
        &self.preparation_original
    }
    /// Exact original signed raw314, retained without exposing a pending owner.
    #[must_use]
    pub fn raw_admission_original(&self) -> &[u8] {
        &self.raw_admission_original
    }
    /// Complete canonical platform container whose untouched originals were joined at issuance.
    #[must_use]
    pub fn platform_original(&self) -> &[u8] {
        &self.platform_original
    }
    /// Exact original canonical E possession archive; this cannot sign or renew another E.
    #[must_use]
    pub fn possession_original(&self) -> &[u8] {
        &self.possession_original
    }
    /// Recheck current policy/credential lifetime and reject trusted-time regression.
    /// # Errors
    /// Rejects a changed policy/root original, expired current credential/Integrity or regression.
    pub fn recheck_current(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        now: u64,
    ) -> Result<(), String> {
        if now < self.authenticated_at_ms
            || self.credential.identity_policy_id() != policy.policy_id()
            || self.credential.identity_policy_original() != policy.original()
            || self.credential.identity_authority_original() != policy.authority_original()
        {
            return Err("historical enrollment current policy/original time differs".into());
        }
        policy.recheck_at_trusted_time(now)?;
        self.credential.recheck_at_trusted_time(now)
    }
}
/// Authenticated archived C data for exact receipt identification only.
/// It has no decoder, Clone, pending/current/financial owner or lifetime renewal API.
pub struct KagemushaArchivedOrdinaryPreparationOriginalDataV1 {
    challenge: KagemushaOrdinaryAppEnrollmentChallengeV1,
    original: Vec<u8>,
}
impl KagemushaArchivedOrdinaryPreparationOriginalDataV1 {
    /// Borrow immutable signed subject data; this never grants pending authority.
    #[must_use]
    pub const fn challenge(&self) -> &KagemushaOrdinaryAppEnrollmentChallengeV1 {
        &self.challenge
    }
    /// Complete exact signed transport original for receipt matching only.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
}

/// Complete archived enrollment authenticity data for exact receipt lookup.
/// Its mathematical evaluation view and checked history stay private. No API returns
/// an expired checked credential, current-state owner, native identity or issuance grant.
pub struct KagemushaArchivedOrdinaryEnrollmentOriginalDataV1 {
    history: KagemushaVerifiedHistoricalOrdinaryEnrollmentV1,
    preparation: KagemushaVerifiedOrdinaryAppEnrollmentPreparationV1,
    evaluation_policy: KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
}
impl KagemushaArchivedOrdinaryEnrollmentOriginalDataV1 {
    /// Immutable original credential subject data, without a current credential owner.
    #[must_use]
    pub fn credential_subject(&self) -> &super::KagemushaOrdinaryAppCredentialSubjectV1 {
        self.history.credential.subject()
    }
    /// Exact original credential bytes, without current publication permission.
    #[must_use]
    pub fn credential_original(&self) -> &[u8] {
        self.history.credential.original()
    }
    /// Verify an archived signed reply's complete original joins and original interval.
    /// The Core signature is checked before its own issue instant is used inside the
    /// closed mathematical evaluation. No caller timestamp or current owner escapes.
    /// # Errors
    /// Rejects wrong complete issuer/namespace/cap, Core role/signature, nonce, floor,
    /// originals or original interval. No historical evaluation returns a current owner.
    pub fn authenticate_current_reply_original_data(
        &self,
        issuer: &super::KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1,
        independently_installed_lane_namespace: [u8; 32],
        reply_original: &[u8],
        expected_native_query_nonce: &[u8; 32],
        minimum_native_state_epoch: u64,
    ) -> Result<(), String> {
        issuer.authenticate_archived_current_reply_original_data(
            &self.evaluation_policy,
            independently_installed_lane_namespace,
            &self.history,
            reply_original,
            expected_native_query_nonce,
            minimum_native_state_epoch,
        )
    }
}
impl KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1 {
    /// Authenticate complete archived signed C data under the same checked installed roots.
    /// Its signature is verified before the original signed issue instant is evaluated.
    /// Current expiry is deliberately not converted into pending publication authority.
    /// # Errors
    /// Rejects signature, exact expected C, full policy joins or original signed interval.
    pub fn authenticate_archived_preparation_original_data(
        &self,
        preparation_original: &[u8],
        expected_c: &KagemushaOrdinaryAppEnrollmentChallengeV1,
    ) -> Result<KagemushaArchivedOrdinaryPreparationOriginalDataV1, String> {
        let preparation =
            super::KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(
                preparation_original,
            )?;
        preparation
            .signature
            .verify(
                &self.policy.enrollment_issuer_key,
                &preparation.challenge.canonical_signing_bytes()?,
            )
            .map_err(|_| "archived C Core signature rejected")?;
        let evaluation = Self {
            policy: self.policy.clone(),
            policy_id: self.policy_id,
            original: self.original.clone(),
            authority_original: self.authority_original.clone(),
            app_authority_original: self.app_authority_original.clone(),
            authenticated_at_ms: preparation.challenge.issued_at_ms,
        };
        let checked = evaluation.authenticate_preparation(
            &preparation,
            expected_c,
            preparation.challenge.issued_at_ms,
        )?;
        Ok(KagemushaArchivedOrdinaryPreparationOriginalDataV1 {
            challenge: *checked.challenge(),
            original: checked.original().to_vec(),
        })
    }
    /// Authenticate every archived C/raw/container/E/credential original and signature.
    /// Reuses the sole historical enrollment mathematics. The legitimate App credential
    /// signature authenticates the original evaluation instant before it is used; there
    /// is no caller time argument. The result contains data only for exact receipt lookup.
    /// # Errors
    /// Rejects any complete original, role/key/signature, counter or original interval drift.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate_archived_enrollment_original_data(
        &self,
        preparation_original: &[u8],
        expected_native_c: &KagemushaOrdinaryAppEnrollmentChallengeV1,
        raw_admission_original: &[u8],
        platform_original: &[u8],
        possession_original: &[u8],
        credential_original: &[u8],
        independently_selected_key: &super::KagemushaDevicePublicKeyV1,
    ) -> Result<KagemushaArchivedOrdinaryEnrollmentOriginalDataV1, String> {
        let credential =
            super::KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(credential_original)?;
        let authority = self.policy.app_authority();
        credential
            .signature
            .verify(
                &authority.authority_key,
                &credential.subject.canonical_signing_bytes()?,
            )
            .map_err(|_| "historical credential issuer signature rejected")?;
        let historical_ms = credential.subject.issued_at_ms;
        // This private local evaluation view retains the SAME complete threshold/root originals.
        // It is never returned and cannot install an older policy or grant current pending work.
        // Its timestamp is the already authenticated original credential issue time, not a
        // constructor argument or an unsigned local journal timestamp.
        let original_evaluation = Self {
            policy: self.policy.clone(),
            policy_id: self.policy_id,
            original: self.original.clone(),
            authority_original: self.authority_original.clone(),
            app_authority_original: self.app_authority_original.clone(),
            authenticated_at_ms: historical_ms,
        };
        let preparation =
            super::KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(
                preparation_original,
            )?;
        let checked_c = original_evaluation.authenticate_preparation(
            &preparation,
            expected_native_c,
            historical_ms,
        )?;
        let raw = super::KagemushaRawAppAttestationAdmissionV1::from_transport_bytes(
            raw_admission_original,
        )?;
        let checked_raw = raw.authenticate(
            &original_evaluation,
            &checked_c,
            platform_original,
            historical_ms,
        )?;
        if checked_raw.subject().app_public_key != *independently_selected_key {
            return Err("historical enrollment selected key differs".into());
        }
        let proof =
            super::KagemushaAppEnrollmentPossessionV1::decode_canonical_exact(possession_original)?;
        let checked_e = proof.authenticate(
            &preparation,
            &self.policy.enrollment_issuer_key,
            expected_native_c,
            &checked_raw,
            platform_original,
            historical_ms,
        )?;
        let original_credential = credential.authenticate(
            &original_evaluation,
            &checked_c,
            independently_selected_key,
            historical_ms,
        )?;
        checked_e.bind_credential(&original_credential, historical_ms)?;
        let history = KagemushaVerifiedHistoricalOrdinaryEnrollmentV1 {
            credential: original_credential,
            preparation_original: preparation_original.to_vec(),
            raw_admission_original: raw_admission_original.to_vec(),
            platform_original: platform_original.to_vec(),
            possession_original: possession_original.to_vec(),
            authenticated_at_ms: historical_ms,
        };
        history.recheck_current(&original_evaluation, historical_ms)?;
        Ok(KagemushaArchivedOrdinaryEnrollmentOriginalDataV1 {
            history,
            preparation: checked_c,
            evaluation_policy: original_evaluation,
        })
    }
    /// Reauthenticate all complete enrollment originals for committed native restart.
    /// Archived authenticity remains private data; actual current policy, credential,
    /// Play refresh and trusted-time checks remain mandatory before a history owner exists.
    /// # Errors
    /// Rejects complete original drift, original mathematics or current expiry/regression.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate_historical_enrollment_originals(
        &self,
        preparation_original: &[u8],
        expected_native_c: &KagemushaOrdinaryAppEnrollmentChallengeV1,
        raw_admission_original: &[u8],
        platform_original: &[u8],
        possession_original: &[u8],
        credential_original: &[u8],
        independently_selected_key: &super::KagemushaDevicePublicKeyV1,
        trusted_now_ms: u64,
    ) -> Result<KagemushaVerifiedHistoricalOrdinaryEnrollmentV1, String> {
        self.recheck_at_trusted_time(trusted_now_ms)?;
        let data = self.authenticate_archived_enrollment_original_data(
            preparation_original,
            expected_native_c,
            raw_admission_original,
            platform_original,
            possession_original,
            credential_original,
            independently_selected_key,
        )?;
        if data.credential_subject().issued_at_ms > trusted_now_ms {
            return Err("historical credential issue time is in the future".into());
        }
        let credential =
            super::KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(credential_original)?;
        let current_credential = credential.authenticate(
            self,
            &data.preparation,
            independently_selected_key,
            trusted_now_ms,
        )?;
        let result = KagemushaVerifiedHistoricalOrdinaryEnrollmentV1 {
            credential: current_credential,
            preparation_original: preparation_original.to_vec(),
            raw_admission_original: raw_admission_original.to_vec(),
            platform_original: platform_original.to_vec(),
            possession_original: possession_original.to_vec(),
            authenticated_at_ms: trusted_now_ms,
        };
        result.recheck_current(self, trusted_now_ms)?;
        Ok(result)
    }
}

#[cfg(test)]
pub(super) fn identity_fixture_profile(
    profile: &super::KagemushaHardwareProfileV1,
    c: &KagemushaOrdinaryAppEnrollmentChallengeV1,
) -> KagemushaOrdinaryAppIdentityProfileV1 {
    // Preserve existing signed wire goldens: the old synthetic profile is a fixture source
    // of planned coordinates only. No monetary validation/catalog/provider is invoked.
    KagemushaOrdinaryAppIdentityProfileV1 {
        version: 1,
        identity_profile_id: [0; 32],
        platform_class: profile.platform_class,
        planned_release_id: c.release_id,
        planned_hardware_profile_id: profile.hardware_profile_id,
        planned_suite_id: c.suite_id,
        policy_epoch: profile.policy_epoch,
        trust_policy_digest: profile.firmware_policy_digest,
        app_authority_policy_digest: profile.app_attestation_authority_policy_digest,
        platform_trust_roots_digest: profile.attestation_trust_roots_digest,
        valid_from_ms: profile.valid_from_ms,
        expires_at_ms: profile.expires_at_ms,
    }
    .seal_identity_profile_id()
    .unwrap()
}
#[cfg(test)]
pub(super) fn identity_fixture_policy(
    profile: &super::KagemushaHardwareProfileV1,
    trust: &KagemushaOrdinaryAppTrustPolicyV1,
    authority: &KagemushaAppAttestationAuthorityPolicyV1,
    c: &KagemushaOrdinaryAppEnrollmentChallengeV1,
    core_issuer_key: &PublicKey,
    now: u64,
) -> Result<KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1, String> {
    // Genuine model cryptography on explicit synthetic governance originals; no device,
    // raw PKIX/Play, installed governance or monetary/native qualification is claimed.
    let one = iroha_crypto::KeyPair::from_seed(vec![81; 32], Algorithm::Ed25519);
    let two = iroha_crypto::KeyPair::from_seed(vec![82; 32], Algorithm::Ed25519);
    let policy = KagemushaOrdinaryAppIdentityPolicyV1 {
        version: 1,
        authority_set_id: [80; 32],
        network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed(c.network_id),
        )),
        profile: identity_fixture_profile(profile, c),
        trust: trust.clone(),
        app_authority_key: authority.authority_key.clone(),
        app_authority_platform_class: authority.platform_class,
        app_signing_identity_digest: authority.app_signing_identity_digest,
        app_release_digest: authority.app_release_digest,
        app_authority_maximum_lifetime_ms: authority.maximum_lifetime_ms,
        enrollment_issuer_key: core_issuer_key.clone(),
        enrollment_issuer_p256_key: profile.governance_credential_public_key,
        enrollment_issuer_policy_digest: c.issuer_policy_digest,
    };
    let message = policy.approval_signing_bytes()?;
    let mut approvals = vec![
        KagemushaOrdinaryAppIdentityPolicyApprovalV1 {
            public_key: one.public_key().clone(),
            signature: Signature::try_new(one.private_key(), &message)
                .map_err(|e| e.to_string())?,
        },
        KagemushaOrdinaryAppIdentityPolicyApprovalV1 {
            public_key: two.public_key().clone(),
            signature: Signature::try_new(two.private_key(), &message)
                .map_err(|e| e.to_string())?,
        },
    ];
    approvals.sort_by(|a, b| a.public_key.cmp(&b.public_key));
    let roots = KagemushaOrdinaryAppIdentityAuthorityPolicyV1 {
        version: 1,
        authority_set_id: policy.authority_set_id,
        network_id: policy.network_id,
        expected_identity_policy_id: policy.canonical_digest()?,
        threshold: 2,
        authorized_signers: approvals.iter().map(|a| a.public_key.clone()).collect(),
    };
    KagemushaSignedOrdinaryAppIdentityPolicyV1 { policy, approvals }.authenticate(&roots, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha::*;
    use iroha_crypto::{Hash, HashOf, KeyPair};
    use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

    struct Fixture {
        signed: KagemushaSignedOrdinaryAppIdentityPolicyV1,
        roots: KagemushaOrdinaryAppIdentityAuthorityPolicyV1,
        signers: [KeyPair; 2],
        issuer: KeyPair,
        raw_issuer: KeyPair,
        app: SigningKey,
        preparation: KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
    }
    fn fixture() -> Fixture {
        // Synthetic governance/issuer/raw originals with real math; no actual raw platform,
        // policy deployment, financial qualification, physical key or native owner is claimed.
        let signers = [
            KeyPair::from_seed(vec![81; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![82; 32], Algorithm::Ed25519),
        ];
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let raw_issuer = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let app = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let authority = KagemushaAppAttestationAuthorityPolicyV1 {
            authority_key: raw_issuer.public_key().clone(),
            platform_class: KagemushaHardwarePlatformClassV1::AndroidKeyMint,
            app_signing_identity_digest: [2; 32],
            app_release_digest: [3; 32],
            maximum_lifetime_ms: 10000,
        };
        let trust = KagemushaOrdinaryAppTrustPolicyV1 {
            version: 1,
            app_authority_policy_digest: authority.canonical_digest().unwrap(),
            platform_class: authority.platform_class,
            distribution: KagemushaOrdinaryAppDistributionV1::Development,
            apple_environment: None,
            platform_trust_roots_digest: [7; 32],
            platform_revocation_policy_digest: [44; 32],
            allowed_android_security_levels: vec![
                KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment,
                KagemushaAppKeySecurityLevelV1::StrongBox,
            ],
            play_integrity_policy: None,
            maximum_credential_lifetime_ms: 10000,
        };
        // These planned identifiers deliberately have no monetary manifest/profile/artifact.
        // This profile has no usage405/rollback303/one-use/guarantee/provider/OEM fields.
        let profile = KagemushaOrdinaryAppIdentityProfileV1 {
            version: 1,
            identity_profile_id: [0; 32],
            platform_class: authority.platform_class,
            planned_release_id: [17; 32],
            planned_hardware_profile_id: [27; 32],
            planned_suite_id: [18; 32],
            policy_epoch: 1,
            trust_policy_digest: trust.canonical_digest().unwrap(),
            app_authority_policy_digest: authority.canonical_digest().unwrap(),
            platform_trust_roots_digest: trust.platform_trust_roots_digest,
            valid_from_ms: 1,
            expires_at_ms: 20000,
        }
        .seal_identity_profile_id()
        .unwrap();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([11; 32])));
        let policy = KagemushaOrdinaryAppIdentityPolicyV1 {
            version: 1,
            authority_set_id: [80; 32],
            network_id: network,
            profile,
            trust,
            app_authority_key: authority.authority_key.clone(),
            app_authority_platform_class: authority.platform_class,
            app_signing_identity_digest: authority.app_signing_identity_digest,
            app_release_digest: authority.app_release_digest,
            app_authority_maximum_lifetime_ms: authority.maximum_lifetime_ms,
            enrollment_issuer_key: issuer.public_key().clone(),
            enrollment_issuer_p256_key:
                crate::testing::ordinary_app_enrollment::ordinary_test_issuer_public_key_v1(),
            enrollment_issuer_policy_digest: [20; 32],
        };
        let message = policy.approval_signing_bytes().unwrap();
        let mut approvals: Vec<_> = signers
            .iter()
            .map(|key| KagemushaOrdinaryAppIdentityPolicyApprovalV1 {
                public_key: key.public_key().clone(),
                signature: Signature::try_new(key.private_key(), &message).unwrap(),
            })
            .collect();
        approvals.sort_by(|a, b| a.public_key.cmp(&b.public_key));
        let roots = KagemushaOrdinaryAppIdentityAuthorityPolicyV1 {
            version: 1,
            authority_set_id: policy.authority_set_id,
            network_id: network,
            expected_identity_policy_id: policy.canonical_digest().unwrap(),
            threshold: 2,
            authorized_signers: approvals.iter().map(|a| a.public_key.clone()).collect(),
        };
        let c = KagemushaOrdinaryAppEnrollmentChallengeV1 {
            version: 1,
            platform_class: profile.platform_class,
            enrollment_id: [12; 32],
            client_nonce: [13; 32],
            server_nonce: [14; 32],
            account_binding: [15; 32],
            network_id: *network.as_bytes(),
            lane_id: [16; 32],
            release_id: profile.planned_release_id,
            hardware_profile_id: profile.planned_hardware_profile_id,
            suite_id: profile.planned_suite_id,
            trust_policy_digest: profile.trust_policy_digest,
            app_authority_policy_digest: profile.app_authority_policy_digest,
            financial_authority_commitment: [19; 32],
            issuer_policy_digest: policy.enrollment_issuer_policy_digest,
            policy_epoch: profile.policy_epoch,
            hardware_epoch: 1,
            issued_at_ms: 100,
            expires_at_ms: 2000,
        };
        let preparation = KagemushaSignedOrdinaryAppEnrollmentChallengeV1 {
            challenge: c,
            signature: Signature::try_new(
                issuer.private_key(),
                &c.canonical_signing_bytes().unwrap(),
            )
            .unwrap(),
        };
        Fixture {
            signed: KagemushaSignedOrdinaryAppIdentityPolicyV1 { policy, approvals },
            roots,
            signers,
            issuer,
            raw_issuer,
            app,
            preparation,
        }
    }
    fn checked(f: &Fixture) -> KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1 {
        f.signed.authenticate(&f.roots, 100).unwrap()
    }
    fn sign_policy(
        f: &Fixture,
        policy: KagemushaOrdinaryAppIdentityPolicyV1,
    ) -> KagemushaSignedOrdinaryAppIdentityPolicyV1 {
        let message = policy.approval_signing_bytes().unwrap();
        let mut approvals: Vec<_> = f
            .signers
            .iter()
            .map(|key| KagemushaOrdinaryAppIdentityPolicyApprovalV1 {
                public_key: key.public_key().clone(),
                signature: Signature::try_new(key.private_key(), &message).unwrap(),
            })
            .collect();
        approvals.sort_by(|a, b| a.public_key.cmp(&b.public_key));
        KagemushaSignedOrdinaryAppIdentityPolicyV1 { policy, approvals }
    }
    #[test]
    fn identity_policy_public_constituents_roundtrip_complete_p256_and_threshold_originals() {
        fn roundtrip<T>(value: &T)
        where
            T: core::fmt::Debug
                + PartialEq
                + norito::NoritoSerialize
                + norito::json::JsonSerialize
                + norito::json::JsonDeserialize,
            for<'de> T: norito::NoritoDeserialize<'de>,
        {
            let original = norito::encode_canonical(value).unwrap();
            let decoded: T = norito::decode_canonical_with_limits(
                &original,
                norito::canonical_decode_limits(original.len()),
            )
            .unwrap();
            assert_eq!(&decoded, value);
            let json = norito::json::to_json(value).unwrap();
            let json_decoded: T = norito::json::from_str(&json).unwrap();
            assert_eq!(&json_decoded, value);
            assert_eq!(norito::encode_canonical(&json_decoded).unwrap(), original);
            assert!(norito::json::to_json_bounded(value, json.len() - 1).is_err());
        }
        let f = fixture();
        roundtrip(&f.signed.policy.profile);
        roundtrip(&f.roots);
        roundtrip(&f.signed.policy);
        roundtrip(&f.signed.approvals[0]);
        roundtrip(&f.signed);
        let complete = norito::encode_canonical(&f.signed).unwrap();
        let decoded =
            KagemushaSignedOrdinaryAppIdentityPolicyV1::decode_canonical_exact(&complete).unwrap();
        let checked = decoded.authenticate(&f.roots, 100).unwrap();
        assert_eq!(checked.original(), complete);
        assert_eq!(
            checked.policy().enrollment_issuer_p256_key,
            f.signed.policy.enrollment_issuer_p256_key
        );
        assert!(
            decoded
                .authenticate(&f.roots, f.signed.policy.profile.expires_at_ms)
                .is_err()
        );
    }
    #[test]
    fn identity_policy_real_threshold_and_exact_archive_are_nonmonetary() {
        let f = fixture();
        let owner = checked(&f);
        assert_eq!(owner.policy_id(), f.roots.expected_identity_policy_id);
        let archive = bounded_encode(&f.signed).unwrap();
        let decoded =
            KagemushaSignedOrdinaryAppIdentityPolicyV1::decode_canonical_exact(&archive).unwrap();
        assert_eq!(decoded, f.signed);
        assert_eq!(owner.original(), archive);
        assert!(!owner.authority_original().is_empty());
        assert_eq!(
            owner.app_authority_original(),
            f.signed
                .policy
                .app_authority()
                .canonical_digest_preimage_v1()
                .unwrap()
                .bytes
        );
        assert_eq!(
            Sha256::digest(owner.app_authority_original()).as_slice(),
            f.signed.policy.profile.app_authority_policy_digest
        );
        let c = owner
            .authenticate_preparation(&f.preparation, &f.preparation.challenge, 100)
            .unwrap();
        assert_eq!(c.original().len(), 515);
        assert_eq!(c.original(), f.preparation.to_transport_bytes().unwrap());
        assert_eq!(c.challenge().canonical_signing_bytes().unwrap().len(), 512);
        assert!(c.require_policy_original(&owner, 3000).is_ok());
        assert!(c.recheck_at_trusted_time(3000).is_err());
    }
    #[test]
    fn checked_preparation_requires_exact_root_and_policy_approval_originals() {
        let f = fixture();
        let owner_a = checked(&f);
        let c_a = owner_a
            .authenticate_preparation(&f.preparation, &f.preparation.challenge, 100)
            .unwrap();
        let sign_with = |keys: &[KeyPair]| {
            let message = f.signed.policy.approval_signing_bytes().unwrap();
            let mut approvals: Vec<_> = keys
                .iter()
                .map(|key| KagemushaOrdinaryAppIdentityPolicyApprovalV1 {
                    public_key: key.public_key().clone(),
                    signature: Signature::try_new(key.private_key(), &message).unwrap(),
                })
                .collect();
            approvals.sort_by(|a, b| a.public_key.cmp(&b.public_key));
            KagemushaSignedOrdinaryAppIdentityPolicyV1 {
                policy: f.signed.policy.clone(),
                approvals,
            }
        };
        // Real, distinct governance-original fixtures authenticate the exact same policy body.
        // Their common set label and body digest cannot replace the held authority originals.
        let other_roots_keys = [
            KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![100; 32], Algorithm::Ed25519),
        ];
        let signed_b = sign_with(&other_roots_keys);
        let mut roots_b = f.roots.clone();
        roots_b.authorized_signers = signed_b
            .approvals
            .iter()
            .map(|approval| approval.public_key.clone())
            .collect();
        let owner_b = signed_b.authenticate(&roots_b, 100).unwrap();
        assert_eq!(owner_a.policy_id(), owner_b.policy_id());
        assert_eq!(owner_a.policy(), owner_b.policy());
        assert_ne!(owner_a.authority_original(), owner_b.authority_original());
        assert!(c_a.require_policy_original(&owner_b, 100).is_err());
        assert!(c_a.require_policy_original(&owner_a, 100).is_ok());

        // Even one identical independently held root configuration does not replace the
        // complete actual approved original with another mathematically valid approval subset.
        let third = KeyPair::from_seed(vec![83; 32], Algorithm::Ed25519);
        let mut roots_c = f.roots.clone();
        roots_c.authorized_signers.push(third.public_key().clone());
        roots_c.authorized_signers.sort();
        let owner_c = f.signed.authenticate(&roots_c, 100).unwrap();
        let c_c = owner_c
            .authenticate_preparation(&f.preparation, &f.preparation.challenge, 100)
            .unwrap();
        let signed_d = sign_with(&[f.signers[0].clone(), third]);
        let owner_d = signed_d.authenticate(&roots_c, 100).unwrap();
        assert_eq!(owner_c.policy_id(), owner_d.policy_id());
        assert_eq!(owner_c.authority_original(), owner_d.authority_original());
        assert_ne!(owner_c.original(), owner_d.original());
        assert!(c_c.require_policy_original(&owner_d, 100).is_err());
        assert!(c_c.require_policy_original(&owner_c, 100).is_ok());
        assert!(c_c.require_policy_original(&owner_c, 99).is_err());
    }

    #[test]
    fn identity_policy_threshold_order_unknown_keys_and_domains_reject() {
        let f = fixture();
        let mut bad = f.signed.clone();
        bad.approvals.pop();
        assert!(bad.authenticate(&f.roots, 100).is_err());
        bad = f.signed.clone();
        bad.approvals.reverse();
        assert!(bad.authenticate(&f.roots, 100).is_err());
        bad = f.signed.clone();
        bad.approvals[1] = bad.approvals[0].clone();
        assert!(bad.authenticate(&f.roots, 100).is_err());
        let foreign = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519);
        bad = f.signed.clone();
        bad.approvals[0] = KagemushaOrdinaryAppIdentityPolicyApprovalV1 {
            public_key: foreign.public_key().clone(),
            signature: Signature::try_new(
                foreign.private_key(),
                &bad.policy.approval_signing_bytes().unwrap(),
            )
            .unwrap(),
        };
        bad.approvals
            .sort_by(|a, b| a.public_key.cmp(&b.public_key));
        assert!(bad.authenticate(&f.roots, 100).is_err());
        bad = f.signed.clone();
        let key = f
            .signers
            .iter()
            .find(|k| k.public_key() == &bad.approvals[0].public_key)
            .unwrap();
        let mut wrong_domain = bad.policy.approval_signing_bytes().unwrap();
        wrong_domain[0] ^= 1;
        bad.approvals[0].signature = Signature::try_new(key.private_key(), &wrong_domain).unwrap();
        assert!(bad.authenticate(&f.roots, 100).is_err());
        bad = f.signed.clone();
        bad.approvals[0].signature = Signature::from_bytes(&[0; 63]);
        assert!(bad.authenticate(&f.roots, 100).is_err());
    }
    #[test]
    fn identity_policy_actual_root_pins_and_resigned_selector_mutations_reject() {
        let f = fixture();
        for i in 0..7 {
            let mut policy = f.signed.policy.clone();
            match i {
                0 => {
                    policy.network_id = NetworkId::from_genesis_hash(
                        HashOf::from_untyped_unchecked(Hash::prehashed([77; 32])),
                    )
                }
                1 => policy.authority_set_id = [77; 32],
                2 => {
                    policy.enrollment_issuer_key =
                        KeyPair::from_seed(vec![77; 32], Algorithm::Ed25519)
                            .public_key()
                            .clone()
                }
                3 => policy.enrollment_issuer_policy_digest = [77; 32],
                4 => policy.profile.planned_release_id = [77; 32],
                5 => policy.profile.planned_hardware_profile_id = [77; 32],
                _ => policy.profile.planned_suite_id = [77; 32],
            };
            policy.profile = policy.profile.seal_identity_profile_id().unwrap();
            assert!(sign_policy(&f, policy).authenticate(&f.roots, 100).is_err());
        }
        for i in 0..5 {
            let mut roots = f.roots.clone();
            match i {
                0 => roots.expected_identity_policy_id = [77; 32],
                1 => roots.authority_set_id = [77; 32],
                2 => {
                    roots.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                        Hash::prehashed([77; 32]),
                    ))
                }
                3 => roots.threshold = 0,
                _ => roots.authorized_signers.reverse(),
            };
            assert!(f.signed.authenticate(&roots, 100).is_err());
        }
        let mut roots = f.roots.clone();
        roots.authorized_signers = vec![roots.authorized_signers[0].clone(); 33];
        assert!(roots.validate().is_err());
    }
    #[test]
    fn identity_policy_original_time_and_bounded_codec_reject_renewal_tails() {
        let f = fixture();
        assert!(f.signed.authenticate(&f.roots, 0).is_err());
        assert!(f.signed.authenticate(&f.roots, 20000).is_err());
        let owner = checked(&f);
        assert!(owner.recheck_at_trusted_time(99).is_err());
        assert!(owner.recheck_at_trusted_time(20000).is_err());
        let mut bytes = bounded_encode(&f.signed).unwrap();
        bytes.push(0);
        assert!(
            KagemushaSignedOrdinaryAppIdentityPolicyV1::decode_canonical_exact(&bytes).is_err()
        );
        assert!(
            KagemushaSignedOrdinaryAppIdentityPolicyV1::decode_canonical_exact(&vec![0; 16385])
                .is_err()
        );
        let mut profile = f.signed.policy.profile;
        profile.identity_profile_id[0] ^= 1;
        assert!(profile.validate().is_err());
    }
    #[test]
    fn identity_preparation_requires_original_core_key_full_scope_and_policy_bounds() {
        let f = fixture();
        let owner = checked(&f);
        for i in 0..10 {
            let mut c = f.preparation.challenge;
            match i {
                0 => c.issuer_policy_digest = [77; 32],
                1 => c.network_id = [77; 32],
                2 => c.release_id = [77; 32],
                3 => c.hardware_profile_id = [77; 32],
                4 => c.suite_id = [77; 32],
                5 => c.trust_policy_digest = [77; 32],
                6 => c.app_authority_policy_digest = [77; 32],
                7 => c.policy_epoch += 1,
                8 => c.expires_at_ms = 20001,
                _ => c.expires_at_ms = 11000,
            };
            let signed = KagemushaSignedOrdinaryAppEnrollmentChallengeV1 {
                challenge: c,
                signature: Signature::try_new(
                    f.issuer.private_key(),
                    &c.canonical_signing_bytes().unwrap(),
                )
                .unwrap(),
            };
            assert!(owner.authenticate_preparation(&signed, &c, 100).is_err());
        }
        let foreign = KeyPair::from_seed(vec![77; 32], Algorithm::Ed25519);
        let mut bad = f.preparation.clone();
        bad.signature = Signature::try_new(
            foreign.private_key(),
            &bad.challenge.canonical_signing_bytes().unwrap(),
        )
        .unwrap();
        assert!(
            owner
                .authenticate_preparation(&bad, &bad.challenge, 100)
                .is_err()
        );
        bad = f.preparation.clone();
        bad.challenge.account_binding = [77; 32];
        bad.signature = Signature::try_new(
            f.issuer.private_key(),
            &bad.challenge.canonical_signing_bytes().unwrap(),
        )
        .unwrap();
        assert!(
            owner
                .authenticate_preparation(&bad, &f.preparation.challenge, 100)
                .is_err()
        );
    }
    #[test]
    fn persistent_tee_and_strongbox_public_raw_e_credential_admission_needs_no_monetary_catalog() {
        let f = fixture();
        let owner = checked(&f);
        let c = owner
            .authenticate_preparation(&f.preparation, &f.preparation.challenge, 100)
            .unwrap();
        let app_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            f.app.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let raw = crate::kagemusha::kagemusha_platform_attestation_original_v1::platform_original_fixture(false)
            .canonical_bytes().unwrap();
        for level in [
            KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment,
            KagemushaAppKeySecurityLevelV1::StrongBox,
        ] {
            let subject = KagemushaRawAppAttestationAdmissionSubjectV1 {
                version: 1,
                enrollment_challenge_digest: c.challenge().attestation_challenge().unwrap(),
                authority_policy_digest: c.challenge().app_authority_policy_digest,
                platform_class: c.challenge().platform_class,
                security_level: level,
                app_public_key: app_key,
                attested_key_id: Sha256::digest(app_key.as_sec1_bytes()).into(),
                raw_platform_evidence_digest: Sha256::digest(&raw).into(),
                app_signing_identity_digest: owner.policy().app_signing_identity_digest,
                original_app_attest_counter: 0,
                issued_at_ms: 100,
                expires_at_ms: 2000,
            };
            let raw_signed = KagemushaRawAppAttestationAdmissionV1 {
                subject,
                signature: Signature::try_new(
                    f.raw_issuer.private_key(),
                    &subject.canonical_signing_bytes().unwrap(),
                )
                .unwrap(),
            };
            let admitted = raw_signed.authenticate(&owner, &c, &raw, 300).unwrap();
            assert_ne!(f.issuer.public_key(), f.raw_issuer.public_key());
            let mut wrong_issuer = raw_signed.clone();
            wrong_issuer.signature = Signature::try_new(
                f.issuer.private_key(),
                &subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap();
            assert!(wrong_issuer.authenticate(&owner, &c, &raw, 300).is_err());
            assert_eq!(admitted.identity_policy_id(), owner.policy_id());
            let challenge = KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
                c.challenge(),
                &app_key,
                subject.raw_platform_evidence_digest,
            )
            .unwrap();
            let sig: P256Signature = f.app.sign(&challenge.canonical_signing_bytes().unwrap());
            let possession = KagemushaAppEnrollmentPossessionV1 {
                challenge,
                evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                    signature_der: sig.to_der().as_bytes().to_vec(),
                },
            };
            let e = possession
                .authenticate(
                    &f.preparation,
                    f.issuer.public_key(),
                    c.challenge(),
                    &admitted,
                    &raw,
                    300,
                )
                .unwrap();
            let ch = c.challenge();
            let certificate_subject = KagemushaOrdinaryAppCredentialSubjectV1 {
                version: 1,
                platform_class: ch.platform_class,
                security_level: level,
                enrollment_id: ch.enrollment_id,
                client_nonce: ch.client_nonce,
                server_nonce: ch.server_nonce,
                account_binding: ch.account_binding,
                network_id: ch.network_id,
                lane_id: ch.lane_id,
                release_id: ch.release_id,
                hardware_profile_id: ch.hardware_profile_id,
                suite_id: ch.suite_id,
                trust_policy_digest: ch.trust_policy_digest,
                app_authority_policy_digest: ch.app_authority_policy_digest,
                app_signing_identity_digest: subject.app_signing_identity_digest,
                app_release_digest: owner.policy().app_release_digest,
                attested_key_id: subject.attested_key_id,
                app_key_reference: kagemusha_device_key_reference_v1(&app_key),
                financial_authority_commitment: ch.financial_authority_commitment,
                platform_evidence_digest: e.platform_evidence_digest(),
                enrollment_challenge_digest: ch.attestation_challenge().unwrap(),
                app_public_key: app_key,
                policy_epoch: ch.policy_epoch,
                hardware_epoch: ch.hardware_epoch,
                issued_at_ms: 200,
                expires_at_ms: 10200,
                app_attest_counter_floor: 0,
                play_integrity: None,
            };
            let signature = Signature::try_new(
                f.raw_issuer.private_key(),
                &certificate_subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap();
            let certificate = KagemushaOrdinaryAppCredentialV1 {
                subject: certificate_subject,
                circuit_admission:
                    crate::testing::ordinary_app_enrollment::ordinary_test_issuer_admission_v1(
                        KagemushaOrdinaryAppCredentialV1::circuit_admission_subject_for(
                            &certificate_subject,
                            &signature,
                        )
                        .unwrap(),
                    ),
                signature,
            };
            let verified = certificate.authenticate(&owner, &c, &app_key, 300).unwrap();
            let mut wrong_issuer = certificate.clone();
            wrong_issuer.signature = Signature::try_new(
                f.issuer.private_key(),
                &certificate_subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap();
            assert!(
                wrong_issuer
                    .authenticate(&owner, &c, &app_key, 300)
                    .is_err()
            );
            e.bind_credential(&verified, 300).unwrap();
            assert_eq!(verified.subject().security_level, level);
            assert_eq!(verified.preparation_original(), c.original());
            assert_eq!(challenge.canonical_signing_bytes().unwrap().len(), 424);
            assert!(raw_signed.authenticate(&owner, &c, &raw, 2000).is_err());
            assert!(certificate.authenticate(&owner, &c, &app_key, 3000).is_ok());
        }
    }
}
