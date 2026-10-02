//! Exact initial app-key possession, separate from financial transition approval.
//!
//! Signature authentication produces a non-monetary cryptographic original. The native
//! enrollment owner must independently hold the pending issuer scope, reserve this attempt
//! durably, consume the original once, and activate identity only after durable completion.

use super::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1, KagemushaAppOperationApprovalEvidenceV1,
    KagemushaDevicePublicKeyV1, KagemushaHardwarePlatformClassV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Sole signing domain, including the final NUL.
pub const KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:app-enrollment-possession\0";
/// Maximum original platform assertion bytes before parsing or retaining it.
pub const KAGEMUSHA_APP_ENROLLMENT_POSSESSION_MAX_BYTES_V1: usize = 4096;
const BODY_BYTES: usize = 371;

/// Native-selected enrollment possession transcript; decoding does not admit its scope.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaAppEnrollmentPossessionChallengeV1")]
pub struct KagemushaAppEnrollmentPossessionChallengeV1 {
    /// Sole first-release version.
    pub version: u16,
    /// SHA256 of the complete original C signing message; distinct from its enrollment ID.
    pub enrollment_attempt_id: [u8; 32],
    /// Original client nonce from C.
    pub client_nonce: [u8; 32],
    /// Original server nonce durably reserved for C.
    pub server_nonce: [u8; 32],
    /// Model-owned binding of the independently selected account.
    pub account_binding: [u8; 32],
    /// Exact network from original C.
    pub network_id: [u8; 32],
    /// Independently admitted app-attestation authority policy digest.
    pub app_authority_policy_digest: [u8; 32],
    /// Actual release associated with this pending identity policy.
    pub release_id: [u8; 32],
    /// Associated profile; this association grants no financial capability.
    pub hardware_profile_id: [u8; 32],
    /// Original enrollment lane.
    pub lane_id: [u8; 32],
    /// SHA256 of the original attested uncompressed SEC1 key.
    pub attested_key_id: [u8; 32],
    /// Digest of the original raw platform enrollment evidence, not a verdict DTO.
    pub raw_platform_evidence_digest: [u8; 32],
    /// Inclusive original native issue time.
    pub issued_at_ms: u64,
    /// Exclusive original expiry, never renewed on retry.
    pub expires_at_ms: u64,
}
impl KagemushaAppEnrollmentPossessionChallengeV1 {
    /// Return E: domain || LE64(371) || version || purpose01 || eleven raw32 || times.
    /// # Errors
    /// Rejects missing scope, equal nonces, invalid version or original lifetime.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        let fields = [
            self.enrollment_attempt_id,
            self.client_nonce,
            self.server_nonce,
            self.account_binding,
            self.network_id,
            self.app_authority_policy_digest,
            self.release_id,
            self.hardware_profile_id,
            self.lane_id,
            self.attested_key_id,
            self.raw_platform_evidence_digest,
        ];
        if self.version != 1
            || fields.contains(&[0; 32])
            || self.client_nonce == self.server_nonce
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1
        {
            return Err("app enrollment possession scope invalid".into());
        }
        let mut bytes = Vec::with_capacity(
            KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1.len() + 8 + BODY_BYTES,
        );
        bytes.extend_from_slice(KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1);
        bytes.extend_from_slice(&(BODY_BYTES as u64).to_le_bytes());
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.push(1);
        for field in fields {
            bytes.extend_from_slice(&field);
        }
        bytes.extend_from_slice(&self.issued_at_ms.to_le_bytes());
        bytes.extend_from_slice(&self.expires_at_ms.to_le_bytes());
        Ok(bytes)
    }
    /// Parse the sole exact domain/length/purpose E signing message, without admitting its scope.
    /// # Errors
    /// Rejects an alternate domain, purpose, width, tail or malformed selector/interval.
    pub fn from_signing_bytes(bytes: &[u8]) -> Result<Self, String> {
        let start = KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1.len() + 8;
        if bytes.len() != start + BODY_BYTES
            || !bytes.starts_with(KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1)
            || bytes[start - 8..start] != (BODY_BYTES as u64).to_le_bytes()
            || bytes[start..start + 3] != [1, 0, 1]
        {
            return Err("enrollment possession message layout differs".into());
        }
        let fields: [[u8; 32]; 11] = core::array::from_fn(|i| {
            bytes[start + 3 + i * 32..start + 35 + i * 32]
                .try_into()
                .unwrap()
        });
        let value = Self {
            version: 1,
            enrollment_attempt_id: fields[0],
            client_nonce: fields[1],
            server_nonce: fields[2],
            account_binding: fields[3],
            network_id: fields[4],
            app_authority_policy_digest: fields[5],
            release_id: fields[6],
            hardware_profile_id: fields[7],
            lane_id: fields[8],
            attested_key_id: fields[9],
            raw_platform_evidence_digest: fields[10],
            issued_at_ms: u64::from_le_bytes(bytes[start + 355..start + 363].try_into().unwrap()),
            expires_at_ms: u64::from_le_bytes(bytes[start + 363..start + 371].try_into().unwrap()),
        };
        if value.canonical_signing_bytes()? != bytes {
            return Err("enrollment possession message is not canonical".into());
        }
        Ok(value)
    }
}

/// Exact E and platform original. Expected E is derived from authenticated C and raw admission.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaAppEnrollmentPossessionV1")]
pub struct KagemushaAppEnrollmentPossessionV1 {
    /// Exact native selected E.
    pub challenge: KagemushaAppEnrollmentPossessionChallengeV1,
    /// Original platform assertion, not a callback boolean.
    pub evidence: KagemushaAppOperationApprovalEvidenceV1,
}

/// Non-cloneable checked cryptographic original; it is not a native identity or monetary owner.
pub struct KagemushaVerifiedAppEnrollmentPossessionV1 {
    challenge: KagemushaAppEnrollmentPossessionChallengeV1,
    identity_policy_id: [u8; 32],
    identity_policy_original: Vec<u8>,
    identity_authority_original: Vec<u8>,
    original: Vec<u8>,
    preparation_original: Vec<u8>,
    raw_admission_original: Vec<u8>,
    raw_attestation_original: Vec<u8>,
    enrollment_challenge: super::KagemushaOrdinaryAppEnrollmentChallengeV1,
    platform_evidence_digest: [u8; 32],
    raw_subject: super::KagemushaRawAppAttestationAdmissionSubjectV1,
    authenticated_at_ms: u64,
    digest: [u8; 32],
    app_attest_counter: Option<u32>,
    app_attest_release_measurement: Option<super::KagemushaAppAttestReleaseMeasurementV1>,
}
impl KagemushaVerifiedAppEnrollmentPossessionV1 {
    /// Borrow exact E for native scope checks.
    #[must_use]
    pub const fn challenge(&self) -> &KagemushaAppEnrollmentPossessionChallengeV1 {
        &self.challenge
    }
    /// Borrow the original canonical evidence archive.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Digest of the original archive, never a capability constructor.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Checked Apple assertion counter; unrelated to a financial secure index.
    #[must_use]
    pub const fn app_attest_counter(&self) -> Option<u32> {
        self.app_attest_counter
    }
    /// Actual signed release measurement; limited37-byte assertions explicitly report unavailable.
    #[must_use]
    pub const fn app_attest_release_measurement(
        &self,
    ) -> Option<super::KagemushaAppAttestReleaseMeasurementV1> {
        self.app_attest_release_measurement
    }
    /// Retained exact independently verified Core-signed C transport.
    #[must_use]
    pub fn preparation_original(&self) -> &[u8] {
        &self.preparation_original
    }
    /// Retained governed issuer raw-attestation admission original.
    #[must_use]
    pub fn raw_admission_original(&self) -> &[u8] {
        &self.raw_admission_original
    }
    /// Retained raw bytes whose SHA256 is signed in E and authenticated by the raw issuer.
    #[must_use]
    pub fn raw_attestation_original(&self) -> &[u8] {
        &self.raw_attestation_original
    }
    /// Commitment of complete original attestation and unmodified original possession bytes.
    #[must_use]
    pub const fn platform_evidence_digest(&self) -> [u8; 32] {
        self.platform_evidence_digest
    }
    /// Check the exact original interval and reject time regression; no lease renewal occurs.
    /// # Errors
    /// Rejects an observation before authentication or at/after original expiry.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<(), String> {
        if now < self.authenticated_at_ms
            || now < self.challenge.issued_at_ms
            || now >= self.challenge.expires_at_ms
        {
            return Err("enrollment possession original interval expired".into());
        }
        Ok(())
    }
    /// Bind a separately issuer-authenticated final credential to these exact originals.
    /// Native pending consumption/current qualification remains mandatory and is not performed here.
    /// # Errors
    /// Rejects any complete-C scope/key/evidence or original platform-counter substitution.
    pub fn bind_credential(
        &self,
        credential: &super::KagemushaVerifiedOrdinaryAppCredentialV1,
        now: u64,
    ) -> Result<(), String> {
        self.recheck_at_trusted_time(now)?;
        credential.recheck_at_trusted_time(now)?;
        let s = credential.subject();
        let c = &self.enrollment_challenge;
        if credential.identity_policy_id() != self.identity_policy_id
            || credential.identity_policy_original() != self.identity_policy_original
            || credential.identity_authority_original() != self.identity_authority_original
            || credential.preparation_original() != self.preparation_original
            || s.platform_class != c.platform_class
            || s.enrollment_id != c.enrollment_id
            || s.client_nonce != c.client_nonce
            || s.server_nonce != c.server_nonce
            || s.account_binding != c.account_binding
            || s.network_id != c.network_id
            || s.lane_id != c.lane_id
            || s.release_id != c.release_id
            || s.hardware_profile_id != c.hardware_profile_id
            || s.suite_id != c.suite_id
            || s.trust_policy_digest != c.trust_policy_digest
            || s.app_authority_policy_digest != c.app_authority_policy_digest
            || s.financial_authority_commitment != c.financial_authority_commitment
            || s.policy_epoch != c.policy_epoch
            || s.hardware_epoch != c.hardware_epoch
            || s.enrollment_challenge_digest != c.attestation_challenge()?
            || s.app_public_key != self.raw_subject.app_public_key
            || s.app_signing_identity_digest != self.raw_subject.app_signing_identity_digest
            || s.security_level != self.raw_subject.security_level
            || s.attested_key_id != self.challenge.attested_key_id
            || s.platform_evidence_digest != self.platform_evidence_digest
            || self
                .app_attest_counter
                .map_or(s.app_attest_counter_floor != 0, |counter| {
                    s.app_attest_counter_floor != counter
                })
        {
            return Err("ordinary credential differs from joined possession originals".into());
        }
        Ok(())
    }
}
impl KagemushaAppEnrollmentPossessionV1 {
    /// Verify the original platform signature only after authenticating exact signed C and
    /// joining its governed raw-attestation admission to the retained actual raw bytes.
    /// The independent issuer key, expected C and opaque raw admission must be held by the
    /// native pending owner. No native owner, durable consumption or financial grant is returned.
    /// # Errors
    /// Rejects changed C/issuer/key/raw evidence/purpose/interval/platform originals.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate(
        &self,
        preparation: &super::KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
        core_issuer_key: &iroha_crypto::PublicKey,
        expected_c: &super::KagemushaOrdinaryAppEnrollmentChallengeV1,
        raw_admission: &super::KagemushaVerifiedRawAppAttestationAdmissionV1,
        raw_attestation: &[u8],
        trusted_now_ms: u64,
    ) -> Result<KagemushaVerifiedAppEnrollmentPossessionV1, String> {
        if core_issuer_key != raw_admission.enrollment_issuer_key()
            || preparation.to_transport_bytes()? != raw_admission.preparation_original()
        {
            return Err("possession differs from independently admitted Core preparation".into());
        }
        preparation.authenticate(core_issuer_key, expected_c, trusted_now_ms)?;
        raw_admission.recheck_at_trusted_time(trusted_now_ms)?;
        if raw_attestation != raw_admission.platform_original_bytes()
            || raw_admission.subject().enrollment_challenge_digest
                != expected_c.attestation_challenge()?
        {
            return Err("possession differs from complete original platform evidence or C".into());
        }
        let message = super::kagemusha_ordinary_app_enrollment_possession_message_v1(
            expected_c,
            &raw_admission.subject().app_public_key,
            raw_admission.subject().raw_platform_evidence_digest,
        )?;
        let raw = raw_admission.subject();
        let expected = KagemushaAppEnrollmentPossessionChallengeV1::from_signing_bytes(&message)?;
        if self.challenge != expected {
            return Err("enrollment possession original E differs".into());
        }
        let floor = (raw.platform_class == KagemushaHardwarePlatformClassV1::AppleAppAttest)
            .then_some(raw.original_app_attest_counter);
        let (counter, release_measurement) = self.evidence.authenticate_signature(
            raw.platform_class,
            &raw.app_public_key,
            raw.app_signing_identity_digest,
            raw_admission.app_release_digest(),
            floor,
            &message,
        )?;
        let original =
            norito::encode_canonical(self).map_err(|_| "possession canonical encoding failed")?;
        if original.len() > KAGEMUSHA_APP_ENROLLMENT_POSSESSION_MAX_BYTES_V1 + 1024 {
            return Err("possession archive oversized".into());
        }
        let raw_possession = match &self.evidence {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                signature_der.as_slice()
            }
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
                raw_assertion.as_slice()
            }
        };
        let platform_evidence_digest = super::kagemusha_ordinary_app_enrollment_evidence_digest_v1(
            raw_attestation,
            raw_possession,
        )?;
        let mut digest = Sha256::new();
        digest.update(b"iroha:kagemusha:v1:app-enrollment-possession-original\0");
        digest.update((original.len() as u64).to_le_bytes());
        digest.update(&original);
        Ok(KagemushaVerifiedAppEnrollmentPossessionV1 {
            challenge: expected,
            identity_policy_id: raw_admission.identity_policy_id(),
            identity_policy_original: raw_admission.identity_policy_original().to_vec(),
            identity_authority_original: raw_admission.identity_authority_original().to_vec(),
            original,
            preparation_original: preparation.to_transport_bytes()?,
            raw_admission_original: raw_admission.original().to_vec(),
            raw_attestation_original: raw_attestation.to_vec(),
            enrollment_challenge: *expected_c,
            platform_evidence_digest,
            raw_subject: *raw,
            authenticated_at_ms: trusted_now_ms,
            digest: digest.finalize().into(),
            app_attest_counter: counter,
            app_attest_release_measurement: release_measurement,
        })
    }
    /// Decode a single exact bounded canonical archive; no tail or alternate framing is admitted.
    /// Decoding grants no original-owner or platform authority.
    /// # Errors
    /// Rejects bound, decoding or exact re-encoding mismatch.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_APP_ENROLLMENT_POSSESSION_MAX_BYTES_V1 + 1024
        {
            return Err("possession archive bound differs".into());
        }
        let value: Self =
            norito::decode_from_bytes(bytes).map_err(|_| "possession archive decode failed")?;
        value.challenge.canonical_signing_bytes()?;
        let canonical =
            norito::encode_canonical(&value).map_err(|_| "possession archive encode failed")?;
        if canonical != bytes {
            return Err("possession archive is not exact canonical".into());
        }
        Ok(value)
    }
}

impl KagemushaAppEnrollmentPossessionChallengeV1 {
    /// Construct E from exact original C, point and full raw-attestation digest.
    ///
    /// This is a codec formatter, not issuer, native identity or monetary admission. The
    /// original C interval is retained exactly and cannot be renewed by a retry.
    /// # Errors
    /// Rejects malformed C, original point or missing raw platform attestation digest.
    pub fn from_original_enrollment(
        original: &super::KagemushaOrdinaryAppEnrollmentChallengeV1,
        key: &KagemushaDevicePublicKeyV1,
        raw_platform_evidence_digest: [u8; 32],
    ) -> Result<Self, String> {
        original.canonical_signing_bytes()?;
        key.validate().map_err(|_| "E original point rejected")?;
        let challenge = Self {
            version: 1,
            enrollment_attempt_id: original.attestation_challenge()?,
            client_nonce: original.client_nonce,
            server_nonce: original.server_nonce,
            account_binding: original.account_binding,
            network_id: original.network_id,
            app_authority_policy_digest: original.app_authority_policy_digest,
            release_id: original.release_id,
            hardware_profile_id: original.hardware_profile_id,
            lane_id: original.lane_id,
            attested_key_id: Sha256::digest(key.as_sec1_bytes()).into(),
            raw_platform_evidence_digest,
            issued_at_ms: original.issued_at_ms,
            expires_at_ms: original.expires_at_ms,
        };
        challenge.canonical_signing_bytes()?;
        Ok(challenge)
    }
}

/// Derive the single ordinary Android alias from exact C451, including its original domain.
/// This name is correlation data; native custody independently retains the generation original.
/// # Errors
/// Rejects malformed C or another platform. No retired preparation/layout is accepted.
pub fn kagemusha_ordinary_android_app_key_alias_v1(
    original: &super::KagemushaOrdinaryAppEnrollmentChallengeV1,
) -> Result<String, String> {
    use core::fmt::Write as _;

    if original.platform_class != KagemushaHardwarePlatformClassV1::AndroidKeyMint {
        return Err("ordinary Android alias platform differs".into());
    }
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:ordinary-app-key-alias\0");
    hash.update(original.canonical_signing_bytes()?);
    let digest: [u8; 32] = hash.finalize().into();
    let mut alias = String::from("kagemusha-ordinary-app-v1-");
    for byte in digest {
        write!(&mut alias, "{byte:02x}").map_err(|_| "ordinary alias encoding failed")?;
    }
    Ok(alias)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::{SigningKey, signature::Signer as _};
    fn challenge(key: &KagemushaDevicePublicKeyV1) -> KagemushaAppEnrollmentPossessionChallengeV1 {
        KagemushaAppEnrollmentPossessionChallengeV1 {
            version: 1,
            enrollment_attempt_id: [1; 32],
            client_nonce: [2; 32],
            server_nonce: [3; 32],
            account_binding: [4; 32],
            network_id: [5; 32],
            app_authority_policy_digest: [6; 32],
            release_id: [7; 32],
            hardware_profile_id: [8; 32],
            lane_id: [9; 32],
            attested_key_id: Sha256::digest(key.as_sec1_bytes()).into(),
            raw_platform_evidence_digest: [11; 32],
            issued_at_ms: 1000,
            expires_at_ms: 121000,
        }
    }
    fn joined_fixture(
        apple: bool,
    ) -> (
        crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1,
        super::super::KagemushaVerifiedRawAppAttestationAdmissionV1,
        KagemushaAppEnrollmentPossessionV1,
    ) {
        use iroha_crypto::{Algorithm, KeyPair, Signature};
        let f = crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
        let c = &f.selection.preparation.challenge;
        let app = &f.selection.issuance.credential.subject;
        let subject = super::super::KagemushaRawAppAttestationAdmissionSubjectV1 {
            version: 1,
            enrollment_challenge_digest: c.attestation_challenge().unwrap(),
            authority_policy_digest: c.app_authority_policy_digest,
            platform_class: c.platform_class,
            security_level: app.security_level,
            app_public_key: app.app_public_key,
            attested_key_id: app.attested_key_id,
            raw_platform_evidence_digest: Sha256::digest(&f.proof.raw_attestation).into(),
            app_signing_identity_digest: f.app_authority.app_signing_identity_digest,
            original_app_attest_counter: 0,
            issued_at_ms: c.issued_at_ms,
            expires_at_ms: c.expires_at_ms,
        };
        let raw = super::super::KagemushaRawAppAttestationAdmissionV1 {
            subject,
            signature: Signature::try_new(
                KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519).private_key(),
                &subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap(),
        }
        .authenticate(
            f.ordinary_policy.identity_policy(),
            &f.checked_preparation().unwrap(),
            &f.proof.raw_attestation,
            300,
        )
        .unwrap();
        let proof = KagemushaAppEnrollmentPossessionV1 {
            challenge: KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
                c,
                &app.app_public_key,
                subject.raw_platform_evidence_digest,
            )
            .unwrap(),
            evidence: f.proof.app_possession.clone(),
        };
        (f, raw, proof)
    }
    #[test]
    fn possession_exact_body_and_purpose_are_not_financial_approval() {
        let signing = SigningKey::from_bytes((&[17; 32]).into()).unwrap();
        let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signing.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let c = challenge(&key);
        let bytes = c.canonical_signing_bytes().unwrap();
        let offset = KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1.len();
        assert_eq!(bytes.len(), offset + 8 + 371);
        assert_eq!(&bytes[offset..offset + 8], &371u64.to_le_bytes());
        assert_eq!(bytes[offset + 10], 1);
        assert_ne!(
            KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1,
            super::super::KAGEMUSHA_APP_OPERATION_APPROVAL_DOMAIN_V1
        );
        assert_eq!(
            norito::decode_from_bytes::<KagemushaAppEnrollmentPossessionChallengeV1>(
                &norito::encode_canonical(&c).unwrap()
            )
            .unwrap(),
            c
        );
    }
    #[test]
    fn possession_android_keeps_raw_signature_and_rejects_substitution_and_expiry() {
        let (f, raw, proof) = joined_fixture(false);
        let c = &f.selection.preparation.challenge;
        let check = |proof: &KagemushaAppEnrollmentPossessionV1,
                     expected: &super::super::KagemushaOrdinaryAppEnrollmentChallengeV1,
                     original: &[u8],
                     now| {
            proof.authenticate(
                &f.selection.preparation,
                &f.ordinary_policy
                    .identity_policy()
                    .policy()
                    .enrollment_issuer_key,
                expected,
                &raw,
                original,
                now,
            )
        };
        let checked = check(&proof, c, &f.proof.raw_attestation, 300).unwrap();
        assert_eq!(
            checked.original(),
            norito::encode_canonical(&proof).unwrap()
        );
        assert_eq!(checked.app_attest_counter(), None);
        assert_eq!(checked.app_attest_release_measurement(), None);
        assert!(check(&proof, c, &f.proof.raw_attestation, c.expires_at_ms).is_err());
        let mut wrong = *c;
        wrong.server_nonce[0] ^= 1;
        assert!(check(&proof, &wrong, &f.proof.raw_attestation, 300).is_err());
        let mut original = f.proof.raw_attestation.clone();
        original[0] ^= 1;
        assert!(check(&proof, c, &original, 300).is_err());
        let mut wrong = proof.clone();
        wrong.evidence = KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
            raw_assertion: vec![0; 37],
        };
        assert!(check(&wrong, c, &f.proof.raw_attestation, 300).is_err());
    }
    #[test]
    fn possession_apple_uses_exact_e_and_original_counter_and_rp() {
        let (f, raw, proof) = joined_fixture(true);
        let c = &f.selection.preparation.challenge;
        let check = |proof: &KagemushaAppEnrollmentPossessionV1| {
            proof.authenticate(
                &f.selection.preparation,
                &f.ordinary_policy
                    .identity_policy()
                    .policy()
                    .enrollment_issuer_key,
                c,
                &raw,
                &f.proof.raw_attestation,
                300,
            )
        };
        let checked = check(&proof).unwrap();
        assert_eq!(checked.app_attest_counter(), Some(11));
        assert_eq!(
            checked.app_attest_release_measurement(),
            Some(super::super::KagemushaAppAttestReleaseMeasurementV1::Unavailable)
        );
        assert_eq!(
            checked.original(),
            norito::encode_canonical(&proof).unwrap()
        );
        // Re-sign wrong RP and non-increasing counter to isolate actual verifier selectors.
        let signing = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        for (rp, counter) in [([13; 32], 11u32), ([2; 32], 0u32)] {
            let mut auth = rp.to_vec();
            auth.push(0x40);
            auth.extend_from_slice(&counter.to_be_bytes());
            let mut nonce = Sha256::new();
            nonce.update(&auth);
            nonce.update(Sha256::digest(
                proof.challenge.canonical_signing_bytes().unwrap(),
            ));
            let sig: p256::ecdsa::Signature = signing.sign(&nonce.finalize());
            let der = sig.to_der();
            let mut assertion = vec![0xa2, 0x71];
            assertion.extend_from_slice(b"authenticatorData");
            assertion.extend_from_slice(&[0x58, 37]);
            assertion.extend_from_slice(&auth);
            assertion.push(0x69);
            assertion.extend_from_slice(b"signature");
            assertion.extend_from_slice(&[0x58, der.as_bytes().len() as u8]);
            assertion.extend_from_slice(der.as_bytes());
            let changed = KagemushaAppEnrollmentPossessionV1 {
                challenge: proof.challenge,
                evidence: KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                    raw_assertion: assertion,
                },
            };
            assert!(check(&changed).is_err());
        }
        let mut changed = proof.clone();
        changed.challenge.raw_platform_evidence_digest[0] ^= 1;
        assert!(check(&changed).is_err());
        changed = proof.clone();
        changed.evidence = KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
            raw_assertion: vec![0; KAGEMUSHA_APP_ENROLLMENT_POSSESSION_MAX_BYTES_V1 + 1],
        };
        assert!(check(&changed).is_err());
        changed.evidence = KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: vec![0; 64],
        };
        assert!(check(&changed).is_err());
    }

    #[test]
    fn possession_formatter_and_alias_retain_original_c_scope_and_reject_other_roles() {
        use super::super::KagemushaOrdinaryAppEnrollmentChallengeV1;
        let signing = SigningKey::from_bytes((&[17; 32]).into()).unwrap();
        let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signing.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let mut c = KagemushaOrdinaryAppEnrollmentChallengeV1 {
            version: 1,
            platform_class: KagemushaHardwarePlatformClassV1::AndroidKeyMint,
            enrollment_id: [1; 32],
            client_nonce: [2; 32],
            server_nonce: [3; 32],
            account_binding: [4; 32],
            network_id: [5; 32],
            lane_id: [6; 32],
            release_id: [7; 32],
            hardware_profile_id: [8; 32],
            suite_id: [9; 32],
            trust_policy_digest: [10; 32],
            app_authority_policy_digest: [11; 32],
            financial_authority_commitment: [12; 32],
            issuer_policy_digest: [13; 32],
            policy_epoch: 1,
            hardware_epoch: 1,
            issued_at_ms: 1000,
            expires_at_ms: 121000,
        };
        let e = KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
            &c, &key, [14; 32],
        )
        .unwrap();
        assert_eq!(
            (e.issued_at_ms, e.expires_at_ms),
            (c.issued_at_ms, c.expires_at_ms)
        );
        assert_eq!(e.enrollment_attempt_id, c.attestation_challenge().unwrap());
        assert_ne!(e.enrollment_attempt_id, c.enrollment_id);
        assert_eq!(e.raw_platform_evidence_digest, [14; 32]);
        assert_eq!(e.app_authority_policy_digest, c.app_authority_policy_digest);
        assert_eq!(
            super::super::kagemusha_ordinary_app_enrollment_possession_message_v1(
                &c, &key, [14; 32]
            )
            .unwrap(),
            e.canonical_signing_bytes().unwrap()
        );
        let original_message = e.canonical_signing_bytes().unwrap();
        let original_signature: p256::ecdsa::Signature = signing.sign(&original_message);
        for field in 0..4 {
            let mut substituted = c;
            match field {
                0 => substituted.hardware_epoch += 1,
                1 => substituted.financial_authority_commitment[0] ^= 1,
                2 => substituted.suite_id[0] ^= 1,
                _ => substituted.trust_policy_digest[0] ^= 1,
            }
            let changed = KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
                &substituted,
                &key,
                [14; 32],
            )
            .unwrap();
            assert_ne!(changed.canonical_signing_bytes().unwrap(), original_message);
            use p256::ecdsa::signature::Verifier as _;
            assert!(
                signing
                    .verifying_key()
                    .verify(
                        &changed.canonical_signing_bytes().unwrap(),
                        &original_signature,
                    )
                    .is_err()
            );
            assert_ne!(
                substituted.attestation_challenge().unwrap(),
                c.attestation_challenge().unwrap()
            );
        }
        let mut stable_id = e;
        stable_id.enrollment_attempt_id = c.enrollment_id;
        let stable_signature: p256::ecdsa::Signature =
            signing.sign(&stable_id.canonical_signing_bytes().unwrap());
        use p256::ecdsa::signature::Verifier as _;
        assert!(
            signing
                .verifying_key()
                .verify(&original_message, &stable_signature)
                .is_err()
        );
        let alias = kagemusha_ordinary_android_app_key_alias_v1(&c).unwrap();
        assert!(alias.starts_with("kagemusha-ordinary-app-v1-"));
        assert_eq!(alias.len(), "kagemusha-ordinary-app-v1-".len() + 64);
        assert!(
            alias["kagemusha-ordinary-app-v1-".len()..]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
        c.server_nonce = [15; 32];
        assert_ne!(
            kagemusha_ordinary_android_app_key_alias_v1(&c).unwrap(),
            alias
        );
        assert!(
            KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
                &c, &key, [0; 32]
            )
            .is_err()
        );
        c.platform_class = KagemushaHardwarePlatformClassV1::AppleAppAttest;
        assert!(kagemusha_ordinary_android_app_key_alias_v1(&c).is_err());
        c.client_nonce = c.server_nonce;
        assert!(
            KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
                &c, &key, [14; 32]
            )
            .is_err()
        );
    }
}
