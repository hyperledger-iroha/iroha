//! One-use issuer enrollment ceremony with account and governed-device possession.
//!
//! This proof is not a native transient observation, live KYC result, wallet-open lease or
//! successor credential capability. The issuer must retain the exact server-created challenge,
//! enforce one-use durable ownership, and recheck live account/KYC authority before issuance.

use super::kagemusha_retail_enrollment_v1::CatalogBinding;
use super::{
    KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1, KagemushaAuthenticatedReleaseV1,
    KagemushaDeviceQualificationReplyV1, KagemushaDeviceReadCredentialCommandV1,
    KagemushaHardwareProfileV1, KagemushaRetailEnrollmentCertificateV1,
    KagemushaRetailEnrollmentIssuanceV1, KagemushaRetailEnrollmentIssuerPolicyV1,
    KagemushaRetailEnrollmentOwnerV1, KagemushaRetailEnrollmentSelectionV1,
    KagemushaRetailEnrollmentSubjectV1, kagemusha_verify_device_response_v1,
};
use crate::name::Name;

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{Algorithm, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Maximum lifetime of a one-use enrollment challenge.
pub const KAGEMUSHA_RETAIL_ENROLLMENT_CHALLENGE_LIFETIME_MS_V1: u64 = 300_000;
/// Bound applied before parsing a canonical challenge.
pub const KAGEMUSHA_RETAIL_ENROLLMENT_CHALLENGE_MAX_BYTES_V1: usize = 16 * 1024;
/// Bound applied before parsing a proof and its complete signed device frame.
pub const KAGEMUSHA_RETAIL_ENROLLMENT_PROOF_MAX_BYTES_V1: usize =
    KAGEMUSHA_RETAIL_ENROLLMENT_CHALLENGE_MAX_BYTES_V1
        + KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1
        + 1024;
const REQUEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:retail-enrollment-device-challenge";
const ACCOUNT_DOMAIN: &str = "iroha:kagemusha:v1:retail-enrollment-account-possession";
const EVIDENCE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:retail-enrollment-ceremony-evidence";

/// Exact server-created challenge. Decoding grants no issuer or outstanding-challenge authority.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(schema_name = "iroha.kagemusha.v1.retail-enrollment-challenge")]
#[norito(deny_unknown_fields)]
pub struct KagemushaRetailEnrollmentChallengeV1 {
    /// Sole first-release format, 1.
    pub version: u16,
    /// Native client CSPRNG nonce retained before the issuer request. This field alone
    /// proves neither native ownership nor freshness; the native pending owner checks both.
    pub client_nonce: [u8; 32],
    /// Independent issuer CSPRNG one-use challenge, distinct from the client nonce.
    pub server_nonce: [u8; 32],
    /// Exact independently selected issuer policy.
    pub issuer_policy_id: [u8; 32],
    /// Exact purpose-bound issuer audience.
    pub issuer_audience: Name,
    /// Entire canonical account, runtime and stable lane.
    pub owner: KagemushaRetailEnrollmentOwnerV1,
    /// Exact governed credential and release selected at challenge creation.
    pub issuance: KagemushaRetailEnrollmentIssuanceV1,
    /// Inclusive activation in authoritative service Unix milliseconds.
    pub issued_at_ms: u64,
    /// Exclusive bounded one-use challenge deadline.
    pub expires_at_ms: u64,
}

/// Account-controller signing payload with a separate purpose domain.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(schema_name = "iroha.kagemusha.v1.retail-enrollment-account-proof")]
#[norito(deny_unknown_fields)]
pub struct KagemushaRetailEnrollmentAccountProofV1 {
    /// Exact account-possession domain populated by `account_signing_payload`.
    pub domain: String,
    /// Full exact challenge also committed by the device request identity.
    pub challenge: KagemushaRetailEnrollmentChallengeV1,
}

/// Complete dual-possession proof; the challenge must match the server's retained bytes.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(schema_name = "iroha.kagemusha.v1.retail-enrollment-possession-proof")]
#[norito(deny_unknown_fields)]
pub struct KagemushaRetailEnrollmentPossessionProofV1 {
    /// Exact server challenge, never independently trusted from this envelope.
    pub challenge: KagemushaRetailEnrollmentChallengeV1,
    /// Signature under the Ed25519 controller derived from the exact account identity.
    pub account_signature: SignatureOf<KagemushaRetailEnrollmentAccountProofV1>,
    /// Complete command-bound operation-1 response frame, including raw low-S signature.
    pub device_response: Vec<u8>,
}

/// Evidence verified under independent issuer/catalog trust at one service time.
///
/// No public constructor/decoder creates this value. It does not consume the server's
/// challenge, claim live KYC or authorize native observation/monetary dispatch.
#[derive(Debug, PartialEq, Eq)]
pub struct KagemushaVerifiedRetailEnrollmentPossessionV1 {
    challenge: KagemushaRetailEnrollmentChallengeV1,
    evidence_digest: [u8; 32],
    verified_at_ms: u64,
}
impl KagemushaVerifiedRetailEnrollmentPossessionV1 {
    /// Exact verified challenge.
    #[must_use]
    pub fn challenge(&self) -> &KagemushaRetailEnrollmentChallengeV1 {
        &self.challenge
    }
    /// Commitment to the full challenge and both exact proofs for the issuer certificate.
    #[must_use]
    pub fn evidence_digest(&self) -> [u8; 32] {
        self.evidence_digest
    }
    /// Authoritative service time used for this verification.
    #[must_use]
    pub fn verified_at_ms(&self) -> u64 {
        self.verified_at_ms
    }
}

/// Closed ceremony verification failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KagemushaRetailEnrollmentChallengeErrorV1 {
    /// Malformed, oversized or noncanonical bytes.
    Encoding,
    /// Reserved fields, invalid credential shape, version or challenge lifetime.
    Shape,
    /// Exact retained challenge, independent issuer/runtime or catalog mismatch.
    Binding,
    /// Expired/not-yet-active challenge, issuer or governed credential.
    Validity,
    /// Unsupported account controller or invalid account-possession signature.
    AccountProof,
    /// Invalid full command-bound device response or substituted qualification.
    DeviceProof,
    /// Issuer certificate signature, exact proof commitment or issuance scope differs.
    IssuerEvidence,
}
impl core::fmt::Display for KagemushaRetailEnrollmentChallengeErrorV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid retail enrollment ceremony: {self:?}")
    }
}
impl std::error::Error for KagemushaRetailEnrollmentChallengeErrorV1 {}
type Result<T> = core::result::Result<T, KagemushaRetailEnrollmentChallengeErrorV1>;

/// Authenticated issuer decision bound to both possession proofs and one client nonce.
///
/// This is historical evidence, not a current-time certificate, native session, live KYC
/// result or monetary capability. The native owner must compare the independently retained
/// nonce and exact selector and enforce its original continuous deadline before admission.
/// No public constructor or decoder creates this result, and it contains no host time.
#[derive(Debug, PartialEq, Eq)]
pub struct KagemushaVerifiedRetailEnrollmentIssuerEvidenceV1 {
    certificate: KagemushaRetailEnrollmentCertificateV1,
    client_nonce: [u8; 32],
}

impl KagemushaVerifiedRetailEnrollmentIssuerEvidenceV1 {
    /// Exact certificate whose issuer signature commits to the complete verified proof.
    #[must_use]
    pub fn certificate(&self) -> &KagemushaRetailEnrollmentCertificateV1 {
        &self.certificate
    }

    /// Nonce bound by account, device and issuer signatures; possession of these bytes
    /// does not prove that the caller owns the native pending attempt.
    #[must_use]
    pub fn client_nonce(&self) -> [u8; 32] {
        self.client_nonce
    }
}

fn encode<T: norito::NoritoSerialize>(value: &T, maximum: usize) -> Result<Vec<u8>> {
    let bytes = norito::encode_canonical(value)
        .map_err(|_| KagemushaRetailEnrollmentChallengeErrorV1::Encoding)?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(KagemushaRetailEnrollmentChallengeErrorV1::Encoding);
    }
    Ok(bytes)
}
fn decode<T>(bytes: &[u8], maximum: usize) -> Result<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(KagemushaRetailEnrollmentChallengeErrorV1::Encoding);
    }
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
        .map_err(|_| KagemushaRetailEnrollmentChallengeErrorV1::Encoding)
}
fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update([0]);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
impl KagemushaRetailEnrollmentChallengeV1 {
    fn validate_shape(&self) -> Result<()> {
        use KagemushaRetailEnrollmentChallengeErrorV1::Shape;
        if self.client_nonce == [0; 32]
            || self.server_nonce == [0; 32]
            || self.client_nonce == self.server_nonce
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > KAGEMUSHA_RETAIL_ENROLLMENT_CHALLENGE_LIFETIME_MS_V1
        {
            return Err(Shape);
        }
        // Reuse the certificate's exact owner/issuance shape invariants. This local subject
        // is never signed and never constructs verified certificate evidence.
        KagemushaRetailEnrollmentSubjectV1 {
            version: self.version,
            enrollment_id: self.owner.enrollment_id().map_err(|_| Shape)?,
            issuer_policy_id: self.issuer_policy_id,
            issuer_audience: self.issuer_audience.clone(),
            owner: self.owner.clone(),
            issuance: self.issuance.clone(),
            challenge_evidence_digest: self.server_nonce,
            issued_at_ms: self.issued_at_ms,
            expires_at_ms: self.expires_at_ms,
        }
        .approval_payload()
        .map_err(|_| Shape)?;
        Ok(())
    }
    /// Encode the bounded exact challenge without granting it issuer authority.
    /// # Errors
    /// Rejects malformed or oversized challenge fields.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate_shape()?;
        encode(self, KAGEMUSHA_RETAIL_ENROLLMENT_CHALLENGE_MAX_BYTES_V1)
    }
    /// Decode the sole canonical challenge format.
    /// # Errors
    /// Rejects unknown fields, invalid shape, trailing bytes and bounds violations.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self> {
        let challenge: Self = decode(bytes, KAGEMUSHA_RETAIL_ENROLLMENT_CHALLENGE_MAX_BYTES_V1)?;
        challenge.validate_shape()?;
        Ok(challenge)
    }
    /// Derive op1's outer request identity from every exact challenge field.
    ///
    /// This challenge belongs to the issuer ceremony. It MUST NOT be installed as a native
    /// BeginObservation nonce, whose one-use lifecycle is independently native-owned.
    /// # Errors
    /// Rejects invalid challenge fields or a reserved resulting identity.
    pub fn device_request_id(&self) -> Result<[u8; 32]> {
        let id = digest(REQUEST_DOMAIN, &self.canonical_bytes()?);
        if id == [0; 32] {
            return Err(KagemushaRetailEnrollmentChallengeErrorV1::Shape);
        }
        Ok(id)
    }
    /// Build the exact domain-separated account signing payload.
    /// # Errors
    /// Rejects malformed/oversized challenge fields.
    pub fn account_signing_payload(&self) -> Result<KagemushaRetailEnrollmentAccountProofV1> {
        self.canonical_bytes()?;
        Ok(KagemushaRetailEnrollmentAccountProofV1 {
            domain: ACCOUNT_DOMAIN.to_owned(),
            challenge: self.clone(),
        })
    }
    /// Exact message for an external Ed25519 account signer.
    ///
    /// `SignatureOf` signs the typed Norito hash, not the serialized payload itself.
    /// Native clients derive these bytes from the independently checked challenge before
    /// asking their account signer to sign; a service-provided byte string is not authority.
    /// # Errors
    /// Rejects malformed or oversized challenge fields.
    pub fn account_signing_message(&self) -> Result<[u8; 32]> {
        let hash = iroha_crypto::HashOf::new(&self.account_signing_payload()?);
        Ok(*hash.as_ref())
    }
    /// Validate challenge scope against independent service policy and authenticated catalog.
    ///
    /// Callers must also retain the exact server-created nonce and enforce one-use completion.
    /// # Errors
    /// Rejects scope, catalog, credential and trusted-time substitutions.
    pub fn validate(
        &self,
        policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
        release: &KagemushaAuthenticatedReleaseV1,
        trusted_time_ms: u64,
    ) -> Result<()> {
        let profile = release
            .enabled_profile(self.issuance.credential.hardware_profile_id)
            .ok_or(KagemushaRetailEnrollmentChallengeErrorV1::Binding)?;
        self.validate_bound(
            policy,
            release.release_id(),
            release.hardware_policy_digest(),
            &profile.hardware_profile,
            trusted_time_ms,
        )
    }
    fn validate_bound(
        &self,
        policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
        release_id: [u8; 32],
        hardware_policy: [u8; 32],
        profile: &KagemushaHardwareProfileV1,
        time: u64,
    ) -> Result<()> {
        use KagemushaRetailEnrollmentChallengeErrorV1::{Binding, Validity};
        self.canonical_bytes()?;
        policy.validate().map_err(|_| Binding)?;
        if self.issuer_policy_id != policy.issuer_policy_id
            || self.issuer_audience != policy.issuer_audience
            || self.owner.runtime != policy.runtime
            || self.issuance.release_id != release_id
            || self.issuance.hardware_policy_digest != hardware_policy
            || self
                .issuance
                .credential
                .validate_against_profile(profile)
                .is_err()
        {
            return Err(Binding);
        }
        let credential = &self.issuance.credential;
        if time == 0
            || time < self.issued_at_ms
            || time >= self.expires_at_ms
            || self.issued_at_ms < policy.valid_from_ms
            || self.expires_at_ms > policy.expires_at_ms
            || self.issued_at_ms < credential.issued_at_ms
            || self.expires_at_ms > credential.expires_at_ms
        {
            return Err(Validity);
        }
        Ok(())
    }
}
impl KagemushaRetailEnrollmentPossessionProofV1 {
    /// Authenticate a nonce-bound issuer decision without claiming current UTC validity.
    ///
    /// `policy`, `release`, `expected` and `expected_client_nonce` must be independent
    /// native pending inputs. This method cannot enforce native nonce ownership or elapsed
    /// time; its opaque result supplies only the cryptographic evidence for those checks.
    /// The signed issuer instant checks historical challenge validity and never creates
    /// `KagemushaVerifiedRetailEnrollmentCertificateV1` or a hardware commit time.
    ///
    /// # Errors
    /// Rejects any nonce, scope, catalog, certificate/proof signature, evidence digest or
    /// historical issuance interval mismatch.
    pub fn authenticate_issuer_evidence(
        &self,
        certificate: &KagemushaRetailEnrollmentCertificateV1,
        policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
        release: &KagemushaAuthenticatedReleaseV1,
        expected: &KagemushaRetailEnrollmentSelectionV1,
        expected_client_nonce: [u8; 32],
    ) -> Result<KagemushaVerifiedRetailEnrollmentIssuerEvidenceV1> {
        let profile = release
            .enabled_profile(expected.issuance.credential.hardware_profile_id)
            .ok_or(KagemushaRetailEnrollmentChallengeErrorV1::Binding)?;
        self.authenticate_issuer_evidence_bound(
            certificate,
            policy,
            CatalogBinding {
                release_id: release.release_id(),
                hardware_policy_digest: release.hardware_policy_digest(),
                profile: &profile.hardware_profile,
            },
            expected,
            expected_client_nonce,
        )
    }

    fn authenticate_issuer_evidence_bound(
        &self,
        certificate: &KagemushaRetailEnrollmentCertificateV1,
        policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
        catalog: CatalogBinding<'_>,
        expected: &KagemushaRetailEnrollmentSelectionV1,
        expected_client_nonce: [u8; 32],
    ) -> Result<KagemushaVerifiedRetailEnrollmentIssuerEvidenceV1> {
        use KagemushaRetailEnrollmentChallengeErrorV1::{Binding, IssuerEvidence};
        self.canonical_bytes()?;
        if expected_client_nonce == [0; 32] || self.challenge.client_nonce != expected_client_nonce
        {
            return Err(Binding);
        }
        certificate
            .verify_issuer_bound(policy, catalog, expected)
            .map_err(|_| IssuerEvidence)?;
        let subject = &certificate.subject;
        if subject.owner != self.challenge.owner
            || subject.issuance != self.challenge.issuance
            || subject.issuer_policy_id != self.challenge.issuer_policy_id
            || subject.issuer_audience != self.challenge.issuer_audience
            || subject.challenge_evidence_digest != self.canonical_evidence_digest()?
        {
            return Err(IssuerEvidence);
        }
        // This is the issuer's signed historical decision instant, not a native clock.
        self.challenge.validate_bound(
            policy,
            catalog.release_id,
            catalog.hardware_policy_digest,
            catalog.profile,
            subject.issued_at_ms,
        )?;
        self.authenticate_possession(&self.challenge, catalog.profile, subject.issued_at_ms)?;
        Ok(KagemushaVerifiedRetailEnrollmentIssuerEvidenceV1 {
            certificate: certificate.clone(),
            client_nonce: expected_client_nonce,
        })
    }

    /// Commit to the exact canonical proof bytes without authenticating either signature.
    ///
    /// This digest is a selector/integrity binding only. It does not establish issuer trust,
    /// possession, one-use challenge completion, live KYC or native wallet authority.
    /// # Errors
    /// Rejects malformed or oversized canonical proof fields.
    pub fn canonical_evidence_digest(&self) -> Result<[u8; 32]> {
        Ok(digest(EVIDENCE_DOMAIN, &self.canonical_bytes()?))
    }
    /// Decode a bounded exact proof; this supplies no possession authority.
    /// # Errors
    /// Rejects malformed, oversized and noncanonical encodings.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self> {
        let proof: Self = decode(bytes, KAGEMUSHA_RETAIL_ENROLLMENT_PROOF_MAX_BYTES_V1)?;
        proof.canonical_bytes()?;
        Ok(proof)
    }
    /// Encode a bounded exact proof without authenticating signatures.
    /// # Errors
    /// Rejects invalid challenge/body bounds.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.challenge.canonical_bytes()?;
        if self.device_response.is_empty()
            || self.device_response.len() > KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1
        {
            return Err(KagemushaRetailEnrollmentChallengeErrorV1::Encoding);
        }
        encode(self, KAGEMUSHA_RETAIL_ENROLLMENT_PROOF_MAX_BYTES_V1)
    }
    /// Authenticate both proofs against the exact retained server challenge and catalog.
    ///
    /// The independent issuer must still recheck live MiBank approval/current account and
    /// durably consume the challenge with the unique lane ownership/certificate before reply.
    /// # Errors
    /// Rejects any challenge, account proof, governed device or trusted-time mismatch.
    pub fn authenticate(
        &self,
        expected: &KagemushaRetailEnrollmentChallengeV1,
        policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
        release: &KagemushaAuthenticatedReleaseV1,
        trusted_time_ms: u64,
    ) -> Result<KagemushaVerifiedRetailEnrollmentPossessionV1> {
        expected.validate(policy, release, trusted_time_ms)?;
        let profile = release
            .enabled_profile(expected.issuance.credential.hardware_profile_id)
            .ok_or(KagemushaRetailEnrollmentChallengeErrorV1::Binding)?;
        self.authenticate_possession(expected, &profile.hardware_profile, trusted_time_ms)
    }
    fn authenticate_possession(
        &self,
        expected: &KagemushaRetailEnrollmentChallengeV1,
        profile: &KagemushaHardwareProfileV1,
        time: u64,
    ) -> Result<KagemushaVerifiedRetailEnrollmentPossessionV1> {
        use KagemushaRetailEnrollmentChallengeErrorV1::{AccountProof, Binding, DeviceProof};
        let evidence_digest = self.canonical_evidence_digest()?;
        if self.challenge != *expected {
            return Err(Binding);
        }
        let account_key = expected
            .owner
            .account_id
            .controller()
            .single_signatory()
            .ok_or(AccountProof)?;
        if account_key.algorithm() != Algorithm::Ed25519 {
            return Err(AccountProof);
        }
        self.account_signature
            .verify(account_key, &expected.account_signing_payload()?)
            .map_err(|_| AccountProof)?;
        let command =
            KagemushaDeviceReadCredentialCommandV1::canonical_bytes().map_err(|_| DeviceProof)?;
        let response = kagemusha_verify_device_response_v1(
            &self.device_response,
            &command,
            1,
            expected.device_request_id()?,
            expected.issuance.hardware_policy_digest,
            profile.qualification_report_digest,
            &expected.issuance.credential.device_public_key,
        )
        .map_err(|_| DeviceProof)?;
        let reply = KagemushaDeviceQualificationReplyV1::decode_canonical_exact(response.payload)
            .map_err(|_| DeviceProof)?;
        if reply.release_id != expected.issuance.release_id
            || reply.hardware_policy_digest != expected.issuance.hardware_policy_digest
            || reply.core_authorization_key_reference
                != expected.issuance.core_authorization_key_reference
            || reply.profile != *profile
            || reply.credential != expected.issuance.credential
        {
            return Err(DeviceProof);
        }
        Ok(KagemushaVerifiedRetailEnrollmentPossessionV1 {
            challenge: expected.clone(),
            evidence_digest,
            verified_at_ms: time,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Private catalog-kernel fixtures test real account/device signatures. They do not
    //! construct an authenticated release, enroll physical hardware or test a live issuer.
    use super::*;
    use crate::kagemusha::*;
    use crate::kagemusha::{
        KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KagemushaDevicePublicKeyV1,
        KagemushaDeviceSignatureV1, KagemushaHardwarePlatformClassV1,
        kagemusha_device_key_reference_v1, kagemusha_suite_commitment_v1,
    };
    use crate::{
        NetworkId,
        account::AccountId,
        asset::AssetDefinitionId,
        nexus::{AxtAssetIncarnationV1, DataSpaceId},
    };
    use iroha_crypto::{Hash, HashOf, KeyPair};
    use p256::ecdsa::{SigningKey, signature::Signer as _};

    struct Fixture {
        issuer: KeyPair,
        profile: KagemushaHardwareProfileV1,
        policy: KagemushaRetailEnrollmentIssuerPolicyV1,
        certificate: KagemushaRetailEnrollmentCertificateV1,
        selection: KagemushaRetailEnrollmentSelectionV1,
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn p256_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes((&[seed; 32]).into()).expect("test signing key")
    }

    fn public(key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
        KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("test public key")
    }

    impl Fixture {
        fn new(generation: u8) -> Self {
            let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
            let governance = p256_key(2);
            let device = public(&p256_key(generation + 2));
            let suite = [31; 32];
            let profile = KagemushaHardwareProfileV1 {
                version: 1,
                protocol_version: 1,
                hardware_profile_id: [0; 32],
                provider_id: [1; 32],
                platform_class: KagemushaHardwarePlatformClassV1::OtherQualified,
                product_class_digest: [2; 32],
                firmware_policy_digest: [3; 32],
                enrollment_attestation_verifier_digest: [4; 32],
                attestation_trust_roots_digest: [5; 32],
                allowed_suite_commitment: kagemusha_suite_commitment_v1(suite),
                policy_epoch: 1,
                governance_credential_public_key: public(&governance),
                capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
                qualification_report_digest: [8; 32],
                valid_from_ms: 100,
                expires_at_ms: 10_000,
            }
            .seal_hardware_profile_id()
            .expect("test profile identity");
            let runtime = KagemushaRetailEnrollmentRuntimeV1 {
                fi_id: "mibank".parse().expect("FI name"),
                ledger_dataspace_id: DataSpaceId::new(10),
                authentication_namespace: "mibank.bpng".parse().expect("auth namespace"),
                network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"enrollment-test-genesis"),
                )),
                asset: AssetDefinitionId::from_uuid_bytes([
                    0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84,
                    0xfd, 0xcd, 0x2f,
                ])
                .expect("test asset ID"),
                asset_incarnation: AxtAssetIncarnationV1::try_from_bytes(
                    *Hash::new(b"enrollment-test-incarnation").as_ref(),
                )
                .expect("test asset incarnation"),
                scale: 2,
            };
            let owner = KagemushaRetailEnrollmentOwnerV1 {
                account_id: account(12),
                runtime: runtime.clone(),
                lane_id: [32; 32],
            };
            let mut credential = KagemushaHardwareCredentialV1 {
                version: 1,
                credential_id: [0; 32],
                network_id: runtime.network_id,
                hardware_profile_id: profile.hardware_profile_id,
                suite_id: suite,
                firmware_policy_digest: profile.firmware_policy_digest,
                policy_epoch: profile.policy_epoch,
                lane_commitment: owner.lane_id,
                hardware_epoch_id: [generation; 32],
                hardware_epoch_generation: u64::from(generation),
                device_public_key: device,
                device_key_reference: kagemusha_device_key_reference_v1(&device),
                issued_at_ms: 200,
                expires_at_ms: 9_000,
                governance_signature: KagemushaDeviceSignatureV1::from_raw_bytes(&[1; 64])
                    .expect("placeholder shape"),
            }
            .seal_credential_id()
            .expect("test credential ID");
            let signature: p256::ecdsa::Signature = governance.sign(
                &credential
                    .canonical_signing_bytes()
                    .expect("credential transcript"),
            );
            credential.governance_signature = KagemushaDeviceSignatureV1::from_raw_bytes(
                &signature.normalize_s().unwrap_or(signature).to_bytes(),
            )
            .expect("test credential signature");
            let issuance = KagemushaRetailEnrollmentIssuanceV1 {
                release_id: [40 + generation; 32],
                hardware_policy_digest: [50 + generation; 32],
                core_authorization_key_reference: [60 + generation; 32],
                credential,
            };
            let policy = KagemushaRetailEnrollmentIssuerPolicyV1 {
                version: 1,
                issuer_policy_id: [71; 32],
                issuer_public_key: issuer.public_key().clone(),
                issuer_audience: "test-retail-enrollment-service"
                    .parse()
                    .expect("test audience"),
                runtime,
                valid_from_ms: 100,
                expires_at_ms: 9_000,
                maximum_certificate_lifetime_ms: 4_000,
            };
            let subject = KagemushaRetailEnrollmentSubjectV1 {
                version: 1,
                enrollment_id: owner.enrollment_id().expect("test enrollment ID"),
                issuer_policy_id: policy.issuer_policy_id,
                issuer_audience: policy.issuer_audience.clone(),
                owner,
                issuance: issuance.clone(),
                challenge_evidence_digest: [72; 32],
                issued_at_ms: 1_000,
                expires_at_ms: 3_000,
            };
            let certificate = KagemushaRetailEnrollmentCertificateV1 {
                signature: SignatureOf::try_new(
                    issuer.private_key(),
                    &subject.approval_payload().expect("test subject"),
                )
                .expect("test issuer signature"),
                subject,
            };
            let selection = KagemushaRetailEnrollmentSelectionV1 {
                enrollment_id: certificate.subject.enrollment_id,
                account_id: certificate.subject.owner.account_id.clone(),
                lane_id: certificate.subject.owner.lane_id,
                issuance,
            };
            Self {
                issuer,
                profile,
                policy,
                certificate,
                selection,
            }
        }
    }

    fn challenge(f: &Fixture) -> KagemushaRetailEnrollmentChallengeV1 {
        KagemushaRetailEnrollmentChallengeV1 {
            version: 1,
            client_nonce: [92; 32],
            server_nonce: [93; 32],
            issuer_policy_id: f.policy.issuer_policy_id,
            issuer_audience: f.policy.issuer_audience.clone(),
            owner: f.certificate.subject.owner.clone(),
            issuance: f.selection.issuance.clone(),
            issued_at_ms: 1000,
            expires_at_ms: 2000,
        }
    }
    fn response(
        f: &Fixture,
        challenge: &KagemushaRetailEnrollmentChallengeV1,
        command: &[u8],
        reply: &KagemushaDeviceQualificationReplyV1,
    ) -> Vec<u8> {
        let body = norito::encode_canonical(reply).unwrap();
        let id = challenge.device_request_id().unwrap();
        let transcript = kagemusha_device_response_signing_bytes_v1(
            1,
            id,
            command,
            &body,
            f.selection.issuance.hardware_policy_digest,
            f.profile.qualification_report_digest,
        )
        .unwrap();
        let sig: p256::ecdsa::Signature = p256_key(3).sign(&transcript);
        let sig = sig.normalize_s().unwrap_or(sig).to_bytes();
        let mut frame = b"IKGMJRS1".to_vec();
        frame.extend_from_slice(&1u16.to_le_bytes());
        frame.extend_from_slice(&[1, 0]);
        frame.extend_from_slice(&id);
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(&64u32.to_le_bytes());
        frame.extend_from_slice(&Sha256::digest(&body));
        frame.extend_from_slice(&Sha256::digest(sig));
        frame.extend_from_slice(&body);
        frame.extend_from_slice(&sig);
        frame
    }
    fn reply(f: &Fixture) -> KagemushaDeviceQualificationReplyV1 {
        KagemushaDeviceQualificationReplyV1 {
            version: 1,
            operation: 1,
            release_id: f.selection.issuance.release_id,
            hardware_policy_digest: f.selection.issuance.hardware_policy_digest,
            core_authorization_key_reference: f.selection.issuance.core_authorization_key_reference,
            profile: f.profile.clone(),
            credential: f.selection.issuance.credential.clone(),
        }
    }
    fn proof(
        f: &Fixture,
        challenge: &KagemushaRetailEnrollmentChallengeV1,
    ) -> KagemushaRetailEnrollmentPossessionProofV1 {
        let account = KeyPair::from_seed(vec![12; 32], Algorithm::Ed25519);
        KagemushaRetailEnrollmentPossessionProofV1 {
            challenge: challenge.clone(),
            account_signature: SignatureOf::try_new(
                account.private_key(),
                &challenge.account_signing_payload().unwrap(),
            )
            .unwrap(),
            device_response: response(
                f,
                challenge,
                &KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap(),
                &reply(f),
            ),
        }
    }
    fn verify(
        f: &Fixture,
        proof: &KagemushaRetailEnrollmentPossessionProofV1,
        expected: &KagemushaRetailEnrollmentChallengeV1,
        time: u64,
    ) -> Result<KagemushaVerifiedRetailEnrollmentPossessionV1> {
        expected.validate_bound(
            &f.policy,
            f.selection.issuance.release_id,
            f.selection.issuance.hardware_policy_digest,
            &f.profile,
            time,
        )?;
        proof.authenticate_possession(expected, &f.profile, time)
    }
    #[test]
    fn external_account_signer_uses_the_exact_typed_hash_message() {
        let f = Fixture::new(1);
        let c = challenge(&f);
        let account = KeyPair::from_seed(vec![12; 32], Algorithm::Ed25519);
        let payload = c.account_signing_payload().unwrap();
        let message = c.account_signing_message().unwrap();
        let external = iroha_crypto::Signature::try_new(account.private_key(), &message).unwrap();
        let external = SignatureOf::from_signature(external);
        assert_eq!(
            external,
            SignatureOf::try_new(account.private_key(), &payload).unwrap()
        );
        external.verify(account.public_key(), &payload).unwrap();
        let mut possession = proof(&f, &c);
        possession.account_signature = external;
        verify(&f, &possession, &c, 1000).unwrap();

        let raw_payload_signature = iroha_crypto::Signature::try_new(
            account.private_key(),
            &norito::encode_canonical(&payload).unwrap(),
        )
        .unwrap();
        possession.account_signature = SignatureOf::from_signature(raw_payload_signature);
        assert_eq!(
            verify(&f, &possession, &c, 1000).unwrap_err(),
            KagemushaRetailEnrollmentChallengeErrorV1::AccountProof
        );
        let mut changed = c;
        changed.server_nonce[0] ^= 1;
        assert_ne!(message, changed.account_signing_message().unwrap());
    }

    #[test]
    fn nonce_bound_issuer_evidence_requires_all_three_signatures_and_exact_commitment() {
        let f = Fixture::new(1);
        let c = challenge(&f);
        let p = proof(&f, &c);
        let seal = |proof: &KagemushaRetailEnrollmentPossessionProofV1, time| {
            let mut certificate = f.certificate.clone();
            certificate.subject.challenge_evidence_digest =
                proof.canonical_evidence_digest().unwrap();
            certificate.subject.issued_at_ms = time;
            certificate.signature = SignatureOf::try_new(
                f.issuer.private_key(),
                &certificate.subject.approval_payload().unwrap(),
            )
            .unwrap();
            certificate
        };
        let verify_issuer = |proof: &KagemushaRetailEnrollmentPossessionProofV1,
                             certificate: &KagemushaRetailEnrollmentCertificateV1,
                             nonce| {
            proof.authenticate_issuer_evidence_bound(
                certificate,
                &f.policy,
                CatalogBinding {
                    release_id: f.selection.issuance.release_id,
                    hardware_policy_digest: f.selection.issuance.hardware_policy_digest,
                    profile: &f.profile,
                },
                &f.selection,
                nonce,
            )
        };
        let certificate = seal(&p, 1000);
        let evidence = verify_issuer(&p, &certificate, c.client_nonce).unwrap();
        assert_eq!(evidence.certificate(), &certificate);
        assert_eq!(evidence.client_nonce(), c.client_nonce);
        for nonce in [[0; 32], [91; 32], c.server_nonce] {
            assert_eq!(
                verify_issuer(&p, &certificate, nonce).unwrap_err(),
                KagemushaRetailEnrollmentChallengeErrorV1::Binding
            );
        }
        let mut wrong_certificate = certificate.clone();
        wrong_certificate.signature = SignatureOf::try_new(
            KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519).private_key(),
            &wrong_certificate.subject.approval_payload().unwrap(),
        )
        .unwrap();
        assert_eq!(
            verify_issuer(&p, &wrong_certificate, c.client_nonce).unwrap_err(),
            KagemushaRetailEnrollmentChallengeErrorV1::IssuerEvidence
        );
        assert!(
            verify_issuer(&p, &f.certificate, c.client_nonce).is_err(),
            "An unrelated signed certificate cannot authenticate this proof"
        );
        for time in [999, 2000] {
            assert_eq!(
                verify_issuer(&p, &seal(&p, time), c.client_nonce).unwrap_err(),
                KagemushaRetailEnrollmentChallengeErrorV1::Validity
            );
        }
        let mut wrong_account = p.clone();
        wrong_account.account_signature = SignatureOf::try_new(
            KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519).private_key(),
            &c.account_signing_payload().unwrap(),
        )
        .unwrap();
        assert_eq!(
            verify_issuer(&wrong_account, &seal(&wrong_account, 1000), c.client_nonce).unwrap_err(),
            KagemushaRetailEnrollmentChallengeErrorV1::AccountProof
        );
        let mut wrong_device = p.clone();
        *wrong_device.device_response.last_mut().unwrap() ^= 1;
        let signature_start = wrong_device.device_response.len() - 64;
        let hash = Sha256::digest(&wrong_device.device_response[signature_start..]);
        wrong_device.device_response[84..116].copy_from_slice(&hash);
        assert_eq!(
            verify_issuer(&wrong_device, &seal(&wrong_device, 1000), c.client_nonce).unwrap_err(),
            KagemushaRetailEnrollmentChallengeErrorV1::DeviceProof
        );
        let mut fresh_challenge = c.clone();
        fresh_challenge.client_nonce = [91; 32];
        let fresh_proof = proof(&f, &fresh_challenge);
        assert_eq!(
            verify_issuer(&fresh_proof, &certificate, fresh_challenge.client_nonce).unwrap_err(),
            KagemushaRetailEnrollmentChallengeErrorV1::IssuerEvidence
        );
    }

    #[test]
    fn client_and_server_nonces_are_distinct_and_bound_by_both_proofs() {
        let f = Fixture::new(1);
        let c = challenge(&f);
        let p = proof(&f, &c);
        for change_client in [true, false] {
            let mut changed = c.clone();
            if change_client {
                changed.client_nonce[0] ^= 1;
            } else {
                changed.server_nonce[0] ^= 1;
            }
            assert_ne!(
                changed.device_request_id().unwrap(),
                c.device_request_id().unwrap()
            );
            assert_ne!(
                changed.account_signing_message().unwrap(),
                c.account_signing_message().unwrap()
            );
            assert_eq!(
                verify(&f, &p, &changed, 1000).unwrap_err(),
                KagemushaRetailEnrollmentChallengeErrorV1::Binding
            );
            let mut substituted = p.clone();
            substituted.challenge = changed.clone();
            assert_ne!(
                substituted.canonical_evidence_digest().unwrap(),
                p.canonical_evidence_digest().unwrap()
            );
            assert_eq!(
                verify(&f, &substituted, &changed, 1000).unwrap_err(),
                KagemushaRetailEnrollmentChallengeErrorV1::AccountProof
            );
            substituted.account_signature = proof(&f, &changed).account_signature;
            assert_eq!(
                verify(&f, &substituted, &changed, 1000).unwrap_err(),
                KagemushaRetailEnrollmentChallengeErrorV1::DeviceProof
            );
        }
        for (client_nonce, server_nonce) in [
            ([0; 32], c.server_nonce),
            (c.client_nonce, [0; 32]),
            (c.client_nonce, c.client_nonce),
        ] {
            let mut invalid = c.clone();
            invalid.client_nonce = client_nonce;
            invalid.server_nonce = server_nonce;
            assert!(invalid.canonical_bytes().is_err());
        }
    }

    #[test]
    fn exact_dual_possession_proof_and_bounded_canonical_roundtrip() {
        let f = Fixture::new(1);
        let c = challenge(&f);
        let p = proof(&f, &c);
        let evidence = verify(&f, &p, &c, 1000).unwrap();
        assert_eq!(evidence.challenge(), &c);
        assert_eq!(evidence.verified_at_ms(), 1000);
        assert_ne!(evidence.evidence_digest(), [0; 32]);
        assert_eq!(
            evidence.evidence_digest(),
            p.canonical_evidence_digest().unwrap()
        );
        let mut unverified = p.clone();
        unverified.device_response[84] ^= 1;
        assert_ne!(
            unverified.canonical_evidence_digest().unwrap(),
            evidence.evidence_digest()
        );
        assert!(verify(&f, &unverified, &c, 1000).is_err());
        assert_eq!(
            KagemushaRetailEnrollmentChallengeV1::decode_canonical_exact(
                &c.canonical_bytes().unwrap()
            )
            .unwrap(),
            c
        );
        assert_eq!(
            KagemushaRetailEnrollmentPossessionProofV1::decode_canonical_exact(
                &p.canonical_bytes().unwrap()
            )
            .unwrap(),
            p
        );
        assert_eq!(
            verify(&f, &p, &c, 1999).unwrap().evidence_digest(),
            evidence.evidence_digest()
        );
    }
    #[test]
    fn nonce_account_lane_runtime_and_issuance_all_change_device_request_identity() {
        let f = Fixture::new(1);
        let c = challenge(&f);
        let id = c.device_request_id().unwrap();
        let p = proof(&f, &c);
        let mut changed = c.clone();
        changed.server_nonce[0] ^= 1;
        assert_ne!(changed.device_request_id().unwrap(), id);
        assert_eq!(
            verify(&f, &p, &changed, 1000),
            Err(KagemushaRetailEnrollmentChallengeErrorV1::Binding)
        );
        changed = c.clone();
        changed.owner.account_id = account(13);
        assert_ne!(changed.device_request_id().unwrap(), id);
        changed = c.clone();
        changed.owner.runtime.authentication_namespace = "other.bpng".parse().unwrap();
        assert_ne!(changed.device_request_id().unwrap(), id);
        changed = c.clone();
        changed.owner.lane_id = [43; 32];
        changed.issuance.credential.lane_commitment = [43; 32];
        changed.issuance.credential = changed.issuance.credential.seal_credential_id().unwrap();
        assert_ne!(changed.device_request_id().unwrap(), id);
        changed = c.clone();
        changed.issuance.core_authorization_key_reference = [88; 32];
        assert_ne!(changed.device_request_id().unwrap(), id);
        changed = c.clone();
        changed.issuer_audience = "other-audience".parse().unwrap();
        assert_ne!(changed.device_request_id().unwrap(), id);
        changed = c.clone();
        changed.expires_at_ms += 1;
        assert_ne!(changed.device_request_id().unwrap(), id);
    }
    #[test]
    fn unrelated_account_signature_and_wrong_account_domain_are_rejected() {
        let f = Fixture::new(1);
        let c = challenge(&f);
        let mut p = proof(&f, &c);
        p.account_signature = SignatureOf::try_new(
            f.issuer.private_key(),
            &c.account_signing_payload().unwrap(),
        )
        .unwrap();
        assert_eq!(
            verify(&f, &p, &c, 1000),
            Err(KagemushaRetailEnrollmentChallengeErrorV1::AccountProof)
        );
        let account = KeyPair::from_seed(vec![12; 32], Algorithm::Ed25519);
        let mut payload = c.account_signing_payload().unwrap();
        payload.domain = "iroha:kagemusha:v1:retail-enrollment-approval".into();
        p.account_signature = SignatureOf::try_new(account.private_key(), &payload).unwrap();
        assert_eq!(
            verify(&f, &p, &c, 1000),
            Err(KagemushaRetailEnrollmentChallengeErrorV1::AccountProof)
        );
    }
    #[test]
    fn old_nonce_wrong_command_and_substituted_signed_qualification_are_rejected() {
        let f = Fixture::new(1);
        let c = challenge(&f);
        let mut p = proof(&f, &c);
        let mut old = c.clone();
        old.server_nonce = [11; 32];
        p.device_response = proof(&f, &old).device_response;
        assert_eq!(
            verify(&f, &p, &c, 1000),
            Err(KagemushaRetailEnrollmentChallengeErrorV1::DeviceProof)
        );
        p.device_response = response(&f, &c, b"wrong command", &reply(&f));
        assert_eq!(
            verify(&f, &p, &c, 1000),
            Err(KagemushaRetailEnrollmentChallengeErrorV1::DeviceProof)
        );
        let mut other = reply(&f);
        other.core_authorization_key_reference = [89; 32];
        p.device_response = response(
            &f,
            &c,
            &KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap(),
            &other,
        );
        assert_eq!(
            verify(&f, &p, &c, 1000),
            Err(KagemushaRetailEnrollmentChallengeErrorV1::DeviceProof)
        );
    }
    #[test]
    fn frame_corruption_bounds_and_unknown_wire_version_are_rejected() {
        let f = Fixture::new(1);
        let c = challenge(&f);
        let p = proof(&f, &c);
        for offset in [0, 8, 10, 11, 12, 44, 48, 52, 84, 116] {
            let mut altered = p.clone();
            altered.device_response[offset] ^= 1;
            assert!(verify(&f, &altered, &c, 1000).is_err(), "offset {offset}");
        }
        for len in [0, 1, 115, p.device_response.len() - 1] {
            let mut altered = p.clone();
            altered.device_response.truncate(len);
            assert!(verify(&f, &altered, &c, 1000).is_err());
        }
        let mut bytes = p.canonical_bytes().unwrap();
        bytes.push(0);
        assert!(
            KagemushaRetailEnrollmentPossessionProofV1::decode_canonical_exact(&bytes).is_err()
        );
        assert!(
            KagemushaRetailEnrollmentPossessionProofV1::decode_canonical_exact(&vec![
                0;
                KAGEMUSHA_RETAIL_ENROLLMENT_PROOF_MAX_BYTES_V1
                    + 1
            ])
            .is_err()
        );
        let mut changed = c.clone();
        changed.version = 2;
        assert!(changed.canonical_bytes().is_err());
        changed = c.clone();
        changed.server_nonce = [0; 32];
        assert!(changed.canonical_bytes().is_err());
    }
    #[test]
    fn exact_policy_catalog_and_exclusive_validity_are_required() {
        let f = Fixture::new(1);
        let c = challenge(&f);
        let p = proof(&f, &c);
        for time in [0, 999, 2000, 2001] {
            assert!(verify(&f, &p, &c, time).is_err());
        }
        let mut policy = f.policy.clone();
        policy.issuer_audience = "other".parse().unwrap();
        assert!(
            c.validate_bound(
                &policy,
                f.selection.issuance.release_id,
                f.selection.issuance.hardware_policy_digest,
                &f.profile,
                1000
            )
            .is_err()
        );
        assert!(
            c.validate_bound(
                &f.policy,
                [99; 32],
                f.selection.issuance.hardware_policy_digest,
                &f.profile,
                1000
            )
            .is_err()
        );
        let mut changed = c.clone();
        changed.expires_at_ms =
            changed.issued_at_ms + KAGEMUSHA_RETAIL_ENROLLMENT_CHALLENGE_LIFETIME_MS_V1 + 1;
        assert!(changed.canonical_bytes().is_err());
    }

    #[test]
    fn json_unknown_fields_at_challenge_and_proof_boundaries_are_rejected() {
        let f = Fixture::new(1);
        let c = challenge(&f);
        let p = proof(&f, &c);
        for missing in ["client_nonce", "server_nonce"] {
            let mut value = norito::json::to_value(&c).unwrap();
            value.as_object_mut().unwrap().remove(missing);
            assert!(
                norito::json::from_value::<KagemushaRetailEnrollmentChallengeV1>(value).is_err()
            );
        }
        let mut old = norito::json::to_value(&c).unwrap();
        let object = old.as_object_mut().unwrap();
        object.remove("client_nonce");
        let nonce = object.remove("server_nonce").unwrap();
        object.insert("nonce".into(), nonce);
        assert!(norito::json::from_value::<KagemushaRetailEnrollmentChallengeV1>(old).is_err());
        let mut value = norito::json::to_value(&c).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("grant_native_open".into(), norito::json::Value::Bool(true));
        assert!(norito::json::from_value::<KagemushaRetailEnrollmentChallengeV1>(value).is_err());
        let mut value = norito::json::to_value(&p).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("kyc_approved".into(), norito::json::Value::Bool(true));
        assert!(
            norito::json::from_value::<KagemushaRetailEnrollmentPossessionProofV1>(value).is_err()
        );
    }
}
