//! Mandatory purpose-specific governed issuer admission for private ordinary proof witnesses.
//! Native admission also authenticates the original Ed certificate/lease. This signature hashes
//! only its canonical Ed-only original, avoiding a self-referential countersignature digest.

use super::{
    KagemushaAuthenticatedReleaseV1, KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1,
    KagemushaHardwareProfileV1, KagemushaReleasePurposeV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use norito::codec::{Decode, Encode};

/// Sole first-release issuer admission signing domain, including NUL.
pub const KAGEMUSHA_ORDINARY_ISSUER_CIRCUIT_ADMISSION_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-issuer-circuit-admission\0";
/// Sole secret-seed derivation domain; this derives a private key, never a trusted public pin.
pub const KAGEMUSHA_ORDINARY_ISSUER_P256_SEED_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-issuer-p256-seed\0";
/// Exact unsigned issuer admission body width.
pub const KAGEMUSHA_ORDINARY_ISSUER_CIRCUIT_ADMISSION_BODY_BYTES_V1: usize = 99;
/// Exact fixed transport width, unsigned body followed by low-S `r || s`.
pub const KAGEMUSHA_ORDINARY_ISSUER_CIRCUIT_ADMISSION_TRANSPORT_BYTES_V1: usize = 163;

/// Model-owned absolute raw positions in the actual issuer signing message.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaOrdinaryIssuerCircuitAdmissionSigningLayoutV1;
impl KagemushaOrdinaryIssuerCircuitAdmissionSigningLayoutV1 {
    /// Exact domain bytes.
    pub const DOMAIN: core::ops::Range<usize> =
        0..KAGEMUSHA_ORDINARY_ISSUER_CIRCUIT_ADMISSION_DOMAIN_V1.len();
    /// LE64(99).
    pub const BODY_LENGTH: core::ops::Range<usize> = Self::DOMAIN.end..Self::DOMAIN.end + 8;
    /// Complete unsigned body.
    pub const BODY: core::ops::Range<usize> = Self::BODY_LENGTH.end..Self::BODY_LENGTH.end + 99;
    /// LE16(1).
    pub const VERSION: core::ops::Range<usize> = Self::BODY.start..Self::BODY.start + 2;
    /// Credential1 or Integrity lease2.
    pub const PURPOSE: usize = Self::BODY.start + 2;
    /// Actual signed release identity.
    pub const RELEASE_ID: core::ops::Range<usize> = Self::BODY.start + 3..Self::BODY.start + 35;
    /// Actual signed governed profile identity.
    pub const HARDWARE_PROFILE_ID: core::ops::Range<usize> =
        Self::BODY.start + 35..Self::BODY.start + 67;
    /// SHA256 of the complete canonical Ed-only original.
    pub const ED_ORIGINAL_SHA256: core::ops::Range<usize> =
        Self::BODY.start + 67..Self::BODY.start + 99;
    /// Complete signing message width.
    pub const TOTAL_BYTES: usize = Self::BODY.end;
}

/// Exact mandatory issuer body, separate from app approval and OEM compact credentials.
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
#[norito_schema(
    name = "iroha_data_model::kagemusha::KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1"
)]
pub struct KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Credential1 or Integrity lease2; all other purposes are rejected.
    pub purpose: u8,
    /// Actual release selected before signing.
    pub release_id: [u8; 32],
    /// Actual profile containing the independent governed P256 issuer public key.
    pub hardware_profile_id: [u8; 32],
    /// SHA256 of the complete model canonical Ed-only original, including its Ed signature.
    pub ed_original_sha256: [u8; 32],
}
impl KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1 {
    /// Parse the exact unsigned body without authenticating its issuer.
    /// # Errors
    /// Rejects another width, purpose or zero scope/digest.
    pub fn from_signing_body(raw: &[u8]) -> Result<Self, String> {
        if raw.len() != 99 {
            return Err("ordinary issuer admission body width differs".into());
        }
        let this = Self {
            version: u16::from_le_bytes(raw[..2].try_into().unwrap()),
            purpose: raw[2],
            release_id: raw[3..35].try_into().unwrap(),
            hardware_profile_id: raw[35..67].try_into().unwrap(),
            ed_original_sha256: raw[67..99].try_into().unwrap(),
        };
        this.canonical_signing_bytes()?;
        Ok(this)
    }
    /// Sole P256-SHA256 signing message for the original's private circuit admission.
    /// # Errors
    /// Rejects an unknown version/purpose or zero scope/digest.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        if self.version != 1
            || !matches!(self.purpose, 1 | 2)
            || self.release_id == [0; 32]
            || self.hardware_profile_id == [0; 32]
            || self.ed_original_sha256 == [0; 32]
        {
            return Err("ordinary issuer admission subject rejected".into());
        }
        let mut raw = KAGEMUSHA_ORDINARY_ISSUER_CIRCUIT_ADMISSION_DOMAIN_V1.to_vec();
        raw.extend_from_slice(&99u64.to_le_bytes());
        raw.extend_from_slice(&self.version.to_le_bytes());
        raw.push(self.purpose);
        raw.extend_from_slice(&self.release_id);
        raw.extend_from_slice(&self.hardware_profile_id);
        raw.extend_from_slice(&self.ed_original_sha256);
        Ok(raw)
    }
}
/// Mandatory real governed issuer signature; no optional or legacy admission exists.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryIssuerCircuitAdmissionV1")]
pub struct KagemushaOrdinaryIssuerCircuitAdmissionV1 {
    /// Exact purpose and Ed-only original hash.
    pub subject: KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1,
    /// Fixed canonical low-S P256 signature under the actual signed profile issuer key.
    pub signature: KagemushaDeviceSignatureV1,
}
/// Native verified signature under the threshold-authenticated actual governed profile.
/// This is not enrollment, an approval nonce, a State proof or monetary authority.
pub struct KagemushaVerifiedOrdinaryIssuerCircuitAdmissionV1 {
    original: KagemushaOrdinaryIssuerCircuitAdmissionV1,
    public_key: KagemushaDevicePublicKeyV1,
    transport_original: Vec<u8>,
}
impl KagemushaVerifiedOrdinaryIssuerCircuitAdmissionV1 {
    /// Borrow the exact verified fixed transport original.
    #[must_use]
    pub fn transport_original(&self) -> &[u8] {
        &self.transport_original
    }

    /// Borrow the actual independently verified admission original.
    #[must_use]
    pub const fn original(&self) -> &KagemushaOrdinaryIssuerCircuitAdmissionV1 {
        &self.original
    }
    /// Borrow the actual signed profile's issuer point, never derived from an Ed public key.
    #[must_use]
    pub const fn public_key(&self) -> &KagemushaDevicePublicKeyV1 {
        &self.public_key
    }
}
impl KagemushaOrdinaryIssuerCircuitAdmissionV1 {
    /// Encode the fixed unsigned body plus canonical low-S signature; no authority is inferred.
    /// # Errors
    /// Rejects a malformed subject/signature.
    pub fn to_transport_bytes(&self) -> Result<Vec<u8>, String> {
        self.signature
            .validate()
            .map_err(|_| "ordinary issuer signature shape rejected")?;
        let signing = self.subject.canonical_signing_bytes()?;
        let mut raw =
            signing[KAGEMUSHA_ORDINARY_ISSUER_CIRCUIT_ADMISSION_DOMAIN_V1.len() + 8..].to_vec();
        raw.extend_from_slice(self.signature.as_raw_bytes());
        Ok(raw)
    }
    /// Decode exactly one bounded fixed original without granting issuer authority.
    /// # Errors
    /// Rejects trailing bytes, unknown subjects or noncanonical/high-S signatures.
    pub fn from_transport_bytes(raw: &[u8]) -> Result<Self, String> {
        if raw.len() != 163 {
            return Err("ordinary issuer admission transport width differs".into());
        }
        Ok(Self {
            subject: KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1::from_signing_body(
                &raw[..99],
            )?,
            signature: KagemushaDeviceSignatureV1::from_raw_bytes(&raw[99..])
                .map_err(|_| "ordinary issuer signature shape rejected")?,
        })
    }
    /// Verify the exact expected original under the actual threshold-admitted release/profile.
    /// Native credential/lease admission derives `expected` from its full Ed-verified original.
    /// # Errors
    /// Rejects another original/purpose, a nonproduction release or another governed issuer key.
    pub fn authenticate(
        &self,
        expected: &KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1,
        release: &KagemushaAuthenticatedReleaseV1,
    ) -> Result<KagemushaVerifiedOrdinaryIssuerCircuitAdmissionV1, String> {
        if self.subject != *expected
            || self.subject.release_id != release.release_id()
            || release.purpose() != KagemushaReleasePurposeV1::Production
        {
            return Err("ordinary issuer admission original differs".into());
        }
        let profile = release
            .enabled_profile(self.subject.hardware_profile_id)
            .ok_or("ordinary issuer profile unavailable")?;
        if !profile.hardware_profile.platform_class.is_ordinary_app() {
            return Err("ordinary issuer profile class differs".into());
        }
        self.authenticate_for_profile(expected, &profile.hardware_profile)
    }
    /// Verify the same complete purpose-bound admission under a threshold-authenticated identity pin.
    /// This creates no release, Native, Current, signer or monetary owner.
    /// # Errors
    /// Refuses stale policy, wrong complete purpose/release/profile/hash or actual P256 signature.
    pub fn authenticate_under_identity_policy(
        &self,
        expected: &KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1,
        policy: &super::KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        now: u64,
    ) -> Result<KagemushaVerifiedOrdinaryIssuerCircuitAdmissionV1, String> {
        policy.recheck_at_trusted_time(now)?;
        let original = policy.policy();
        if self.subject != *expected
            || self.subject.release_id != original.profile.planned_release_id
            || self.subject.hardware_profile_id != original.profile.planned_hardware_profile_id
            || !original.profile.platform_class.is_ordinary_app()
        {
            return Err("ordinary issuer exact admission/identity policy differs".into());
        }
        let public_key = original.enrollment_issuer_p256_key;
        self.signature
            .verify(&public_key, &self.subject.canonical_signing_bytes()?)
            .map_err(|_| "ordinary circuit issuer identity-policy signature rejected")?;
        Ok(KagemushaVerifiedOrdinaryIssuerCircuitAdmissionV1 {
            original: *self,
            transport_original: self.to_transport_bytes()?,
            public_key,
        })
    }
    pub(crate) fn authenticate_for_profile(
        &self,
        expected: &KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1,
        profile: &KagemushaHardwareProfileV1,
    ) -> Result<KagemushaVerifiedOrdinaryIssuerCircuitAdmissionV1, String> {
        if self.subject != *expected
            || self.subject.hardware_profile_id != profile.hardware_profile_id
            || !profile.platform_class.is_ordinary_app()
        {
            return Err("ordinary issuer original/profile differs".into());
        }
        let public_key = profile.governance_credential_public_key;

        self.signature
            .verify(&public_key, &self.subject.canonical_signing_bytes()?)
            .map_err(|_| "ordinary circuit issuer signature rejected")?;
        Ok(KagemushaVerifiedOrdinaryIssuerCircuitAdmissionV1 {
            original: *self,
            transport_original: self.to_transport_bytes()?,
            public_key,
        })
    }
}

/// Encoder-owned raw positions in a canonical nested admission payload.
/// Offsets are relative to its declared payload; all field framing remains pinned.
/// Copying these offsets grants no signature or release admission.
#[derive(Debug, Clone, Copy)]
pub struct KagemushaOrdinaryIssuerCircuitAdmissionOriginalLayoutV1 {
    /// Raw LE16 version positions.
    pub version_bytes: [usize; 2],
    /// Raw purpose byte position.
    pub purpose_byte: usize,
    /// Raw release/profile/Ed-original SHA32 byte positions in signing order.
    pub fixed_digest_bytes: [[usize; 32]; 3],
    /// Raw low-S P256 r||s64 byte positions.
    pub signature_bytes: [usize; 64],
}
impl KagemushaOrdinaryIssuerCircuitAdmissionV1 {
    /// Derive exact raw positions from the sole model encoder under the original frame flags.
    /// This data-only layout is not signature or release admission.
    /// # Errors
    /// Rejects another declared field layout or unsupported original flags.
    pub fn original_payload_layout(
        &self,
        flags: u8,
    ) -> Result<KagemushaOrdinaryIssuerCircuitAdmissionOriginalLayoutV1, String> {
        use super::kagemusha_ordinary_app_enrollment_v1::{
            layout_field_payload, layout_fixed_bytes_field, sole_changed_raw_position,
        };
        self.to_transport_bytes()?;
        let raw = layout_field_payload(self, flags)?;
        let mut cursor = 0;
        let s = crate::isi::read_aos_field(&raw, &mut cursor, flags).map_err(|e| e.to_string())?;
        let start = cursor - s.len();
        if s != layout_field_payload(&self.subject, flags)? {
            return Err("issuer admission subject layout differs".into());
        }
        let sig =
            crate::isi::read_aos_field(&raw, &mut cursor, flags).map_err(|e| e.to_string())?;
        let sig_start = cursor - sig.len();
        if cursor != raw.len() || sig != self.signature.as_raw_bytes() {
            return Err("issuer admission signature layout differs".into());
        }
        let mut sub = 0;
        let version = crate::isi::read_aos_field(s, &mut sub, flags).map_err(|e| e.to_string())?;
        if version != self.subject.version.to_le_bytes() {
            return Err("issuer version layout differs".into());
        }
        let version_bytes = core::array::from_fn(|i| start + sub - version.len() + i);
        let purpose = crate::isi::read_aos_field(s, &mut sub, flags).map_err(|e| e.to_string())?;
        if purpose != [self.subject.purpose] {
            return Err("issuer purpose layout differs".into());
        }
        let purpose_byte = start + sub - 1;
        let selectors = [
            self.subject.release_id,
            self.subject.hardware_profile_id,
            self.subject.ed_original_sha256,
        ];
        let mut fixed_digest_bytes = [[0usize; 32]; 3];
        for (selector, positions) in selectors.iter().zip(&mut fixed_digest_bytes) {
            let field =
                crate::isi::read_aos_field(s, &mut sub, flags).map_err(|e| e.to_string())?;
            let field_start = start + sub - field.len();
            if field != layout_fixed_bytes_field(selector, flags)? {
                return Err("issuer selector layout differs".into());
            }
            for (index, position) in positions.iter_mut().enumerate() {
                let mut changed = *selector;
                changed[index] ^= 1;
                *position = field_start
                    + sole_changed_raw_position(
                        field,
                        &layout_fixed_bytes_field(&changed, flags)?,
                        changed[index],
                    )?;
            }
        }
        if sub != s.len() {
            return Err("issuer subject trailing fields".into());
        }
        Ok(KagemushaOrdinaryIssuerCircuitAdmissionOriginalLayoutV1 {
            version_bytes,
            purpose_byte,
            fixed_digest_bytes,
            signature_bytes: core::array::from_fn(|i| sig_start + i),
        })
    }
}
