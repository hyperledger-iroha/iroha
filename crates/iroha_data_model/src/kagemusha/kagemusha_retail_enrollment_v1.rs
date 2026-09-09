//! Canonical MiBank/FI ownership-certificate admission for a governed hardware lane.
//!
//! The issuer ceremony consumes this model; native monetary ownership integration remains
//! incomplete. Current-time certificate admission and nonce-bound historical issuer evidence
//! have separate opaque results. Neither is live KYC, a native wallet session, current
//! device possession, a non-forking wallet, bootstrap permission or an epoch successor.
//! Retained-certificate recovery after expiry/offline rotation requires a separate opaque
//! Core/hardware recovery authority; this module has no host-revision or historical fallback.

use super::{
    KAGEMUSHA_ASSET_SCALE_MAX_V1, KAGEMUSHA_WIRE_VERSION_V1, KagemushaAuthenticatedReleaseV1,
    KagemushaHardwareCredentialV1, KagemushaHardwareProfileV1,
};

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    name::Name,
    nexus::{AxtAssetIncarnationV1, DataSpaceId},
};
use iroha_crypto::{Algorithm, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Admission bound applied before parsing any certificate header or collection length.
pub const KAGEMUSHA_RETAIL_ENROLLMENT_MAX_BYTES_V1: usize = 16 * 1024;
/// Admission bound for independently selected issuer/runtime configuration.
pub const KAGEMUSHA_RETAIL_ENROLLMENT_POLICY_MAX_BYTES_V1: usize = 8 * 1024;
const ID_DOMAIN: &[u8] = b"iroha:kagemusha:v1:retail-enrollment-identity";
const APPROVAL_DOMAIN: &str = "iroha:kagemusha:v1:retail-enrollment-approval";

/// Independently trusted FI/authentication, ledger routing, network and asset scope.
///
/// These fields are not inferred from an authenticated hardware/proof release. Names are
/// exact canonical deployment namespaces, not user aliases or presentation labels.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentRuntimeV1",
    frame = "iroha.kagemusha.v1.retail-enrollment-runtime"
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaRetailEnrollmentRuntimeV1 {
    /// Exact financial institution identifier.
    pub fi_id: Name,
    /// Exact numeric ledger dataspace from authenticated routing, distinct from auth claims.
    pub ledger_dataspace_id: DataSpaceId,
    /// Exact Core/Vault authentication dataspace namespace; not the numerical ledger ID.
    pub authentication_namespace: Name,
    /// Genesis-bound ledger identity.
    pub network_id: NetworkId,
    /// Exact offline asset definition.
    pub asset: AssetDefinitionId,
    /// Exact signed/ledger-authenticated asset registration incarnation.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Aggregate amount scale.
    pub scale: u32,
}

/// Immutable ownership identity shared by successive certificate issuances.
///
/// Credential/key/profile, epoch, release, issuer key/policy, validity and challenge
/// revisions are deliberately absent. Computing this identity supplies no authority.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentOwnerV1",
    frame = "iroha.kagemusha.v1.retail-enrollment-owner"
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaRetailEnrollmentOwnerV1 {
    /// Canonical retail wallet account.
    pub account_id: AccountId,
    /// Immutable independently authenticated FI/auth/routing/network/asset tuple.
    pub runtime: KagemushaRetailEnrollmentRuntimeV1,
    /// Stable lane committed by the governed hardware credential.
    pub lane_id: [u8; 32],
}

/// Exact issuance bindings. They are never compared with a host-supplied numerical floor.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentIssuanceV1",
    frame = "iroha.kagemusha.v1.retail-enrollment-issuance"
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaRetailEnrollmentIssuanceV1 {
    /// Exact authenticated hardware/proof release selected for this issuance.
    pub release_id: [u8; 32],
    /// Exact hardware catalog policy digest from that release.
    pub hardware_policy_digest: [u8; 32],
    /// Exact independently pinned native Core authorization key reference.
    pub core_authorization_key_reference: [u8; 32],
    /// Complete governed credential, including key, epoch, issuance and signature.
    pub credential: KagemushaHardwareCredentialV1,
}

/// Complete issuer assertion; a shape-valid subject is not a verified enrollment.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentSubjectV1",
    frame = "iroha.kagemusha.v1.retail-enrollment-subject"
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaRetailEnrollmentSubjectV1 {
    /// Sole first-release format version.
    pub version: u16,
    /// Digest of the canonical immutable owner identity.
    pub enrollment_id: [u8; 32],
    /// Exact independently selected issuer policy identity.
    pub issuer_policy_id: [u8; 32],
    /// Exact purpose-bound enrollment service audience authorized by the independent policy.
    pub issuer_audience: Name,
    /// Stable account-to-lane ownership assertion.
    pub owner: KagemushaRetailEnrollmentOwnerV1,
    /// Exact original credential/release/Core-key pins.
    pub issuance: KagemushaRetailEnrollmentIssuanceV1,
    /// Issuer's commitment to its one-use account/device enrollment ceremony.
    /// This module checks the signed commitment, not that ceremony or live possession.
    pub challenge_evidence_digest: [u8; 32],
    /// Inclusive certificate activation/issuance time in trusted Unix milliseconds.
    pub issued_at_ms: u64,
    /// Exclusive current-admission deadline in trusted Unix milliseconds.
    pub expires_at_ms: u64,
}

/// Domain-separated typed payload signed by the authorized enrollment issuer.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentApprovalV1",
    frame = "iroha.kagemusha.v1.retail-enrollment-approval"
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaRetailEnrollmentApprovalV1 {
    /// Exact cross-protocol replay separator supplied by `approval_payload`.
    pub domain: String,
    /// Complete ownership, issuance and validity assertion.
    pub subject: KagemushaRetailEnrollmentSubjectV1,
}

/// Canonical signed certificate. Decoding alone never constructs verified evidence.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentCertificateV1",
    frame = "iroha.kagemusha.v1.retail-enrollment-certificate"
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaRetailEnrollmentCertificateV1 {
    /// Exact signed subject.
    pub subject: KagemushaRetailEnrollmentSubjectV1,
    /// Signature under the separately trusted FI enrollment issuer.
    pub signature: SignatureOf<KagemushaRetailEnrollmentApprovalV1>,
}

/// Explicit issuer/runtime trust input, selected before reading an untrusted certificate.
///
/// Deployment/native code must authenticate this configuration independently. Loading
/// these bytes from the certificate, a JWT, UI state or a host path does not establish trust.
/// The hardware profile issuer and release approvers do not implicitly delegate this role.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentIssuerPolicyV1",
    frame = "iroha.kagemusha.v1.retail-enrollment-issuer-policy"
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaRetailEnrollmentIssuerPolicyV1 {
    /// Sole first-release policy format.
    pub version: u16,
    /// Exact purpose-bound policy identity selected by native deployment configuration.
    pub issuer_policy_id: [u8; 32],
    /// Explicitly delegated Ed25519 enrollment issuer. No algorithm negotiation.
    pub issuer_public_key: PublicKey,
    /// Explicit enrollment-service audience, not trust inherited from a JWT audience.
    pub issuer_audience: Name,
    /// Exact authoritative runtime tuple that this issuer may bind.
    pub runtime: KagemushaRetailEnrollmentRuntimeV1,
    /// Inclusive authorization start for certificate issuance/admission.
    pub valid_from_ms: u64,
    /// Exclusive issuer authorization deadline for current admission.
    pub expires_at_ms: u64,
    /// Maximum certificate lifetime allowed by the independently trusted policy.
    pub maximum_certificate_lifetime_ms: u64,
}

/// Exact selection pins supplied by the native enrollment/owner ceremony.
///
/// They correlate a certificate with the requested owner; they provide no issuer,
/// current-device or successor authority. Every credential field must match exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaRetailEnrollmentSelectionV1 {
    /// Exact stable enrollment selected by the native ceremony/retained owner.
    pub enrollment_id: [u8; 32],
    /// Expected canonical retail account.
    pub account_id: AccountId,
    /// Expected stable hardware lane.
    pub lane_id: [u8; 32],
    /// Exact issuance evidence; a greater host generation is not accepted.
    pub issuance: KagemushaRetailEnrollmentIssuanceV1,
}

/// Issuer- and catalog-verified certificate evidence, valid at one supplied trusted time.
///
/// No public constructor, decoder or deserializer can create this value. This is not
/// a wallet lease, live KYC decision, bootstrap grant or hardware successor capability.
#[derive(Debug, PartialEq, Eq)]
pub struct KagemushaVerifiedRetailEnrollmentCertificateV1 {
    certificate: KagemushaRetailEnrollmentCertificateV1,
    authenticated_at_ms: u64,
}

impl KagemushaVerifiedRetailEnrollmentCertificateV1 {
    /// Borrow immutable verified certificate evidence.
    #[must_use]
    pub fn certificate(&self) -> &KagemushaRetailEnrollmentCertificateV1 {
        &self.certificate
    }

    /// Trusted instant at which all current-admission intervals were checked.
    #[must_use]
    pub fn authenticated_at_ms(&self) -> u64 {
        self.authenticated_at_ms
    }
}

/// Closed certificate-admission failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KagemushaRetailEnrollmentErrorV1 {
    /// Empty, oversized, malformed, noncanonical or unknown-field encoding.
    Encoding,
    /// Invalid immutable scope, identity, credential shape or reserved digest.
    InvalidSubject,
    /// Invalid or unsupported independent issuer/runtime trust configuration.
    InvalidPolicy,
    /// Account, FI/service/runtime, lane or exact issuance pins differ.
    SelectionMismatch,
    /// Issuance release/policy/profile is not the selected authenticated catalog.
    CatalogMismatch,
    /// Governed credential is not valid under the authenticated enabled profile.
    InvalidCredential,
    /// Trusted time, nesting or maximum lifetime violates admission policy.
    InvalidValidity,
    /// Issuer signature does not authenticate the exact domain-separated subject.
    InvalidSignature,
}

impl core::fmt::Display for KagemushaRetailEnrollmentErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid retail enrollment: {self:?}")
    }
}

impl std::error::Error for KagemushaRetailEnrollmentErrorV1 {}

type Result<T> = core::result::Result<T, KagemushaRetailEnrollmentErrorV1>;

fn nonzero(value: &[u8; 32]) -> bool {
    value.iter().any(|byte| *byte != 0)
}

fn validate_runtime(runtime: &KagemushaRetailEnrollmentRuntimeV1) -> Result<()> {
    if runtime.scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
        || runtime.asset_incarnation.validate().is_err()
        || !nonzero(runtime.network_id.as_bytes())
    {
        return Err(KagemushaRetailEnrollmentErrorV1::InvalidSubject);
    }
    // Name already enforces canonical syntax and a 255-byte bound, including on decode.
    Ok(())
}

fn encode_bounded<T: norito::NoritoSerialize>(value: &T, maximum: usize) -> Result<Vec<u8>> {
    let bytes =
        norito::encode_canonical(value).map_err(|_| KagemushaRetailEnrollmentErrorV1::Encoding)?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(KagemushaRetailEnrollmentErrorV1::Encoding);
    }
    Ok(bytes)
}

fn decode_bounded<T>(bytes: &[u8], maximum: usize) -> Result<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(KagemushaRetailEnrollmentErrorV1::Encoding);
    }
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
        .map_err(|_| KagemushaRetailEnrollmentErrorV1::Encoding)
}

impl KagemushaRetailEnrollmentOwnerV1 {
    /// Compute a stable, domain-separated selector; the resulting digest is not authority.
    ///
    /// # Errors
    /// Rejects invalid immutable scope/lane or oversized canonical identity bytes.
    pub fn enrollment_id(&self) -> Result<[u8; 32]> {
        validate_runtime(&self.runtime)?;
        if !nonzero(&self.lane_id) {
            return Err(KagemushaRetailEnrollmentErrorV1::InvalidSubject);
        }
        let bytes = encode_bounded(self, KAGEMUSHA_RETAIL_ENROLLMENT_MAX_BYTES_V1)?;
        let mut hash = Sha256::new();
        hash.update(ID_DOMAIN);
        hash.update([0]);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
        Ok(hash.finalize().into())
    }
}

impl KagemushaRetailEnrollmentSubjectV1 {
    fn validate_shape(&self) -> Result<()> {
        let credential = &self.issuance.credential;
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.enrollment_id != self.owner.enrollment_id()?
            || !nonzero(&self.issuer_policy_id)
            || !nonzero(&self.issuance.release_id)
            || !nonzero(&self.issuance.hardware_policy_digest)
            || !nonzero(&self.issuance.core_authorization_key_reference)
            || !nonzero(&self.challenge_evidence_digest)
            || credential.network_id != self.owner.runtime.network_id
            || credential.lane_commitment != self.owner.lane_id
            || credential.validate_shape().is_err()
        {
            return Err(KagemushaRetailEnrollmentErrorV1::InvalidSubject);
        }
        if self.issued_at_ms == 0 || self.expires_at_ms <= self.issued_at_ms {
            return Err(KagemushaRetailEnrollmentErrorV1::InvalidValidity);
        }
        Ok(())
    }

    /// Construct the only typed signing payload supported by this first release.
    ///
    /// # Errors
    /// Rejects a malformed or oversized subject. This does not authorize its contents.
    pub fn approval_payload(&self) -> Result<KagemushaRetailEnrollmentApprovalV1> {
        self.validate_shape()?;
        let payload = KagemushaRetailEnrollmentApprovalV1 {
            domain: APPROVAL_DOMAIN.to_owned(),
            subject: self.clone(),
        };
        encode_bounded(&payload, KAGEMUSHA_RETAIL_ENROLLMENT_MAX_BYTES_V1)?;
        Ok(payload)
    }
}

impl KagemushaRetailEnrollmentIssuerPolicyV1 {
    /// Check configuration shape. The caller must separately establish its authority.
    ///
    /// # Errors
    /// Rejects unsupported versions/algorithms, reserved IDs and invalid scope/time bounds.
    pub fn validate(&self) -> Result<()> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || !nonzero(&self.issuer_policy_id)
            || self.issuer_public_key.algorithm() != Algorithm::Ed25519
            || validate_runtime(&self.runtime).is_err()
            || self.valid_from_ms == 0
            || self.expires_at_ms <= self.valid_from_ms
            || self.maximum_certificate_lifetime_ms == 0
            || self.maximum_certificate_lifetime_ms > self.expires_at_ms - self.valid_from_ms
        {
            return Err(KagemushaRetailEnrollmentErrorV1::InvalidPolicy);
        }
        encode_bounded(self, KAGEMUSHA_RETAIL_ENROLLMENT_POLICY_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Decode bounded canonical configuration without granting that configuration trust.
    ///
    /// # Errors
    /// Rejects malformed/noncanonical/oversized bytes and invalid policy shape.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self> {
        let policy: Self = decode_bounded(bytes, KAGEMUSHA_RETAIL_ENROLLMENT_POLICY_MAX_BYTES_V1)?;
        policy.validate()?;
        Ok(policy)
    }
}

#[derive(Clone, Copy)]
pub(super) struct CatalogBinding<'a> {
    pub(super) release_id: [u8; 32],
    pub(super) hardware_policy_digest: [u8; 32],
    pub(super) profile: &'a KagemushaHardwareProfileV1,
}

impl KagemushaRetailEnrollmentCertificateV1 {
    /// Decode a bounded canonical certificate without authenticating it.
    ///
    /// # Errors
    /// Rejects malformed/noncanonical/oversized bytes and invalid subject shape.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self> {
        let certificate: Self = decode_bounded(bytes, KAGEMUSHA_RETAIL_ENROLLMENT_MAX_BYTES_V1)?;
        certificate.subject.validate_shape()?;
        Ok(certificate)
    }

    /// Encode the sole current certificate format without claiming issuer approval.
    ///
    /// # Errors
    /// Rejects invalid subjects or oversized canonical output.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.subject.validate_shape()?;
        encode_bounded(self, KAGEMUSHA_RETAIL_ENROLLMENT_MAX_BYTES_V1)
    }

    /// Authenticate current certificate evidence against independent trust and exact pins.
    ///
    /// The release must be a real opaque threshold-authenticated catalog. `policy` must be
    /// independently trusted native configuration, and `trusted_time_ms` must come from the
    /// authoritative service/hardware clock. Host time, host revisions and persisted Booleans
    /// are not substitutes. This verifies no device-possession challenge or epoch transition.
    ///
    /// # Errors
    /// Rejects any encoding, identity, scope, issuer, catalog, credential or validity mismatch.
    pub fn authenticate(
        &self,
        policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
        release: &KagemushaAuthenticatedReleaseV1,
        expected: &KagemushaRetailEnrollmentSelectionV1,
        trusted_time_ms: u64,
    ) -> Result<KagemushaVerifiedRetailEnrollmentCertificateV1> {
        let profile = release
            .enabled_profile(self.subject.issuance.credential.hardware_profile_id)
            .ok_or(KagemushaRetailEnrollmentErrorV1::CatalogMismatch)?;
        self.authenticate_bound(
            policy,
            CatalogBinding {
                release_id: release.release_id(),
                hardware_policy_digest: release.hardware_policy_digest(),
                profile: &profile.hardware_profile,
            },
            expected,
            trusted_time_ms,
        )
    }

    // Private kernel allows focused signature/binding tests without exposing a public way to
    // fabricate an authenticated release or obtain evidence under a host-selected profile.
    fn authenticate_bound(
        &self,
        policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
        catalog: CatalogBinding<'_>,
        expected: &KagemushaRetailEnrollmentSelectionV1,
        trusted_time_ms: u64,
    ) -> Result<KagemushaVerifiedRetailEnrollmentCertificateV1> {
        self.verify_issuer_bound(policy, catalog, expected)?;
        if trusted_time_ms == 0
            || trusted_time_ms < self.subject.issued_at_ms
            || trusted_time_ms >= self.subject.expires_at_ms
        {
            return Err(KagemushaRetailEnrollmentErrorV1::InvalidValidity);
        }
        Ok(KagemushaVerifiedRetailEnrollmentCertificateV1 {
            certificate: self.clone(),
            authenticated_at_ms: trusted_time_ms,
        })
    }

    // Shared signature/scope/interval kernel. It intentionally creates no current-time
    // certificate capability: nonce-bound issuer evidence has a separate opaque result.
    pub(super) fn verify_issuer_bound(
        &self,
        policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
        catalog: CatalogBinding<'_>,
        expected: &KagemushaRetailEnrollmentSelectionV1,
    ) -> Result<()> {
        self.canonical_bytes()?;
        policy.validate()?;
        let subject = &self.subject;
        if subject.issuer_policy_id != policy.issuer_policy_id
            || subject.issuer_audience != policy.issuer_audience
            || subject.owner.runtime != policy.runtime
            || subject.enrollment_id != expected.enrollment_id
            || subject.owner.account_id != expected.account_id
            || subject.owner.lane_id != expected.lane_id
            || subject.issuance != expected.issuance
        {
            return Err(KagemushaRetailEnrollmentErrorV1::SelectionMismatch);
        }
        if subject.issuance.release_id != catalog.release_id
            || subject.issuance.hardware_policy_digest != catalog.hardware_policy_digest
            || subject.issuance.credential.hardware_profile_id
                != catalog.profile.hardware_profile_id
        {
            return Err(KagemushaRetailEnrollmentErrorV1::CatalogMismatch);
        }
        let credential = &subject.issuance.credential;
        credential
            .validate_against_profile(catalog.profile)
            .map_err(|_| KagemushaRetailEnrollmentErrorV1::InvalidCredential)?;
        if subject.issued_at_ms < policy.valid_from_ms
            || subject.expires_at_ms > policy.expires_at_ms
            || subject.expires_at_ms - subject.issued_at_ms > policy.maximum_certificate_lifetime_ms
            || subject.issued_at_ms < credential.issued_at_ms
            || subject.expires_at_ms > credential.expires_at_ms
        {
            return Err(KagemushaRetailEnrollmentErrorV1::InvalidValidity);
        }
        self.signature
            .verify(&policy.issuer_public_key, &subject.approval_payload()?)
            .map_err(|_| KagemushaRetailEnrollmentErrorV1::InvalidSignature)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Cryptographic verifier-kernel tests with explicit trusted-catalog fixtures.
    //! These do not construct or claim to test a threshold-authenticated release, an OEM
    //! enrollment service, a physical device, live KYC or the public release lookup wrapper.

    use super::*;
    use crate::kagemusha::{
        KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KagemushaDevicePublicKeyV1,
        KagemushaDeviceSignatureV1, KagemushaHardwarePlatformClassV1,
        kagemusha_device_key_reference_v1, kagemusha_suite_commitment_v1,
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

        fn catalog(&self) -> CatalogBinding<'_> {
            CatalogBinding {
                release_id: self.selection.issuance.release_id,
                hardware_policy_digest: self.selection.issuance.hardware_policy_digest,
                profile: &self.profile,
            }
        }

        fn verify(&self, time: u64) -> Result<KagemushaVerifiedRetailEnrollmentCertificateV1> {
            self.certificate
                .authenticate_bound(&self.policy, self.catalog(), &self.selection, time)
        }

        fn resign(&mut self) {
            self.certificate.signature = SignatureOf::try_new(
                self.issuer.private_key(),
                &self
                    .certificate
                    .subject
                    .approval_payload()
                    .expect("test subject"),
            )
            .expect("test issuer signature");
        }
    }

    #[test]
    fn verifies_exact_issuer_credential_catalog_and_selection() {
        let fixture = Fixture::new(1);
        let evidence = fixture.verify(1_500).expect("valid certificate kernel");
        assert_eq!(evidence.certificate(), &fixture.certificate);
        assert_eq!(evidence.authenticated_at_ms(), 1_500);
    }

    #[test]
    fn canonical_certificate_and_policy_round_trip_with_predecode_bounds() {
        let fixture = Fixture::new(1);
        let bytes = fixture
            .certificate
            .canonical_bytes()
            .expect("canonical certificate");
        assert_eq!(
            KagemushaRetailEnrollmentCertificateV1::decode_canonical_exact(&bytes).unwrap(),
            fixture.certificate
        );
        let policy = norito::encode_canonical(&fixture.policy).unwrap();
        assert_eq!(
            KagemushaRetailEnrollmentIssuerPolicyV1::decode_canonical_exact(&policy).unwrap(),
            fixture.policy
        );
        for length in [0, 1, bytes.len() / 2, bytes.len() - 1] {
            assert!(
                KagemushaRetailEnrollmentCertificateV1::decode_canonical_exact(&bytes[..length])
                    .is_err()
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(KagemushaRetailEnrollmentCertificateV1::decode_canonical_exact(&trailing).is_err());
        assert!(
            KagemushaRetailEnrollmentCertificateV1::decode_canonical_exact(&vec![
                0;
                KAGEMUSHA_RETAIL_ENROLLMENT_MAX_BYTES_V1
                    + 1
            ])
            .is_err()
        );
        assert!(
            KagemushaRetailEnrollmentIssuerPolicyV1::decode_canonical_exact(&vec![
                0;
                KAGEMUSHA_RETAIL_ENROLLMENT_POLICY_MAX_BYTES_V1
                    + 1
            ])
            .is_err()
        );
        let mut wrong_version = fixture.certificate.clone();
        wrong_version.subject.version = 2;
        assert!(
            KagemushaRetailEnrollmentCertificateV1::decode_canonical_exact(
                &norito::encode_canonical(&wrong_version).unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_other_issuer_changed_subject_and_other_signature_domain() {
        let mut fixture = Fixture::new(1);
        fixture.policy.issuer_public_key = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519)
            .public_key()
            .clone();
        assert_eq!(
            fixture.verify(1_500),
            Err(KagemushaRetailEnrollmentErrorV1::InvalidSignature)
        );
        let mut fixture = Fixture::new(1);
        fixture.certificate.subject.challenge_evidence_digest = [99; 32];
        assert_eq!(
            fixture.verify(1_500),
            Err(KagemushaRetailEnrollmentErrorV1::InvalidSignature)
        );
        let mut fixture = Fixture::new(1);
        let mut payload = fixture.certificate.subject.approval_payload().unwrap();
        payload.domain.push_str(":another-purpose");
        fixture.certificate.signature =
            SignatureOf::try_new(fixture.issuer.private_key(), &payload).unwrap();
        assert_eq!(
            fixture.verify(1_500),
            Err(KagemushaRetailEnrollmentErrorV1::InvalidSignature)
        );
    }

    #[test]
    fn rejects_every_independent_runtime_scope_substitution() {
        let original = Fixture::new(1);
        let mut alternatives = Vec::new();
        macro_rules! different {
            ($field:ident, $value:expr) => {{
                let mut value = original.policy.runtime.clone();
                value.$field = $value;
                alternatives.push(value);
            }};
        }
        different!(fi_id, "other-bank".parse().unwrap());
        different!(ledger_dataspace_id, DataSpaceId::new(11));
        different!(authentication_namespace, "other-auth".parse().unwrap());
        different!(
            network_id,
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"other-network"
            )))
        );
        different!(
            asset,
            AssetDefinitionId::from_uuid_bytes([
                0x4f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
                0xcd, 0x2f
            ])
            .unwrap()
        );
        different!(
            asset_incarnation,
            AxtAssetIncarnationV1::try_from_bytes(*Hash::new(b"other-incarnation").as_ref())
                .unwrap()
        );
        different!(scale, 3);
        for runtime in alternatives {
            let mut fixture = Fixture::new(1);
            fixture.policy.runtime = runtime;
            assert_eq!(
                fixture.verify(1_500),
                Err(KagemushaRetailEnrollmentErrorV1::SelectionMismatch)
            );
        }
    }

    #[test]
    fn rejects_other_enrollment_service_audience_without_changing_stable_identity() {
        let mut fixture = Fixture::new(1);
        let stable_id = fixture.certificate.subject.enrollment_id;
        fixture.policy.issuer_audience = "other-enrollment-service".parse().unwrap();
        assert_eq!(
            fixture.verify(1_500),
            Err(KagemushaRetailEnrollmentErrorV1::SelectionMismatch)
        );
        fixture.certificate.subject.issuer_audience = fixture.policy.issuer_audience.clone();
        fixture.resign();
        assert!(fixture.verify(1_500).is_ok());
        assert_eq!(fixture.certificate.subject.enrollment_id, stable_id);
    }

    #[test]
    fn rejects_account_lane_identity_and_every_exact_issuance_pin_change() {
        for change in 0..9 {
            let mut fixture = Fixture::new(1);
            match change {
                0 => fixture.selection.account_id = account(22),
                1 => fixture.selection.lane_id = [88; 32],
                2 => fixture.selection.enrollment_id = [88; 32],
                3 => fixture.selection.issuance.release_id = [88; 32],
                4 => fixture.selection.issuance.hardware_policy_digest = [88; 32],
                5 => fixture.selection.issuance.core_authorization_key_reference = [88; 32],
                6 => {
                    fixture
                        .selection
                        .issuance
                        .credential
                        .hardware_epoch_generation += 1
                }
                7 => fixture.selection.issuance.credential.device_public_key = public(&p256_key(8)),
                8 => fixture.selection.issuance.credential.hardware_epoch_id = [88; 32],
                _ => unreachable!(),
            }
            assert_eq!(
                fixture.verify(1_500),
                Err(KagemushaRetailEnrollmentErrorV1::SelectionMismatch)
            );
        }
    }

    #[test]
    fn rejects_catalog_substitution_and_invalid_governed_signature() {
        let fixture = Fixture::new(1);
        for change in 0..3 {
            let mut profile = fixture.profile;
            let mut catalog = fixture.catalog();
            match change {
                0 => catalog.release_id = [88; 32],
                1 => catalog.hardware_policy_digest = [88; 32],
                2 => {
                    profile.hardware_profile_id = [88; 32];
                    catalog.profile = &profile;
                }
                _ => unreachable!(),
            }
            assert_eq!(
                fixture.certificate.authenticate_bound(
                    &fixture.policy,
                    catalog,
                    &fixture.selection,
                    1_500
                ),
                Err(KagemushaRetailEnrollmentErrorV1::CatalogMismatch)
            );
        }
        let mut fixture = Fixture::new(1);
        fixture
            .certificate
            .subject
            .issuance
            .credential
            .governance_signature = KagemushaDeviceSignatureV1::from_raw_bytes(&[2; 64]).unwrap();
        fixture.selection.issuance = fixture.certificate.subject.issuance.clone();
        fixture.resign();
        assert_eq!(
            fixture.verify(1_500),
            Err(KagemushaRetailEnrollmentErrorV1::InvalidCredential)
        );
    }

    #[test]
    fn validity_is_inclusive_at_start_exclusive_at_expiry_and_nested() {
        let fixture = Fixture::new(1);
        assert!(fixture.verify(1_000).is_ok());
        assert!(fixture.verify(2_999).is_ok());
        for time in [0, 999, 3_000, u64::MAX] {
            assert_eq!(
                fixture.verify(time),
                Err(KagemushaRetailEnrollmentErrorV1::InvalidValidity)
            );
        }
        for change in 0..5 {
            let mut fixture = Fixture::new(1);
            match change {
                0 => fixture.policy.maximum_certificate_lifetime_ms = 1_999,
                1 => fixture.policy.valid_from_ms = 1_001,
                2 => {
                    fixture.policy.expires_at_ms = 2_999;
                    fixture.policy.maximum_certificate_lifetime_ms = 2_500;
                }
                3 => {
                    fixture.certificate.subject.issued_at_ms = 199;
                    fixture.resign();
                }
                4 => {
                    fixture.policy.expires_at_ms = 10_000;
                    fixture.policy.maximum_certificate_lifetime_ms = 9_000;
                    fixture.certificate.subject.expires_at_ms = 9_001;
                    fixture.resign();
                }
                _ => unreachable!(),
            }
            assert!(fixture.verify(1_500).is_err());
        }
    }

    #[test]
    fn identity_survives_credential_release_and_policy_reissuance_without_admitting_successor() {
        let first = Fixture::new(1);
        let mut second = Fixture::new(2);
        second.policy.issuer_policy_id = [91; 32];
        second.certificate.subject.issuer_policy_id = second.policy.issuer_policy_id;
        second.certificate.subject.issued_at_ms += 1;
        second.certificate.subject.expires_at_ms += 1;
        second.certificate.subject.challenge_evidence_digest = [92; 32];
        second.resign();
        assert_eq!(
            first.certificate.subject.enrollment_id,
            second.certificate.subject.enrollment_id
        );
        assert_ne!(
            first.certificate.subject.issuance,
            second.certificate.subject.issuance
        );
        assert!(first.verify(1_500).is_ok());
        assert!(second.verify(1_500).is_ok());
        // Two independent valid issuer certificates are not proof of an allowed hardware
        // successor. Supplying the other issuance pins still fails exact admission.
        assert_eq!(
            second.certificate.authenticate_bound(
                &second.policy,
                second.catalog(),
                &first.selection,
                1_500
            ),
            Err(KagemushaRetailEnrollmentErrorV1::SelectionMismatch)
        );
    }

    #[test]
    fn invalid_policy_subject_and_reserved_values_fail_closed() {
        for change in 0..6 {
            let mut fixture = Fixture::new(1);
            match change {
                0 => fixture.policy.version = 2,
                1 => fixture.policy.issuer_policy_id = [0; 32],
                2 => fixture.policy.valid_from_ms = 0,
                3 => fixture.policy.maximum_certificate_lifetime_ms = 0,
                4 => fixture.policy.runtime.scale = KAGEMUSHA_ASSET_SCALE_MAX_V1 + 1,
                5 => {
                    fixture.policy.issuer_public_key =
                        KeyPair::from_seed(vec![8; 32], Algorithm::Secp256k1)
                            .public_key()
                            .clone()
                }
                _ => unreachable!(),
            }
            assert_eq!(
                fixture.policy.validate(),
                Err(KagemushaRetailEnrollmentErrorV1::InvalidPolicy)
            );
        }
        for change in 0..5 {
            let mut fixture = Fixture::new(1);
            match change {
                0 => fixture.certificate.subject.enrollment_id = [0; 32],
                1 => fixture.certificate.subject.owner.lane_id = [0; 32],
                2 => fixture.certificate.subject.challenge_evidence_digest = [0; 32],
                3 => {
                    fixture
                        .certificate
                        .subject
                        .issuance
                        .core_authorization_key_reference = [0; 32]
                }
                4 => fixture.certificate.subject.issued_at_ms = 0,
                _ => unreachable!(),
            }
            assert!(fixture.certificate.canonical_bytes().is_err());
        }
    }

    #[test]
    fn json_rejects_unknown_fields_at_every_signed_and_policy_boundary() {
        let fixture = Fixture::new(1);
        macro_rules! reject_unknown {
            ($value:expr, $type:ty) => {{
                let json = norito::json::to_json(&$value).unwrap();
                let changed = json.replacen('{', "{\"unexpected\":true,", 1);
                assert!(norito::json::from_str::<$type>(&changed).is_err());
            }};
        }
        reject_unknown!(fixture.certificate, KagemushaRetailEnrollmentCertificateV1);
        reject_unknown!(
            fixture.certificate.subject,
            KagemushaRetailEnrollmentSubjectV1
        );
        reject_unknown!(
            fixture.certificate.subject.owner,
            KagemushaRetailEnrollmentOwnerV1
        );
        reject_unknown!(
            fixture.certificate.subject.issuance,
            KagemushaRetailEnrollmentIssuanceV1
        );
        reject_unknown!(fixture.policy, KagemushaRetailEnrollmentIssuerPolicyV1);
        reject_unknown!(fixture.policy.runtime, KagemushaRetailEnrollmentRuntimeV1);
        reject_unknown!(
            fixture.certificate.subject.approval_payload().unwrap(),
            KagemushaRetailEnrollmentApprovalV1
        );
    }
}

#[cfg(test)]
mod captured_cutover_identity_tests {
    fn check<T>(nominal: &str, frame: &str, hash: &str)
    where
        T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
    {
        assert_eq!(T::nominal_name(), nominal);
        assert_eq!(T::frame_name(), frame);
        assert_eq!(
            hex::encode(norito::schema::identity::frame_hash::<T>()),
            hash
        );
    }

    #[test]
    fn captured_owner_identities() {
        check::<super::KagemushaRetailEnrollmentRuntimeV1>(
            "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentRuntimeV1",
            "iroha.kagemusha.v1.retail-enrollment-runtime",
            "67f3af521a2b2c0e2ad110e6f0a528a0",
        );
        check::<super::KagemushaRetailEnrollmentOwnerV1>(
            "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentOwnerV1",
            "iroha.kagemusha.v1.retail-enrollment-owner",
            "537843eb69ef723fb31655ef7af9b7a8",
        );
        check::<super::KagemushaRetailEnrollmentIssuanceV1>(
            "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentIssuanceV1",
            "iroha.kagemusha.v1.retail-enrollment-issuance",
            "bcc1a56544552e1ec3b9b4add6565ee0",
        );
        check::<super::KagemushaRetailEnrollmentSubjectV1>(
            "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentSubjectV1",
            "iroha.kagemusha.v1.retail-enrollment-subject",
            "74aed55fe9158b5bbb9a26dace1696ef",
        );
        check::<super::KagemushaRetailEnrollmentApprovalV1>(
            "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentApprovalV1",
            "iroha.kagemusha.v1.retail-enrollment-approval",
            "064206d6778777d489ed030ff7239fe2",
        );
        check::<super::KagemushaRetailEnrollmentCertificateV1>(
            "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentCertificateV1",
            "iroha.kagemusha.v1.retail-enrollment-certificate",
            "7205a003f9151ddd6b3701853f0188d7",
        );
        check::<super::KagemushaRetailEnrollmentIssuerPolicyV1>(
            "iroha_data_model::kagemusha::kagemusha_retail_enrollment_v1::KagemushaRetailEnrollmentIssuerPolicyV1",
            "iroha.kagemusha.v1.retail-enrollment-issuer-policy",
            "7934f8746eb0821b84dcee4bfba3e96c",
        );
    }
}
