//! Genuine issuer-authenticated raw platform originals before E and final identity issuance.
//! This purpose carries no financial proof, native State Guard or offline non-forking guarantee.

use super::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1, KagemushaDevicePublicKeyV1,
    KagemushaHardwarePlatformClassV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Single pending raw-attestation issuer signing domain, including the final NUL.
pub const KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:raw-app-attestation-admission\0";
/// Sole fixed subject width, distinct from C451/E371/W275.
pub const KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_BODY_BYTES_V1: usize = 250;
/// Sole purpose-specific private encoder request prefix, never a generic signing oracle.
pub const KAGEMUSHA_RAW_APP_ATTESTATION_SIGNING_REQUEST_MAGIC_V1: &[u8; 6] = b"KRAC01";
/// Exact prefix, fixed subject body and independently selected Ed25519 public-key pin.
pub const KAGEMUSHA_RAW_APP_ATTESTATION_SIGNING_REQUEST_BYTES_V1: usize = 288;
/// Exact public pending original: fixed subject body followed by the Ed25519 signature.
pub const KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_TRANSPORT_BYTES_V1: usize = 314;

/// Issuer-signed admission of raw attestation before E and before final identity issuance.
/// This is a distinct purpose, never an activated ordinary credential or monetary owner.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaRawAppAttestationAdmissionSubjectV1")]
pub struct KagemushaRawAppAttestationAdmissionSubjectV1 {
    /// Sole first-release version.
    pub version: u16,
    /// SHA256 of exact independently prepared C451, including domain and length.
    pub enrollment_challenge_digest: [u8; 32],
    /// Actual app-attestation authority selected by governance.
    pub authority_policy_digest: [u8; 32],
    /// Actual attested platform, not a handset claim.
    pub platform_class: KagemushaHardwarePlatformClassV1,
    /// Actual allowed TEE/StrongBox/AppAttest level from raw verification.
    pub security_level: super::KagemushaAppKeySecurityLevelV1,
    /// Exact verified nonexportable original point.
    pub app_public_key: KagemushaDevicePublicKeyV1,
    /// SHA256 of that exact uncompressed SEC1 point.
    pub attested_key_id: [u8; 32],
    /// SHA256 of full original raw chain/attestation bytes authenticated by the issuer.
    pub raw_platform_evidence_digest: [u8; 32],
    /// Actual allowed app signer/RP digest from raw verification and original policy.
    pub app_signing_identity_digest: [u8; 32],
    /// Raw App Attest initial counter, not a financial index; initial attestation is counter0.
    pub original_app_attest_counter: u32,
    /// Inclusive original signed C scope time; not a renewed verification timestamp.
    pub issued_at_ms: u64,
    /// Exclusive original signed C expiry, retained exactly and never refreshed by the phone.
    pub expires_at_ms: u64,
}
impl KagemushaRawAppAttestationAdmissionSubjectV1 {
    /// Encode the closed raw-admission request for the purpose-specific native child encoder.
    /// The caller must obtain `signer_pin` from the independently authenticated issuer policy.
    /// This formatter grants neither signer custody nor raw-platform verification authority.
    /// # Errors
    /// Rejects another signer role, missing pin or invalid fixed subject shape.
    pub fn to_signing_request(
        &self,
        signer_pin: &iroha_crypto::PublicKey,
    ) -> Result<Vec<u8>, String> {
        let (algorithm, key) = signer_pin.to_bytes();
        if algorithm != iroha_crypto::Algorithm::Ed25519 || key.len() != 32 || key == [0; 32] {
            return Err("raw app admission signer pin rejected".into());
        }
        let message = self.canonical_signing_bytes()?;
        let mut request = KAGEMUSHA_RAW_APP_ATTESTATION_SIGNING_REQUEST_MAGIC_V1.to_vec();
        request.extend_from_slice(
            &message[KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_DOMAIN_V1.len() + 8..],
        );
        request.extend_from_slice(key);
        Ok(request)
    }
    /// Parse only the closed fixed raw-admission child-encoder request.
    /// This is a structural formatter boundary, never an issuer admission function.
    /// # Errors
    /// Rejects another purpose, width, empty signer pin or invalid subject.
    pub fn from_signing_request(request: &[u8]) -> Result<(Self, [u8; 32]), String> {
        if request.len() != KAGEMUSHA_RAW_APP_ATTESTATION_SIGNING_REQUEST_BYTES_V1
            || &request[..6] != KAGEMUSHA_RAW_APP_ATTESTATION_SIGNING_REQUEST_MAGIC_V1
        {
            return Err("raw app admission signing request rejected".into());
        }
        let pin: [u8; 32] = request[256..]
            .try_into()
            .map_err(|_| "raw app admission signer width differs")?;
        if pin == [0; 32] {
            return Err("raw app admission signer pin absent".into());
        }
        Ok((Self::from_signing_body(&request[6..256])?, pin))
    }

    /// Decode the sole fixed subject body for the purpose-specific native issuer encoder.
    /// This is a structural codec; it does not authenticate an issuer or raw platform evidence.
    /// # Errors
    /// Rejects any missing/trailing field, tag, malformed point or noncanonical signing shape.
    pub fn from_signing_body(body: &[u8]) -> Result<Self, String> {
        if body.len() != KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_BODY_BYTES_V1 {
            return Err("raw app admission body width differs".into());
        }
        let mut r = BodyReader {
            bytes: body,
            offset: 0,
        };
        let version = u16::from_le_bytes(r.read()?);
        if r.read::<1>()? != [1] {
            return Err("raw app admission purpose differs".into());
        }
        let enrollment_challenge_digest = r.read()?;
        let authority_policy_digest = r.read()?;
        let platform_class = match r.read::<1>()?[0] {
            1 => KagemushaHardwarePlatformClassV1::AndroidKeyMint,
            2 => KagemushaHardwarePlatformClassV1::AppleAppAttest,
            _ => return Err("raw app admission platform differs".into()),
        };
        let security_level = match r.read::<1>()?[0] {
            1 => super::KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment,
            2 => super::KagemushaAppKeySecurityLevelV1::StrongBox,
            3 => super::KagemushaAppKeySecurityLevelV1::AppleAppAttest,
            _ => return Err("raw app admission level differs".into()),
        };
        let app_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(&r.read::<65>()?)
            .map_err(|_| "raw app admission point rejected")?;
        let subject = Self {
            version,
            enrollment_challenge_digest,
            authority_policy_digest,
            platform_class,
            security_level,
            app_public_key,
            attested_key_id: r.read()?,
            raw_platform_evidence_digest: r.read()?,
            app_signing_identity_digest: r.read()?,
            original_app_attest_counter: u32::from_le_bytes(r.read()?),
            issued_at_ms: u64::from_le_bytes(r.read()?),
            expires_at_ms: u64::from_le_bytes(r.read()?),
        };
        let message = subject.canonical_signing_bytes()?;
        if r.offset != body.len()
            || &message[KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_DOMAIN_V1.len() + 8..] != body
        {
            return Err("raw app admission original is not canonical".into());
        }
        Ok(subject)
    }

    /// Exact issuer message for pending raw attestation, distinct from C/E/W/final credential.
    /// # Errors
    /// Rejects unsupported role, missing key/evidence, invalid platform or interval.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        use super::KagemushaAppKeySecurityLevelV1 as Level;
        let platform = match (self.platform_class, self.security_level) {
            (
                KagemushaHardwarePlatformClassV1::AndroidKeyMint,
                Level::TrustedExecutionEnvironment | Level::StrongBox,
            ) => 1,
            (KagemushaHardwarePlatformClassV1::AppleAppAttest, Level::AppleAppAttest) => 2,
            _ => return Err("raw app admission platform differs".into()),
        };
        self.app_public_key
            .validate()
            .map_err(|_| "raw admission point rejected")?;
        if self.version != 1
            || [
                self.enrollment_challenge_digest,
                self.authority_policy_digest,
                self.attested_key_id,
                self.raw_platform_evidence_digest,
                self.app_signing_identity_digest,
            ]
            .contains(&[0; 32])
            || self.attested_key_id
                != <[u8; 32]>::from(Sha256::digest(self.app_public_key.as_sec1_bytes()))
            || self.original_app_attest_counter != 0
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1
        {
            return Err("raw app admission scope invalid".into());
        }
        let mut body = self.version.to_le_bytes().to_vec();
        body.push(1);
        body.extend_from_slice(&self.enrollment_challenge_digest);
        body.extend_from_slice(&self.authority_policy_digest);
        body.push(platform);
        body.push(self.security_level.signing_tag());
        body.extend_from_slice(self.app_public_key.as_sec1_bytes());
        for field in [
            self.attested_key_id,
            self.raw_platform_evidence_digest,
            self.app_signing_identity_digest,
        ] {
            body.extend_from_slice(&field)
        }
        body.extend_from_slice(&self.original_app_attest_counter.to_le_bytes());
        body.extend_from_slice(&self.issued_at_ms.to_le_bytes());
        body.extend_from_slice(&self.expires_at_ms.to_le_bytes());
        if body.len() != KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_BODY_BYTES_V1 {
            return Err("raw app admission body differs".into());
        }
        let mut message = KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_DOMAIN_V1.to_vec();
        message.extend_from_slice(&(body.len() as u64).to_le_bytes());
        message.extend_from_slice(&body);
        Ok(message)
    }
}
struct BodyReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl BodyReader<'_> {
    fn read<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or("raw admission offset overflow")?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or("raw admission field missing")?;
        self.offset = end;
        value
            .try_into()
            .map_err(|_| "raw admission field width differs".into())
    }
}
/// Bounded issuer signature over raw attestation only. Decoding does not admit its issuer/scope.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaRawAppAttestationAdmissionV1")]
pub struct KagemushaRawAppAttestationAdmissionV1 {
    /// Exact observed pending key/evidence scope.
    pub subject: KagemushaRawAppAttestationAdmissionSubjectV1,
    /// Ed25519 signature under independently selected actual app-attestation issuer.
    pub signature: iroha_crypto::Signature,
}
/// Opaque authenticated raw-attestation original, not final identity or money.
pub struct KagemushaVerifiedRawAppAttestationAdmissionV1 {
    subject: KagemushaRawAppAttestationAdmissionSubjectV1,
    original: Vec<u8>,
}
impl KagemushaVerifiedRawAppAttestationAdmissionV1 {
    /// Exact independently authenticated pending key/evidence selection.
    #[must_use]
    pub const fn subject(&self) -> &KagemushaRawAppAttestationAdmissionSubjectV1 {
        &self.subject
    }
    /// Original issuer bytes, retained before E generation.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Recheck the original pending interval without issuing a credential or granting authority.
    /// # Errors
    /// Rejects before issue or at/after original expiry.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<(), String> {
        if now < self.subject.issued_at_ms || now >= self.subject.expires_at_ms {
            return Err("raw app admission expired".into());
        }
        Ok(())
    }
}
impl KagemushaRawAppAttestationAdmissionV1 {
    /// Encode the sole bounded pending original without granting issuer or key authority.
    /// # Errors
    /// Rejects an invalid fixed subject or another issuer signature width.
    pub fn to_transport_bytes(&self) -> Result<Vec<u8>, String> {
        let message = self.subject.canonical_signing_bytes()?;
        if self.signature.payload().len() != 64 {
            return Err("raw app admission signature width differs".into());
        }
        let mut original =
            message[KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_DOMAIN_V1.len() + 8..].to_vec();
        original.extend_from_slice(self.signature.payload());
        Ok(original)
    }
    /// Decode only the exact fixed pending original. Actual issuer admission remains separate.
    /// # Errors
    /// Rejects width, canonical signing shape, unsupported tags or malformed original point.
    pub fn from_transport_bytes(original: &[u8]) -> Result<Self, String> {
        if original.len() != KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_TRANSPORT_BYTES_V1 {
            return Err("raw app admission transport width differs".into());
        }
        let body = KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_BODY_BYTES_V1;
        let value = Self {
            subject: KagemushaRawAppAttestationAdmissionSubjectV1::from_signing_body(
                &original[..body],
            )?,
            signature: iroha_crypto::Signature::from_bytes(&original[body..]),
        };
        if value.to_transport_bytes()? != original {
            return Err("raw app admission transport is not canonical".into());
        }
        Ok(value)
    }

    /// Authenticate pending raw evidence under actual release/profile and independent issuer policy.
    /// The release/profile association selects scope; it does not require or confer hardware
    /// one-use/non-forking qualification. The issuer must have verified actual raw platform bytes.
    /// # Errors
    /// Rejects mixed C/key/evidence/issuer/platform/level/policy or expired original time.
    pub fn authenticate(
        &self,
        release: &super::KagemushaAuthenticatedReleaseV1,
        trust: &super::KagemushaOrdinaryAppTrustPolicyV1,
        authority: &super::KagemushaAppAttestationAuthorityPolicyV1,
        expected: &super::KagemushaOrdinaryAppEnrollmentChallengeV1,
        now: u64,
    ) -> Result<KagemushaVerifiedRawAppAttestationAdmissionV1, String> {
        let enabled = release
            .enabled_profile(expected.hardware_profile_id)
            .ok_or("raw admission profile absent")?;
        trust.validate_for_profile(&enabled.hardware_profile, authority)?;
        let s = &self.subject;
        let message = s.canonical_signing_bytes()?;
        if expected.release_id != release.release_id()
            || expected.network_id != *release.network_id().as_bytes()
            || expected.suite_id != enabled.suite_id
            || expected.policy_epoch != enabled.policy_epoch
            || s.enrollment_challenge_digest != expected.attestation_challenge()?
            || s.authority_policy_digest != expected.app_authority_policy_digest
            || s.authority_policy_digest != authority.canonical_digest()?
            || expected.trust_policy_digest != trust.canonical_digest()?
            || s.platform_class != expected.platform_class
            || s.platform_class != authority.platform_class
            || s.app_signing_identity_digest != authority.app_signing_identity_digest
            || s.issued_at_ms != expected.issued_at_ms
            || s.expires_at_ms != expected.expires_at_ms
            || s.expires_at_ms - s.issued_at_ms > authority.maximum_lifetime_ms
            || s.issued_at_ms < enabled.hardware_profile.valid_from_ms
            || s.expires_at_ms > enabled.hardware_profile.expires_at_ms
            || self.signature.payload().len() != 64
            || authority.authority_key.algorithm() != iroha_crypto::Algorithm::Ed25519
            || (s.platform_class == KagemushaHardwarePlatformClassV1::AndroidKeyMint
                && !trust
                    .allowed_android_security_levels
                    .contains(&s.security_level))
        {
            return Err("raw attestation admission original differs".into());
        }
        self.signature
            .verify(&authority.authority_key, &message)
            .map_err(|_| "raw attestation admission issuer rejected")?;
        let original = self.to_transport_bytes()?;
        let checked = KagemushaVerifiedRawAppAttestationAdmissionV1 {
            subject: *s,
            original,
        };
        checked.recheck_at_trusted_time(now)?;
        Ok(checked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    use iroha_crypto::{Algorithm, KeyPair, Signature};

    fn raw(apple: bool) -> (Fixture, KagemushaRawAppAttestationAdmissionV1) {
        let f = Fixture::new(apple);
        let c = &f.selection.preparation.challenge;
        let app = &f.selection.issuance.credential.subject;
        let subject = KagemushaRawAppAttestationAdmissionSubjectV1 {
            version: 1,
            enrollment_challenge_digest: c.attestation_challenge().unwrap(),
            authority_policy_digest: f.app_authority.canonical_digest().unwrap(),
            platform_class: app.platform_class,
            security_level: app.security_level,
            app_public_key: app.app_public_key,
            attested_key_id: app.attested_key_id,
            raw_platform_evidence_digest: Sha256::digest(&f.proof.raw_attestation).into(),
            app_signing_identity_digest: f.app_authority.app_signing_identity_digest,
            original_app_attest_counter: 0,
            issued_at_ms: c.issued_at_ms,
            expires_at_ms: c.expires_at_ms,
        };
        let key = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let signature = Signature::try_new(
            key.private_key(),
            &subject.canonical_signing_bytes().unwrap(),
        )
        .unwrap();
        (
            f,
            KagemushaRawAppAttestationAdmissionV1 { subject, signature },
        )
    }

    #[test]
    fn raw_admission_authenticates_actual_pending_signature_and_retains_originals() {
        for apple in [false, true] {
            let (f, original) = raw(apple);
            let expected = &f.selection.preparation.challenge;
            let checked = original
                .authenticate(&f.release, &f.trust, &f.app_authority, expected, 300)
                .unwrap();
            assert_eq!(checked.subject(), &original.subject);
            assert_eq!(checked.original(), original.to_transport_bytes().unwrap());
            assert_ne!(
                checked.original(),
                f.selection.issuance.credential.canonical_bytes().unwrap()
            );
            assert!(
                checked
                    .recheck_at_trusted_time(expected.expires_at_ms)
                    .is_err()
            );
            let bytes = original.subject.canonical_signing_bytes().unwrap();
            let offset = KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_DOMAIN_V1.len();
            assert_eq!(bytes.len(), offset + 8 + 250);
            assert_eq!(&bytes[offset..offset + 8], &250u64.to_le_bytes());
            assert_eq!(bytes[offset + 8 + 67], if apple { 2 } else { 1 });
        }
    }

    #[test]
    fn raw_admission_refuses_new_interval_point_scope_and_wrong_signature() {
        let (f, original) = raw(false);
        let c = &f.selection.preparation.challenge;
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        for field in 0..6 {
            let mut changed = original.clone();
            match field {
                0 => changed.subject.enrollment_challenge_digest[0] ^= 1,
                1 => changed.subject.issued_at_ms += 1,
                2 => changed.subject.raw_platform_evidence_digest = [0; 32],
                3 => changed.subject.original_app_attest_counter = 1,
                4 => changed.subject.attested_key_id[0] ^= 1,
                _ => changed.subject.app_signing_identity_digest[0] ^= 1,
            }
            // Re-sign valid altered shapes to prove expected-original joins are enforced.
            if let Ok(message) = changed.subject.canonical_signing_bytes() {
                changed.signature = Signature::try_new(issuer.private_key(), &message).unwrap();
            }
            assert!(
                changed
                    .authenticate(&f.release, &f.trust, &f.app_authority, c, 300)
                    .is_err()
            );
        }
        // E deliberately retains enrollment ID. The actual signed raw admission supplies
        // the mandatory whole-C join, including selectors outside E's fixed fields.
        for field in 0..4 {
            let mut substituted = *c;
            match field {
                0 => substituted.hardware_epoch += 1,
                1 => substituted.financial_authority_commitment[0] ^= 1,
                2 => substituted.suite_id[0] ^= 1,
                _ => substituted.trust_policy_digest[0] ^= 1,
            }
            assert!(
                original
                    .authenticate(&f.release, &f.trust, &f.app_authority, &substituted, 300)
                    .is_err()
            );
        }
        let mut changed = original;
        let foreign = KeyPair::from_seed(vec![63; 32], Algorithm::Ed25519);
        changed.signature = Signature::try_new(
            foreign.private_key(),
            &changed.subject.canonical_signing_bytes().unwrap(),
        )
        .unwrap();
        assert!(
            changed
                .authenticate(&f.release, &f.trust, &f.app_authority, c, 300)
                .is_err()
        );
    }

    #[test]
    fn raw_admission_codecs_keep_one_purpose_fixed_fields_and_exact_original() {
        for apple in [false, true] {
            let (_, original) = raw(apple);
            let body_message = original.subject.canonical_signing_bytes().unwrap();
            let body = &body_message[KAGEMUSHA_RAW_APP_ATTESTATION_ADMISSION_DOMAIN_V1.len() + 8..];
            assert_eq!(
                KagemushaRawAppAttestationAdmissionSubjectV1::from_signing_body(body).unwrap(),
                original.subject
            );
            let encoded = norito::encode_canonical(&original).unwrap();
            assert_eq!(
                norito::decode_from_bytes::<KagemushaRawAppAttestationAdmissionV1>(&encoded)
                    .unwrap(),
                original
            );
            let transport = original.to_transport_bytes().unwrap();
            assert_eq!(transport.len(), 314);
            assert_eq!(
                KagemushaRawAppAttestationAdmissionV1::from_transport_bytes(&transport).unwrap(),
                original
            );
            let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
            let request = original
                .subject
                .to_signing_request(issuer.public_key())
                .unwrap();
            assert_eq!(request.len(), 288);
            let (subject, pin) =
                KagemushaRawAppAttestationAdmissionSubjectV1::from_signing_request(&request)
                    .unwrap();
            assert_eq!(subject, original.subject);
            assert_eq!(pin.as_slice(), issuer.public_key().to_bytes().1);
            for n in [0, 249, 313] {
                assert!(
                    KagemushaRawAppAttestationAdmissionV1::from_transport_bytes(&transport[..n])
                        .is_err()
                );
            }
            let mut extra = transport.clone();
            extra.push(0);
            assert!(KagemushaRawAppAttestationAdmissionV1::from_transport_bytes(&extra).is_err());
            for field in [0, 8, 73, 256] {
                let mut changed = request.clone();
                if field == 256 {
                    changed[256..].fill(0);
                } else {
                    changed[field] ^= 0xff;
                }
                assert!(
                    KagemushaRawAppAttestationAdmissionSubjectV1::from_signing_request(&changed)
                        .is_err()
                );
            }
        }
    }
}
