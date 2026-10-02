//! Ordinary-app enrollment bound to an independently selected wallet and native financial owner.
//!
//! The platform approval key, wallet Ed25519 key and native financial secret have distinct roles.
//! Admission checks genuine signed release/app credential originals and both possession proofs.
//! The service must separately retain and consume its exact pending challenge, enforce current
//! account approval and unique lane ownership, and publish the original certificate durably.

use super::{
    KagemushaAppOperationApprovalEvidenceV1, KagemushaAuthenticatedReleaseV1,
    KagemushaHardwarePlatformClassV1, KagemushaOrdinaryAppCredentialV1,
    KagemushaRetailEnrollmentIssuerPolicyV1, KagemushaRetailEnrollmentOwnerV1,
    KagemushaSignedOrdinaryAppEnrollmentChallengeV1, KagemushaVerifiedOrdinaryAppCredentialV1,
    kagemusha_ordinary_app_account_binding_v1,
    kagemusha_ordinary_app_enrollment_evidence_digest_v1,
    kagemusha_ordinary_app_enrollment_possession_message_v1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{Algorithm, SignatureOf};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Complete canonical certificate/challenge bound.
pub const KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1: usize = 32 * 1024;
/// Complete canonical proof bound, including bounded original platform attestation.
pub const KAGEMUSHA_ORDINARY_RETAIL_POSSESSION_MAX_BYTES_V1: usize = 160 * 1024;
const ACCOUNT_DOMAIN: &str = "iroha:kagemusha:v1:ordinary-retail-account-possession";
const CERTIFICATE_DOMAIN: &str = "iroha:kagemusha:v1:ordinary-retail-enrollment-approval";
const EVIDENCE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-retail-possession-original\0";
const POLICY_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-retail-issuer-policy\0";

/// Exact ordinary issuance originals; no compact OEM credential is accepted here.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryRetailEnrollmentIssuanceV1")]
pub struct KagemushaOrdinaryRetailEnrollmentIssuanceV1 {
    /// Independently authenticated release identity.
    pub release_id: [u8; 32],
    /// Exact catalog policy digest, distinct from provider policy root.
    pub hardware_policy_digest: [u8; 32],
    /// Independently held Core command signer reference, distinct from the app key.
    pub core_authorization_key_reference: [u8; 32],
    /// Original Ed-signed ordinary credential with separate native financial commitment.
    pub credential: KagemushaOrdinaryAppCredentialV1,
}

/// Public correlation pins retained before accepting a proof or certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaOrdinaryRetailEnrollmentSelectionV1 {
    /// Exact native-selected wallet/FI/runtime/asset-incarnation/lane owner.
    pub owner: KagemushaRetailEnrollmentOwnerV1,
    /// Exact native-selected release, Core signer and credential originals.
    pub issuance: KagemushaOrdinaryRetailEnrollmentIssuanceV1,
    /// Original Core-signed preparation from the retained native attempt.
    pub preparation: KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
}

/// Actual server challenge for the complete wallet and app-key possession decision.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryRetailEnrollmentChallengeV1")]
pub struct KagemushaOrdinaryRetailEnrollmentChallengeV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Exact independently selected native scope and originals.
    pub owner: KagemushaRetailEnrollmentOwnerV1,
    /// Exact original issuance selection.
    pub issuance: KagemushaOrdinaryRetailEnrollmentIssuanceV1,
    /// Full original signed preparation; its nonces and native financial epoch are retained.
    pub preparation: KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
    /// Inclusive native issue time, no earlier than credential issuance.
    pub issued_at_ms: u64,
    /// Exclusive native deadline, bounded by the original preparation and credential.
    pub expires_at_ms: u64,
}

/// Wallet signing payload; the account signer signs its actual typed `HashOf` projection.
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
    name = "iroha_data_model::kagemusha::KagemushaOrdinaryRetailEnrollmentAccountProofV1"
)]
pub struct KagemushaOrdinaryRetailEnrollmentAccountProofV1 {
    /// Sole protocol purpose, checked by the model.
    pub domain: String,
    /// Complete original native challenge.
    pub challenge: KagemushaOrdinaryRetailEnrollmentChallengeV1,
}

/// Original wallet and platform possession evidence, supplied to the retained pending ceremony.
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
    name = "iroha_data_model::kagemusha::KagemushaOrdinaryRetailEnrollmentPossessionProofV1"
)]
pub struct KagemushaOrdinaryRetailEnrollmentPossessionProofV1 {
    /// Exact original challenge; responses cannot select another owner or nonce.
    pub challenge: KagemushaOrdinaryRetailEnrollmentChallengeV1,
    /// Genuine wallet account Ed25519 signature over the purpose-specific typed payload.
    pub account_signature: SignatureOf<KagemushaOrdinaryRetailEnrollmentAccountProofV1>,
    /// Full platform attestation original committed by the independently verified credential.
    pub raw_attestation: Vec<u8>,
    /// Original enrollment possession signature/assertion under that same attested app key.
    pub app_possession: KagemushaAppOperationApprovalEvidenceV1,
}

/// Checked exact possession originals. No decoder, constructor or clone grants this result.
pub struct KagemushaVerifiedOrdinaryRetailEnrollmentPossessionV1 {
    challenge: KagemushaOrdinaryRetailEnrollmentChallengeV1,
    original: Vec<u8>,
    evidence_digest: [u8; 32],
    authenticated_at_ms: u64,
    app_attest_counter: Option<u32>,
    app_attest_release_measurement: Option<super::KagemushaAppAttestReleaseMeasurementV1>,
}
impl KagemushaVerifiedOrdinaryRetailEnrollmentPossessionV1 {
    /// Borrow the independently checked original challenge.
    #[must_use]
    pub fn challenge(&self) -> &KagemushaOrdinaryRetailEnrollmentChallengeV1 {
        &self.challenge
    }
    /// Borrow all canonical original possession evidence.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Model-owned digest of the complete canonical original proof.
    #[must_use]
    pub const fn evidence_digest(&self) -> [u8; 32] {
        self.evidence_digest
    }
    /// Original verified Apple possession counter, independent of financial logical sequence.
    #[must_use]
    pub const fn app_attest_counter(&self) -> Option<u32> {
        self.app_attest_counter
    }
    /// Actual signed Apple release measurement; no version is claimed for the limited form.
    #[must_use]
    pub const fn app_attest_release_measurement(
        &self,
    ) -> Option<super::KagemushaAppAttestReleaseMeasurementV1> {
        self.app_attest_release_measurement
    }
    /// Native supplied instant when the original current interval was checked.
    #[must_use]
    pub const fn authenticated_at_ms(&self) -> u64 {
        self.authenticated_at_ms
    }
    /// Recheck the same exclusive interval; this cannot renew an expired challenge.
    /// # Errors
    /// Rejects time regression or expiration.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<(), String> {
        if now < self.authenticated_at_ms
            || now < self.challenge.issued_at_ms
            || now >= self.challenge.expires_at_ms
        {
            return Err("ordinary retail possession interval expired".into());
        }
        Ok(())
    }
}

/// Issuer's original account-to-native-lane assertion, including every separate key binding.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryRetailEnrollmentSubjectV1")]
pub struct KagemushaOrdinaryRetailEnrollmentSubjectV1 {
    /// Sole first-release format.
    pub version: u16,
    /// Stable owner enrollment identity, derived from the complete owner/runtime.
    pub enrollment_id: [u8; 32],
    /// Independently selected FI enrollment issuer policy.
    pub issuer_policy_id: [u8; 32],
    /// Exact delegated issuer purpose/audience.
    pub issuer_audience: iroha_model_base::name::Name,
    /// Independently selected wallet/FI/runtime/asset-incarnation/lane owner.
    pub owner: KagemushaRetailEnrollmentOwnerV1,
    /// Original release, Core signer and ordinary app credential.
    pub issuance: KagemushaOrdinaryRetailEnrollmentIssuanceV1,
    /// Exact digest of the complete original possession evidence.
    pub challenge_evidence_digest: [u8; 32],
    /// Model digest of the independently verified complete ordinary credential original.
    pub ordinary_app_credential_digest: [u8; 32],
    /// Inclusive trusted issuer time.
    pub issued_at_ms: u64,
    /// Exclusive original certificate deadline.
    pub expires_at_ms: u64,
}

/// Purpose-separated FI issuer signing payload.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryRetailEnrollmentApprovalV1")]
pub struct KagemushaOrdinaryRetailEnrollmentApprovalV1 {
    /// Sole FI ordinary-enrollment purpose.
    pub domain: String,
    /// Complete immutable issuer assertion.
    pub subject: KagemushaOrdinaryRetailEnrollmentSubjectV1,
}

/// Full original FI certificate. Decoding gives data, not an authenticated native owner.
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
    name = "iroha_data_model::kagemusha::KagemushaOrdinaryRetailEnrollmentCertificateV1"
)]
pub struct KagemushaOrdinaryRetailEnrollmentCertificateV1 {
    /// Exact signed ownership/issuance assertion.
    pub subject: KagemushaOrdinaryRetailEnrollmentSubjectV1,
    /// Genuine Ed25519 FI issuer signature under independently selected policy.
    pub signature: SignatureOf<KagemushaOrdinaryRetailEnrollmentApprovalV1>,
}

/// Current issuer, app credential and possession admission, without monetary publication authority.
pub struct KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1 {
    certificate: KagemushaOrdinaryRetailEnrollmentCertificateV1,
    app_credential: KagemushaVerifiedOrdinaryAppCredentialV1,
    possession: KagemushaVerifiedOrdinaryRetailEnrollmentPossessionV1,
    authenticated_at_ms: u64,
}
impl KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1 {
    /// Borrow the complete authenticated FI certificate.
    #[must_use]
    pub fn certificate(&self) -> &KagemushaOrdinaryRetailEnrollmentCertificateV1 {
        &self.certificate
    }
    /// Borrow the genuine app-authority-admitted credential and financial commitment.
    #[must_use]
    pub fn app_credential(&self) -> &KagemushaVerifiedOrdinaryAppCredentialV1 {
        &self.app_credential
    }
    /// Borrow the exact checked wallet/app possession originals.
    #[must_use]
    pub fn possession(&self) -> &KagemushaVerifiedOrdinaryRetailEnrollmentPossessionV1 {
        &self.possession
    }
    /// Trusted current admission instant.
    #[must_use]
    pub const fn authenticated_at_ms(&self) -> u64 {
        self.authenticated_at_ms
    }
    /// Recheck the original certificate and credential intervals, without a new possession claim.
    /// # Errors
    /// Rejects time regression, certificate expiry or expired credential/Integrity policy.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<(), String> {
        if now < self.authenticated_at_ms
            || now < self.certificate.subject.issued_at_ms
            || now >= self.certificate.subject.expires_at_ms
        {
            return Err("ordinary retail certificate interval expired".into());
        }
        self.app_credential.recheck_at_trusted_time(now)
    }
    /// Recheck the same FI certificate with a separately admitted Integrity refresh lease.
    /// The original enrollment, financial epoch and app credential remain unchanged.
    /// # Errors
    /// Rejects time regression, FI expiry, another credential or an expired lease.
    pub fn recheck_with_integrity_lease(
        &self,
        lease: &super::KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
        now: u64,
    ) -> Result<(), String> {
        if now < self.authenticated_at_ms
            || now < self.certificate.subject.issued_at_ms
            || now >= self.certificate.subject.expires_at_ms
        {
            return Err("ordinary retail certificate interval expired".into());
        }
        self.app_credential.recheck_with_integrity_lease(lease, now)
    }
}

/// Digest of the exact independently governed FI policy selected before issuance.
/// # Errors
/// Rejects an invalid or oversized policy original.
pub fn kagemusha_ordinary_retail_issuer_policy_digest_v1(
    policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
) -> Result<[u8; 32], String> {
    policy.validate().map_err(|e| e.to_string())?;
    Ok(digest(POLICY_DOMAIN, &encode(policy, 8 * 1024)?))
}

impl KagemushaOrdinaryRetailEnrollmentChallengeV1 {
    /// Encode a shape-checked canonical public challenge; this grants no authority.
    /// # Errors
    /// Rejects malformed, incomplete or oversized immutable scope.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.owner.enrollment_id().map_err(|e| e.to_string())?;
        self.preparation.to_transport_bytes()?;
        self.issuance.credential.subject.canonical_signing_bytes()?;
        if self.version != 1
            || self.issuance.core_authorization_key_reference == [0; 32]
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms > 120_000
        {
            return Err("ordinary retail challenge shape differs".into());
        }
        encode(self, KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1)
    }
    /// Purpose-specific typed wallet signing payload.
    /// # Errors
    /// Rejects invalid challenge shape or bounds.
    pub fn account_signing_payload(
        &self,
    ) -> Result<KagemushaOrdinaryRetailEnrollmentAccountProofV1, String> {
        self.canonical_bytes()?;
        Ok(KagemushaOrdinaryRetailEnrollmentAccountProofV1 {
            domain: ACCOUNT_DOMAIN.to_owned(),
            challenge: self.clone(),
        })
    }
    /// Actual `HashOf` bytes for an external wallet Ed25519 signer; do not hash them again.
    /// # Errors
    /// Rejects invalid challenge shape or bounds.
    pub fn account_signing_message(&self) -> Result<[u8; 32], String> {
        Ok(*iroha_crypto::HashOf::new(&self.account_signing_payload()?).as_ref())
    }
    /// Authenticate exact selected originals and current scope before asking either signer.
    /// # Errors
    /// Rejects offered selectors, issuer/catalog/key/epoch mismatch or expired preparation.
    pub fn validate(
        &self,
        expected: &KagemushaOrdinaryRetailEnrollmentSelectionV1,
        policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
        release: &KagemushaAuthenticatedReleaseV1,
        app: &KagemushaVerifiedOrdinaryAppCredentialV1,
        now: u64,
    ) -> Result<(), String> {
        self.canonical_bytes()?;
        validate_selection(expected, policy, release, app, now)?;
        if self.owner != expected.owner
            || self.issuance != expected.issuance
            || self.preparation != expected.preparation
            || self.issued_at_ms < self.preparation.challenge.issued_at_ms
            || self.issued_at_ms < app.subject().issued_at_ms
            || self.expires_at_ms > self.preparation.challenge.expires_at_ms
            || self.expires_at_ms > app.subject().expires_at_ms
            || self.issued_at_ms < policy.valid_from_ms
            || self.expires_at_ms > policy.expires_at_ms
            || now < self.issued_at_ms
            || now >= self.expires_at_ms
        {
            return Err("ordinary retail challenge original scope or time differs".into());
        }
        Ok(())
    }
}

impl KagemushaOrdinaryRetailEnrollmentPossessionProofV1 {
    /// Encode complete bounded original possession data without authenticating it.
    /// # Errors
    /// Rejects another shape, an absent original or role bounds.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.challenge.canonical_bytes()?;
        let raw = raw_possession(&self.app_possession);
        kagemusha_ordinary_app_enrollment_evidence_digest_v1(&self.raw_attestation, raw)?;
        encode(self, KAGEMUSHA_ORDINARY_RETAIL_POSSESSION_MAX_BYTES_V1)
    }
    /// Authenticate wallet Ed and actual enrolled-key possession under exact retained originals.
    /// The caller must independently enforce native one-use ownership, current approval and CAS.
    /// # Errors
    /// Rejects any signature, original attestation/PoP, scope/counter/financial binding or time mismatch.
    #[expect(
        clippy::too_many_arguments,
        reason = "each independently authenticated original, counter floor and trusted time is required at this verification boundary"
    )]
    pub fn authenticate(
        &self,
        expected_challenge: &KagemushaOrdinaryRetailEnrollmentChallengeV1,
        expected: &KagemushaOrdinaryRetailEnrollmentSelectionV1,
        policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
        release: &KagemushaAuthenticatedReleaseV1,
        app: &KagemushaVerifiedOrdinaryAppCredentialV1,
        original_attestation_counter_floor: Option<u32>,
        now: u64,
    ) -> Result<KagemushaVerifiedOrdinaryRetailEnrollmentPossessionV1, String> {
        expected_challenge.validate(expected, policy, release, app, now)?;
        self.authenticate_originals(
            expected_challenge,
            app,
            original_attestation_counter_floor,
            now,
        )
    }
    fn authenticate_originals(
        &self,
        expected: &KagemushaOrdinaryRetailEnrollmentChallengeV1,
        app: &KagemushaVerifiedOrdinaryAppCredentialV1,
        original_floor: Option<u32>,
        now: u64,
    ) -> Result<KagemushaVerifiedOrdinaryRetailEnrollmentPossessionV1, String> {
        let original = self.canonical_bytes()?;
        if self.challenge != *expected
            || now < expected.issued_at_ms
            || now >= expected.expires_at_ms
        {
            return Err("ordinary retail possession original challenge differs".into());
        }
        let controller = expected.owner.account_id.controller();
        let account_key = match controller {
            crate::account::AccountController::Single(key) => key,
            crate::account::AccountController::Multisig(policy)
                if policy.threshold() == 1
                    && policy.members().len() == 1
                    && policy.members()[0].weight() == 1 =>
            {
                policy.members()[0].public_key()
            }
            _ => return Err("ordinary retail account requires exactly one Ed25519 signer".into()),
        };
        if account_key.algorithm() != Algorithm::Ed25519 {
            return Err("ordinary retail wallet key algorithm differs".into());
        }
        self.account_signature
            .verify(account_key, &expected.account_signing_payload()?)
            .map_err(|_| "ordinary retail wallet signature rejected")?;
        let subject = app.subject();
        if subject.platform_evidence_digest
            != kagemusha_ordinary_app_enrollment_evidence_digest_v1(
                &self.raw_attestation,
                raw_possession(&self.app_possession),
            )?
        {
            return Err(
                "ordinary retail platform originals differ from admitted credential".into(),
            );
        }
        let message = kagemusha_ordinary_app_enrollment_possession_message_v1(
            &expected.preparation.challenge,
            &subject.app_public_key,
            Sha256::digest(&self.raw_attestation).into(),
        )?;
        let (counter, release_measurement) = self.app_possession.authenticate_signature(
            subject.platform_class,
            &subject.app_public_key,
            subject.app_signing_identity_digest,
            subject.app_release_digest,
            original_floor,
            &message,
        )?;
        if (subject.platform_class == KagemushaHardwarePlatformClassV1::AndroidKeyMint
            && (counter.is_some() || subject.app_attest_counter_floor != 0))
            || (subject.platform_class == KagemushaHardwarePlatformClassV1::AppleAppAttest
                && counter != Some(subject.app_attest_counter_floor))
        {
            return Err(
                "ordinary retail possession counter differs from credential original".into(),
            );
        }
        Ok(KagemushaVerifiedOrdinaryRetailEnrollmentPossessionV1 {
            challenge: expected.clone(),
            evidence_digest: digest(EVIDENCE_DOMAIN, &original),
            original,
            authenticated_at_ms: now,
            app_attest_counter: counter,
            app_attest_release_measurement: release_measurement,
        })
    }
}

impl KagemushaOrdinaryRetailEnrollmentSubjectV1 {
    /// Exact purpose-separated signing payload for the authorized FI issuer.
    /// # Errors
    /// Rejects another stable identity, incomplete scope or invalid interval.
    pub fn approval_payload(&self) -> Result<KagemushaOrdinaryRetailEnrollmentApprovalV1, String> {
        if self.version != 1
            || self.enrollment_id != self.owner.enrollment_id().map_err(|e| e.to_string())?
            || [
                self.issuer_policy_id,
                self.challenge_evidence_digest,
                self.ordinary_app_credential_digest,
                self.issuance.core_authorization_key_reference,
                self.issuance.release_id,
                self.issuance.hardware_policy_digest,
            ]
            .contains(&[0; 32])
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
        {
            return Err("ordinary retail issuer subject incomplete".into());
        }
        self.issuance.credential.subject.canonical_signing_bytes()?;
        let payload = KagemushaOrdinaryRetailEnrollmentApprovalV1 {
            domain: CERTIFICATE_DOMAIN.to_owned(),
            subject: self.clone(),
        };
        encode(&payload, KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1)?;
        Ok(payload)
    }
}
impl KagemushaOrdinaryRetailEnrollmentCertificateV1 {
    /// Complete bounded canonical certificate original.
    /// # Errors
    /// Rejects invalid shape or oversized canonical output.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.subject.approval_payload()?;
        encode(self, KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1)
    }
    /// Authenticate current FI certificate, exact ordinary app original and actual possession.
    /// The inputs must originate from the native retained enrollment ceremony/current issuer policy.
    /// # Errors
    /// Rejects foreign owner/runtime/asset/key/credential/possession, issuer signature or current time.
    pub fn authenticate(
        &self,
        expected: &KagemushaOrdinaryRetailEnrollmentSelectionV1,
        policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
        release: &KagemushaAuthenticatedReleaseV1,
        app: KagemushaVerifiedOrdinaryAppCredentialV1,
        possession: KagemushaVerifiedOrdinaryRetailEnrollmentPossessionV1,
        now: u64,
    ) -> Result<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1, String> {
        self.canonical_bytes()?;
        validate_selection(expected, policy, release, &app, now)?;
        possession.recheck_at_trusted_time(now)?;
        let s = &self.subject;
        let c = possession.challenge();
        if s.owner != expected.owner
            || s.issuance != expected.issuance
            || c.owner != s.owner
            || c.issuance != s.issuance
            || c.preparation != expected.preparation
            || s.enrollment_id != expected.owner.enrollment_id().map_err(|e| e.to_string())?
            || s.issuer_policy_id != policy.issuer_policy_id
            || s.issuer_audience != policy.issuer_audience
            || s.challenge_evidence_digest != possession.evidence_digest()
            || s.ordinary_app_credential_digest != app.digest()
            || s.issued_at_ms < c.issued_at_ms
            || s.issued_at_ms >= c.expires_at_ms
            || s.issued_at_ms < app.subject().issued_at_ms
            || s.expires_at_ms > app.subject().expires_at_ms
            || s.issued_at_ms < policy.valid_from_ms
            || s.expires_at_ms > policy.expires_at_ms
            || s.expires_at_ms - s.issued_at_ms > policy.maximum_certificate_lifetime_ms
            || now < s.issued_at_ms
            || now >= s.expires_at_ms
        {
            return Err(
                "ordinary retail issuer original scope, evidence or interval differs".into(),
            );
        }
        self.signature
            .verify(&policy.issuer_public_key, &s.approval_payload()?)
            .map_err(|_| "ordinary retail FI issuer signature rejected")?;
        Ok(KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1 {
            certificate: self.clone(),
            app_credential: app,
            possession,
            authenticated_at_ms: now,
        })
    }
}

fn validate_selection(
    expected: &KagemushaOrdinaryRetailEnrollmentSelectionV1,
    policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
    release: &KagemushaAuthenticatedReleaseV1,
    app: &KagemushaVerifiedOrdinaryAppCredentialV1,
    now: u64,
) -> Result<(), String> {
    policy.validate().map_err(|e| e.to_string())?;
    app.recheck_at_trusted_time(now)?;
    let p = &expected.preparation.challenge;
    let subject = app.subject();
    let credential_original = encode(&expected.issuance.credential, 16 * 1024)?;
    if expected.owner.runtime != policy.runtime
        || expected.owner.enrollment_id().map_err(|e| e.to_string())? != p.enrollment_id
        || p.issuer_policy_digest != kagemusha_ordinary_retail_issuer_policy_digest_v1(policy)?
        || p.account_binding
            != kagemusha_ordinary_app_account_binding_v1(&expected.owner.account_id)
        || p.network_id != *expected.owner.runtime.network_id.as_bytes()
        || p.lane_id != expected.owner.lane_id
        || expected.issuance.release_id != release.release_id()
        || expected.issuance.hardware_policy_digest != release.hardware_policy_digest()
        || expected.issuance.core_authorization_key_reference == [0; 32]
        || credential_original != app.original()
        || subject.account_binding != p.account_binding
        || subject.financial_authority_commitment != p.financial_authority_commitment
        || subject.enrollment_id != p.enrollment_id
        || subject.hardware_epoch != p.hardware_epoch
        || subject.release_id != release.release_id()
        || subject.network_id != *release.network_id().as_bytes()
        || now < policy.valid_from_ms
        || now >= policy.expires_at_ms
    {
        return Err(
            "ordinary retail native selection differs from actual policy/owner/credential".into(),
        );
    }
    expected
        .preparation
        .authenticate(&policy.issuer_public_key, p, now)
}
fn raw_possession(evidence: &KagemushaAppOperationApprovalEvidenceV1) -> &[u8] {
    match evidence {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => signature_der,
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => raw_assertion,
    }
}
fn encode<T: norito::NoritoSerialize>(value: &T, maximum: usize) -> Result<Vec<u8>, String> {
    let bytes = norito::encode_canonical(value).map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err("ordinary retail canonical archive bound exceeded".into());
    }
    Ok(bytes)
}
fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    //! Actual public model admission under known-public synthetic release/attestation fixtures.
    use super::*;
    use crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    use iroha_crypto::{KeyPair, SignatureOf};

    #[test]
    fn ordinary_retail_both_platforms_admit_actual_dual_possession_and_original_roundtrips() {
        for apple in [false, true] {
            let f = Fixture::new(apple);
            let verified = f.verify(300).unwrap();
            assert_eq!(verified.certificate(), &f.certificate);
            assert_eq!(
                verified.app_credential().original(),
                norito::encode_canonical(&f.selection.issuance.credential).unwrap()
            );
            assert_eq!(
                verified.possession().original(),
                f.proof.canonical_bytes().unwrap()
            );
            assert_eq!(
                verified.possession().app_attest_counter(),
                if apple { Some(11) } else { None }
            );
            assert_eq!(
                verified
                    .app_credential()
                    .subject()
                    .financial_authority_commitment,
                [19; 32]
            );
            assert_ne!(
                verified
                    .app_credential()
                    .subject()
                    .financial_authority_commitment,
                verified.app_credential().subject().attested_key_id
            );
            for bytes in [
                f.certificate.canonical_bytes().unwrap(),
                f.proof.canonical_bytes().unwrap(),
            ] {
                assert!(!bytes.is_empty());
            }
            let bytes = f.certificate.canonical_bytes().unwrap();
            let decoded: KagemushaOrdinaryRetailEnrollmentCertificateV1 =
                norito::decode_canonical_with_limits(
                    &bytes,
                    norito::canonical_decode_limits(bytes.len()),
                )
                .unwrap();
            assert_eq!(decoded, f.certificate);
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(
                norito::decode_canonical::<KagemushaOrdinaryRetailEnrollmentCertificateV1>(
                    &trailing
                )
                .is_err()
            );
            let bytes = f.proof.canonical_bytes().unwrap();
            let decoded: KagemushaOrdinaryRetailEnrollmentPossessionProofV1 =
                norito::decode_canonical_with_limits(
                    &bytes,
                    norito::canonical_decode_limits(bytes.len()),
                )
                .unwrap();
            assert_eq!(decoded, f.proof);
        }
    }

    #[test]
    fn ordinary_retail_re_signed_fi_cannot_substitute_owner_core_key_asset_or_credential() {
        let mut f = Fixture::new(false);
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let original = f.certificate.clone();
        for selector in 0..5 {
            f.certificate = original.clone();
            match selector {
                0 => {
                    f.certificate
                        .subject
                        .issuance
                        .core_authorization_key_reference = [31; 32]
                }
                1 => {
                    f.certificate.subject.owner.runtime.ledger_dataspace_id =
                        iroha_model_base::topology::DataSpaceId::new(11)
                }
                2 => {
                    f.certificate
                        .subject
                        .issuance
                        .credential
                        .subject
                        .financial_authority_commitment = [22; 32]
                }
                3 => f.certificate.subject.ordinary_app_credential_digest = [23; 32],
                _ => f.certificate.subject.challenge_evidence_digest = [24; 32],
            }
            f.certificate.subject.enrollment_id =
                f.certificate.subject.owner.enrollment_id().unwrap();
            f.certificate.signature = SignatureOf::try_new(
                issuer.private_key(),
                &f.certificate.subject.approval_payload().unwrap(),
            )
            .unwrap();
            assert!(f.verify(300).is_err(), "re-signed substitution {selector}");
        }
    }

    #[test]
    fn ordinary_retail_possession_cannot_relabel_platform_original_or_wallet() {
        for apple in [false, true] {
            let mut f = Fixture::new(apple);
            let original = f.proof.clone();
            f.proof.raw_attestation.push(0);
            assert!(f.verify(300).is_err());
            f.proof = original.clone();
            let foreign = KeyPair::from_seed(vec![63; 32], Algorithm::Ed25519);
            f.proof.account_signature = SignatureOf::try_new(
                foreign.private_key(),
                &f.challenge.account_signing_payload().unwrap(),
            )
            .unwrap();
            assert!(f.verify(300).is_err());
            f.proof = original;
            f.proof.app_possession = match f.proof.app_possession {
                KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                    KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                        raw_assertion: signature_der,
                    }
                }
                KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
                    KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                        signature_der: raw_assertion,
                    }
                }
            };
            assert!(f.verify(300).is_err());
        }
    }

    #[test]
    fn ordinary_retail_expired_challenge_cannot_create_new_possession_or_renew_certificate() {
        let f = Fixture::new(true);
        let admitted = f.verify(300).unwrap();
        assert!(f.verify(299).is_err());
        assert!(f.verify(2000).is_err());
        assert!(admitted.possession().recheck_at_trusted_time(2000).is_err());
        // A retained issuer certificate interval is separate from current device possession.
        admitted.recheck_at_trusted_time(5000).unwrap();
        assert!(admitted.recheck_at_trusted_time(9000).is_err());
        assert!(admitted.recheck_at_trusted_time(299).is_err());
    }

    #[test]
    fn ordinary_original_encoder_layout_and_signed_financial_epoch_bind_exact_originals() {
        for f in [
            Fixture::new(false),
            Fixture::new(true),
            Fixture::android_with_integrity(),
        ] {
            let credential = &f.selection.issuance.credential;
            let layout = credential.original_preimage_layout().unwrap();
            let original = credential.canonical_bytes().unwrap();
            assert_eq!(layout.original.end - layout.original.start, original.len());
            assert!(layout.signature.end - layout.signature.start >= 64);
            for (position, byte) in layout
                .signature_bytes
                .iter()
                .zip(credential.signature.payload())
            {
                assert_eq!(layout.bytes[*position], None);
                let original_offset = *position - layout.original.start;
                assert_eq!(original[original_offset], *byte);
            }
            assert_eq!(layout.subject_fields.len(), 28);
            assert_eq!(
                layout.play_integrity_fields.is_some(),
                credential.subject.play_integrity.is_some()
            );
            let mut preimage = b"iroha:kagemusha:v1:ordinary-app-credential-original\0".to_vec();
            preimage.extend_from_slice(&(original.len() as u64).to_le_bytes());
            preimage.extend_from_slice(&original);
            let flags = original[39];
            let encoded_selectors = {
                let _flags = norito::core::DecodeFlagsGuard::enter(flags);
                [
                    credential.subject.platform_class.encode(),
                    credential.subject.security_level.encode(),
                ]
            };
            for (positions, raw) in [&layout.platform_class_bytes, &layout.security_level_bytes]
                .into_iter()
                .zip(encoded_selectors)
            {
                assert_eq!(positions.len(), raw.len());
                for (position, byte) in positions.iter().zip(raw) {
                    assert_eq!(preimage[*position], byte);
                    assert_eq!(layout.bytes[*position], None);
                }
            }
            for (position, byte) in layout
                .version_bytes
                .iter()
                .zip(credential.subject.version.to_le_bytes())
            {
                assert_eq!(preimage[*position], byte);
                assert_eq!(layout.bytes[*position], None);
            }
            let signing = credential.subject.canonical_signing_bytes().unwrap();
            let signing_body =
                &signing[super::super::KAGEMUSHA_ORDINARY_APP_CREDENTIAL_DOMAIN_V1.len() + 8..];
            for (index, positions) in layout.fixed_digest_bytes.iter().enumerate() {
                let range = &layout.subject_fields[index + 3];
                assert_eq!(range.len(), 32, "declared digest fields contain raw bytes");
                assert_eq!(positions.as_slice(), range.clone().collect::<Vec<_>>());
                let expected = &signing_body[4 + index * 32..4 + (index + 1) * 32];
                for (position, byte) in positions.iter().zip(expected) {
                    assert_eq!(preimage[*position], *byte);
                    assert_eq!(layout.bytes[*position], None);
                }
            }
            for (position, byte) in layout
                .app_public_key_bytes
                .iter()
                .zip(credential.subject.app_public_key.as_sec1_bytes())
            {
                assert_eq!(preimage[*position], *byte);
                assert_eq!(layout.bytes[*position], None);
            }
            let scalars = [
                credential.subject.policy_epoch.to_le_bytes().to_vec(),
                credential.subject.hardware_epoch.to_le_bytes().to_vec(),
                credential.subject.issued_at_ms.to_le_bytes().to_vec(),
                credential.subject.expires_at_ms.to_le_bytes().to_vec(),
                credential
                    .subject
                    .app_attest_counter_floor
                    .to_le_bytes()
                    .to_vec(),
            ];
            for (positions, raw) in layout.scalar_bytes.iter().zip(scalars) {
                for (position, byte) in positions.iter().zip(raw) {
                    assert_eq!(preimage[*position], byte);
                    assert_eq!(layout.bytes[*position], None);
                }
            }
            if let Some(pi) = credential.subject.play_integrity {
                let raw = [
                    pi.request_hash.to_vec(),
                    pi.evidence_digest.to_vec(),
                    pi.policy_digest.to_vec(),
                    pi.verified_at_ms.to_le_bytes().to_vec(),
                    pi.refresh_before_ms.to_le_bytes().to_vec(),
                ];
                for range in &layout.play_integrity_fields.as_ref().unwrap()[..3] {
                    assert_eq!(range.len(), 32, "declared Integrity digests are raw bytes");
                }
                for (positions, raw) in layout
                    .play_integrity_bytes
                    .as_ref()
                    .unwrap()
                    .iter()
                    .zip(raw)
                {
                    assert_eq!(positions.len(), raw.len());
                    for (position, byte) in positions.iter().zip(raw) {
                        assert_eq!(preimage[*position], byte);
                        assert_eq!(layout.bytes[*position], None);
                    }
                }
            }
            for (expected, actual) in layout.bytes.iter().zip(&preimage) {
                if let Some(expected) = expected {
                    assert_eq!(expected, actual)
                }
            }
            assert_eq!(
                <[u8; 32]>::from(Sha256::digest(&preimage)),
                f.verify(300).unwrap().app_credential().digest()
            );
            assert_eq!(
                credential.canonical_digest().unwrap(),
                f.verify(300).unwrap().app_credential().digest()
            );
            let epoch = super::super::kagemusha_ordinary_financial_epoch_id_v1(&credential.subject)
                .unwrap();
            let mut changed = credential.subject;
            changed.hardware_epoch += 1;
            assert_ne!(
                epoch,
                super::super::kagemusha_ordinary_financial_epoch_id_v1(&changed).unwrap()
            );
            changed = credential.subject;
            changed.financial_authority_commitment = [25; 32];
            assert_ne!(
                epoch,
                super::super::kagemusha_ordinary_financial_epoch_id_v1(&changed).unwrap()
            );
        }
    }
    #[test]
    fn ordinary_public_codec_golden_producer_retains_real_known_public_originals() {
        let bytes=crate::testing::ordinary_app_enrollment::kagemusha_ordinary_enrollment_public_codec_golden_v1();
        if let Some(path) = std::env::var_os("KAGEMUSHA_ORDINARY_TEST_GOLDEN_OUTPUT") {
            std::fs::write(path, bytes).unwrap();
        }
    }
}
