//! Signed selection of one exact KAGEMUSHA monetary transition.
//!
//! Signature verification is necessary but cannot authorize money alone. A platform verifier
//! must prove that a qualified non-forking device generated the secure index, and the paired
//! recursive proof must fold that verification for every ancestor.

use super::app_attest_extensions::{
    parse_app_attest_assertion, parse_app_attest_assertion_extensions,
};
use super::{
    KAGEMUSHA_WIRE_VERSION_V1, KagemushaDeviceSignatureV1, KagemushaHardwareCredentialV1,
    KagemushaHardwarePlatformClassV1, KagemushaHardwareProfileV1, KagemushaOperationKindV1,
    KagemushaValidationErrorV1, digest_bytes, invalid, require_encoded_size,
};
use crate::kagemusha::kagemusha_app_enrollment_v1::KagemushaAppAttestationAuthorityPolicyV1;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, NetworkId};
use core::ops::Range;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use p256::ecdsa::{Signature as P256Signature, signature::Verifier as _};
use sha2::{Digest as _, Sha256};

const SIGNING_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:hardware-transition-selection\0";
const DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:signed-hardware-selection\0";
const APP_ATTEST_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:app-attest-selection\0";

/// Absolute byte ranges in the sole V1 hardware-selection signing preimage `S`.
///
/// `S = DOMAIN || u64-LE(BODY_BYTES) || BODY`. The body is a fixed-width sequence of
/// fields, without a Norito frame header, padding, checksums, or variable-length tags.
/// The model's ordinary Norito encoding remains the storage and transport format.
/// Every range below is relative to the start of `S`, so a proof can constrain each
/// signed byte directly to its independently assigned state or Guard value.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaHardwareSelectionSigningLayoutV1;

impl KagemushaHardwareSelectionSigningLayoutV1 {
    /// Exact domain bytes, including the terminal NUL.
    pub const DOMAIN_BYTES: &'static [u8] = SIGNING_DOMAIN_V1;
    /// Domain separator in `S`.
    pub const DOMAIN: Range<usize> = 0..SIGNING_DOMAIN_V1.len();
    /// Fixed body length as an unsigned 64-bit little-endian integer.
    pub const BODY_LENGTH: Range<usize> = Self::DOMAIN.end..Self::DOMAIN.end + 8;
    /// Wire version as an unsigned 16-bit little-endian integer.
    pub const VERSION: Range<usize> = Self::BODY_LENGTH.end..Self::BODY_LENGTH.end + 2;
    /// Exact authenticated proof release identifier.
    pub const RELEASE_ID: Range<usize> = Self::VERSION.end..Self::VERSION.end + 32;
    /// Receipt-authenticated provider credential Merkle root.
    pub const PROVIDER_POLICY_ROOT: Range<usize> = Self::RELEASE_ID.end..Self::RELEASE_ID.end + 32;
    /// Governance-signed app policy digest.
    pub const APP_POLICY_DIGEST: Range<usize> =
        Self::PROVIDER_POLICY_ROOT.end..Self::PROVIDER_POLICY_ROOT.end + 32;
    /// Governance-signed device credential identifier.
    pub const CREDENTIAL_ID: Range<usize> =
        Self::APP_POLICY_DIGEST.end..Self::APP_POLICY_DIGEST.end + 32;
    /// Raw, canonical 32-byte genesis-derived `NetworkId`.
    pub const NETWORK_ID: Range<usize> = Self::CREDENTIAL_ID.end..Self::CREDENTIAL_ID.end + 32;
    /// Stable device-lane commitment.
    pub const LANE_COMMITMENT: Range<usize> = Self::NETWORK_ID.end..Self::NETWORK_ID.end + 32;
    /// Governed hardware profile identifier.
    pub const HARDWARE_PROFILE_ID: Range<usize> =
        Self::LANE_COMMITMENT.end..Self::LANE_COMMITMENT.end + 32;
    /// Policy epoch as an unsigned 64-bit little-endian integer.
    pub const POLICY_EPOCH: Range<usize> =
        Self::HARDWARE_PROFILE_ID.end..Self::HARDWARE_PROFILE_ID.end + 8;
    /// Consumed hardware-key epoch identifier.
    pub const HARDWARE_EPOCH_ID: Range<usize> = Self::POLICY_EPOCH.end..Self::POLICY_EPOCH.end + 32;
    /// Hardware-key epoch generation as an unsigned 64-bit little-endian integer.
    pub const HARDWARE_EPOCH_GENERATION: Range<usize> =
        Self::HARDWARE_EPOCH_ID.end..Self::HARDWARE_EPOCH_ID.end + 8;
    /// One-byte stable operation tag: Bootstrap 0, MintFold 1, SendSplit 2,
    /// ReceiveFold 3, RedeemSplit 4, Rotate 5.
    pub const OPERATION_TAG: Range<usize> =
        Self::HARDWARE_EPOCH_GENERATION.end..Self::HARDWARE_EPOCH_GENERATION.end + 1;
    /// Digest of Core's complete exact-next transition statement.
    pub const TRANSITION_STATEMENT_DIGEST: Range<usize> =
        Self::OPERATION_TAG.end..Self::OPERATION_TAG.end + 32;
    /// Verified outgoing candidate envelope digest; zero for non-outgoing operations.
    pub const CANDIDATE_ENVELOPE_DIGEST: Range<usize> =
        Self::TRANSITION_STATEMENT_DIGEST.end..Self::TRANSITION_STATEMENT_DIGEST.end + 32;
    /// Outgoing terminal body commitment; zero for non-outgoing operations.
    pub const TERMINAL_BODY_COMMITMENT: Range<usize> =
        Self::CANDIDATE_ENVELOPE_DIGEST.end..Self::CANDIDATE_ENVELOPE_DIGEST.end + 32;
    /// Predecessor secure index as an unsigned 128-bit little-endian integer.
    pub const SECURE_INDEX_BEFORE: Range<usize> =
        Self::TERMINAL_BODY_COMMITMENT.end..Self::TERMINAL_BODY_COMMITMENT.end + 16;
    /// Exact successor secure index as an unsigned 128-bit little-endian integer.
    pub const SECURE_INDEX_AFTER: Range<usize> =
        Self::SECURE_INDEX_BEFORE.end..Self::SECURE_INDEX_BEFORE.end + 16;
    /// Complete fixed-width body in `S`.
    pub const BODY: Range<usize> = Self::VERSION.start..Self::SECURE_INDEX_AFTER.end;
    /// Exactly 403 bytes of signed subject fields.
    pub const BODY_BYTES: usize = Self::BODY.end - Self::BODY.start;
    /// Exact total size of `S`, including domain and length.
    pub const TOTAL_BYTES: usize = Self::SECURE_INDEX_AFTER.end;
}

const fn operation_tag_v1(kind: KagemushaOperationKindV1) -> u8 {
    match kind {
        KagemushaOperationKindV1::Bootstrap => 0,
        KagemushaOperationKindV1::MintFold => 1,
        KagemushaOperationKindV1::SendSplit => 2,
        KagemushaOperationKindV1::ReceiveFold => 3,
        KagemushaOperationKindV1::RedeemSplit => 4,
        KagemushaOperationKindV1::Rotate => 5,
    }
}

/// Maximum canonical bytes of one signed transition selection.
pub const KAGEMUSHA_SIGNED_HARDWARE_TRANSITION_SELECTION_MAX_BYTES_V1: usize = 1_024;
/// Maximum canonical bytes of one complete App Attest assertion selection.
pub const KAGEMUSHA_APP_ATTEST_SELECTION_MAX_BYTES_V1: usize = 9_216;

/// Exact transition subject selected by a governed hardware service.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_v1::KagemushaHardwareTransitionSelectionV1"
)]
pub struct KagemushaHardwareTransitionSelectionV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Exact authenticated proof release.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub release_id: [u8; 32],
    /// Exact receipt-authenticated provider credential Merkle root.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub provider_policy_root: [u8; 32],
    /// Governance-signed stable app/device/release binding digest.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub app_policy_digest: [u8; 32],
    /// Governance-signed device credential identity.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub credential_id: [u8; 32],
    /// Network of the monetary lane.
    pub network_id: NetworkId,
    /// Stable device-lane commitment.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub lane_commitment: [u8; 32],
    /// Governed hardware profile for this key.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub hardware_profile_id: [u8; 32],
    /// Exact policy epoch of the key.
    pub policy_epoch: u64,
    /// Consumed hardware-key epoch identity.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub hardware_epoch_id: [u8; 32],
    /// Consumed hardware-key epoch generation.
    pub hardware_epoch_generation: u64,
    /// Exact monetary operation whose transition is selected.
    pub operation_kind: KagemushaOperationKindV1,
    /// Digest of Core's complete exact-next transition statement.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub transition_statement_digest: [u8; 32],
    /// Parity-normalized verified candidate digest for an outgoing transition.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub candidate_envelope_digest: [u8; 32],
    /// Self-free terminal body committed by the device for an outgoing transition.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub terminal_body_commitment: [u8; 32],
    /// Irreversible index consumed before this transition.
    pub secure_index_before: u128,
    /// Exact successor index authenticated by the platform signature or assertion.
    /// The profile's counter or one-use-key guarantee must independently prevent forks.
    pub secure_index_after: u128,
}

impl KagemushaHardwareTransitionSelectionV1 {
    /// Reject missing scope, overflow, and skipped or reused secure indices.
    ///
    /// # Errors
    ///
    /// Returns an error for any missing identity or index other than exact-next.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        let outgoing = matches!(
            self.operation_kind,
            KagemushaOperationKindV1::SendSplit | KagemushaOperationKindV1::RedeemSplit
        );
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.operation_kind == KagemushaOperationKindV1::Bootstrap
            || self.network_id.as_bytes() == &[0; 32]
            || self.policy_epoch == 0
            || self.hardware_epoch_generation == 0
            || self.secure_index_before.checked_add(1) != Some(self.secure_index_after)
            || outgoing != (self.candidate_envelope_digest != [0; 32])
            || outgoing != (self.terminal_body_commitment != [0; 32])
            || [
                self.release_id,
                self.provider_policy_root,
                self.app_policy_digest,
                self.credential_id,
                self.lane_commitment,
                self.hardware_profile_id,
                self.hardware_epoch_id,
                self.transition_statement_digest,
            ]
            .contains(&[0; 32])
        {
            return Err(invalid("kagemusha.hardware_selection.shape"));
        }
        Ok(())
    }

    /// Return the fixed-field, domain-separated V1 signature message `S`.
    ///
    /// The signed body is deliberately independent of the Norito frame used to
    /// store and transport this model. See [`KagemushaHardwareSelectionSigningLayoutV1`]
    /// for absolute ranges that a proof must bind to independently assigned values.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid subject or unexpected fixed-field length.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        self.validate_shape()?;
        let mut message =
            Vec::with_capacity(KagemushaHardwareSelectionSigningLayoutV1::TOTAL_BYTES);
        message.extend_from_slice(SIGNING_DOMAIN_V1);
        message.extend_from_slice(
            &(KagemushaHardwareSelectionSigningLayoutV1::BODY_BYTES as u64).to_le_bytes(),
        );
        message.extend_from_slice(&self.version.to_le_bytes());
        message.extend_from_slice(&self.release_id);
        message.extend_from_slice(&self.provider_policy_root);
        message.extend_from_slice(&self.app_policy_digest);
        message.extend_from_slice(&self.credential_id);
        message.extend_from_slice(self.network_id.as_bytes());
        message.extend_from_slice(&self.lane_commitment);
        message.extend_from_slice(&self.hardware_profile_id);
        message.extend_from_slice(&self.policy_epoch.to_le_bytes());
        message.extend_from_slice(&self.hardware_epoch_id);
        message.extend_from_slice(&self.hardware_epoch_generation.to_le_bytes());
        message.push(operation_tag_v1(self.operation_kind));
        message.extend_from_slice(&self.transition_statement_digest);
        message.extend_from_slice(&self.candidate_envelope_digest);
        message.extend_from_slice(&self.terminal_body_commitment);
        message.extend_from_slice(&self.secure_index_before.to_le_bytes());
        message.extend_from_slice(&self.secure_index_after.to_le_bytes());
        if message.len() != KagemushaHardwareSelectionSigningLayoutV1::TOTAL_BYTES {
            return Err(invalid("kagemusha.hardware_selection.length"));
        }
        Ok(message)
    }
}

/// Compact device signature over one canonical selection subject.
///
/// This record establishes exact signed bytes, not physical one-use or counter semantics.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_v1::KagemushaSignedHardwareTransitionSelectionV1"
)]
pub struct KagemushaSignedHardwareTransitionSelectionV1 {
    /// Complete signed subject.
    pub subject: KagemushaHardwareTransitionSelectionV1,
    /// Fixed-width low-S P-256 signature under the enrolled device key.
    pub signature: KagemushaDeviceSignatureV1,
}

/// Original Apple App Attest assertion over the same Core selection subject.
///
/// The original CBOR assertion is parsed here; enrollment attestation must also be checked
/// by the platform verifier. This record verifies the assertion equation and strict-next count;
/// it does not establish that the counter is hardware enforced or grant monetary authority.
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
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_v1::KagemushaAppAttestHardwareTransitionSelectionV1"
)]
pub struct KagemushaAppAttestHardwareTransitionSelectionV1 {
    /// Core-owned canonical transition selected by the assertion.
    pub subject: KagemushaHardwareTransitionSelectionV1,
    /// Complete, unmodified CBOR assertion returned by App Attest.
    pub raw_assertion: Vec<u8>,
}

/// Whether the signed assertion itself measured the app release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KagemushaAppAttestReleaseMeasurementV1 {
    /// The platform returned only the RP ID, flags and counter, as observed on iOS 26.7.
    Unavailable,
    /// Signed validation-category and bundle-version extensions matched release policy.
    SignedExtensions,
}

/// Result of checking one original App Attest assertion against Core's expected selection.
///
/// This is candidate evidence, not a monetary authorization or a complete app enrollment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaVerifiedAppAttestSelectionV1 {
    /// Digest of the exact Norito selection carrying the original assertion bytes.
    pub evidence_digest: [u8; 32],
    /// Whether this particular assertion carried signed release measurements.
    pub release_measurement: KagemushaAppAttestReleaseMeasurementV1,
}

/// Trusted comparisons independently reconstructed from release, Core and secure checkpoint.
///
/// Never derive these values from the signer-supplied subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaHardwareTransitionSelectionExpectedV1 {
    /// Exact authenticated proof release.
    pub release_id: [u8; 32],
    /// Exact receipt-authenticated provider credential Merkle root.
    pub provider_policy_root: [u8; 32],
    /// Independently authenticated app/device/release binding digest.
    pub app_policy_digest: [u8; 32],
    /// Core-derived operation kind.
    pub operation_kind: KagemushaOperationKindV1,
    /// Core-derived transition statement digest.
    pub transition_statement_digest: [u8; 32],
    /// Core-derived verified candidate digest.
    pub candidate_envelope_digest: [u8; 32],
    /// Core-derived terminal body commitment.
    pub terminal_body_commitment: [u8; 32],
    /// Independently authenticated predecessor secure index.
    pub secure_index_before: u128,
}

impl KagemushaSignedHardwareTransitionSelectionV1 {
    /// Verify canonical signature, governed credential/app policy, and trusted comparisons.
    ///
    /// Additional platform counter and recursive-proof verification is mandatory for money.
    ///
    /// # Errors
    ///
    /// Returns an error for any substituted scope, index, key, signature or oversized record.
    pub fn verify_against(
        &self,
        credential: &KagemushaHardwareCredentialV1,
        profile: &KagemushaHardwareProfileV1,
        policy: &KagemushaAppAttestationAuthorityPolicyV1,
        expected: KagemushaHardwareTransitionSelectionExpectedV1,
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        validate_subject_against(&self.subject, credential, profile, expected)?;
        if profile.platform_class == KagemushaHardwarePlatformClassV1::AppleAppAttest {
            // An App Attest assertion signs SHA256(authData || SHA256(S)), not S directly.
            // A direct signature cannot substitute for the platform's original assertion.
            return Err(invalid("kagemusha.hardware_selection.app_attest_equation"));
        }
        credential
            .validate_app_policy_binding_for_release(profile, expected.release_id, policy)
            .map_err(|_| invalid("kagemusha.hardware_selection.app_policy"))?;
        self.signature.validate()?;
        self.signature.verify(
            &credential.device_public_key,
            &self.subject.canonical_signing_bytes()?,
        )?;
        require_encoded_size(
            self,
            KAGEMUSHA_SIGNED_HARDWARE_TRANSITION_SELECTION_MAX_BYTES_V1,
        )?;
        Ok(digest_bytes(
            DIGEST_DOMAIN_V1,
            &norito::encode_canonical(self)?,
        ))
    }
}

impl KagemushaAppAttestHardwareTransitionSelectionV1 {
    /// Return exact signed components from the original canonical CBOR assertion.
    ///
    /// This checks CBOR shape and size only. Callers must still verify the signature, governed
    /// credential, app policy, counter and Core-derived selection before using these bytes.
    ///
    /// # Errors
    /// Rejects malformed, noncanonical, duplicate-key or oversized assertion CBOR.
    pub fn original_assertion_components(
        &self,
    ) -> Result<(&[u8], &[u8]), KagemushaValidationErrorV1> {
        parse_app_attest_assertion(&self.raw_assertion)
            .map_err(|_| invalid("kagemusha.app_attest.assertion"))
    }

    /// Check App Attest's P-256 assertion equation and an exact-next authenticated counter.
    ///
    /// The signed ECDSA-SHA256 input is Apple's nonce
    /// `SHA256(authenticatorData || SHA256(canonical_signing_bytes))`. ECDSA-SHA256 hashes
    /// that nonce once more. Passing the concatenation directly to ECDSA would verify the
    /// wrong equation and rejects genuine iPhone assertions.
    ///
    /// For an Apple profile, `policy.app_signing_identity_digest` is SHA-256 of the exact App ID
    /// and therefore the expected RP ID hash. The policy must come from the authenticated release.
    /// `credential` must contain the key extracted from a trusted App Attest enrollment attestation.
    /// When present, signed assertion release extensions are bound to policy. Current physical
    /// iPhones also return a 37-byte assertion header without extensions; that format proves the
    /// app ID, key and counter but makes no per-assertion app-version claim. The caller remains
    /// responsible for enrollment attestation, release qualification and physical counter
    /// semantics before using this evidence in any monetary proof.
    ///
    /// # Errors
    /// Rejects a malformed assertion, wrong app/key/subject, invalid signature or skipped count.
    pub fn verify_signature_and_counter_against(
        &self,
        credential: &KagemushaHardwareCredentialV1,
        profile: &KagemushaHardwareProfileV1,
        expected: KagemushaHardwareTransitionSelectionExpectedV1,
        policy: &KagemushaAppAttestationAuthorityPolicyV1,
    ) -> Result<KagemushaVerifiedAppAttestSelectionV1, KagemushaValidationErrorV1> {
        validate_subject_against(&self.subject, credential, profile, expected)?;
        if profile.platform_class != KagemushaHardwarePlatformClassV1::AppleAppAttest {
            return Err(invalid("kagemusha.app_attest.platform"));
        }
        credential
            .validate_app_policy_binding_for_release(profile, expected.release_id, policy)
            .map_err(|_| invalid("kagemusha.app_attest.app_policy"))?;
        let (authenticator_data, signature_der) = self.original_assertion_components()?;
        let expected_rp_id = policy.app_signing_identity_digest;
        let counter = u32::try_from(self.subject.secure_index_after)
            .map_err(|_| invalid("kagemusha.app_attest.counter"))?;
        if expected_rp_id == [0; 32]
            || authenticator_data[..32] != expected_rp_id[..]
            || !matches!(authenticator_data[32], 0x40 | 0xc0)
            || (authenticator_data.len() == 37 && authenticator_data[32] != 0x40)
            || authenticator_data[33..37] != counter.to_be_bytes()[..]
        {
            return Err(invalid("kagemusha.app_attest.assertion"));
        }
        let release_measurement = if authenticator_data.len() > 37 {
            parse_app_attest_assertion_extensions(authenticator_data)
                .and_then(|extensions| extensions.verify_release_digest(policy.app_release_digest))
                .map_err(|_| invalid("kagemusha.app_attest.release_extensions"))?;
            KagemushaAppAttestReleaseMeasurementV1::SignedExtensions
        } else {
            KagemushaAppAttestReleaseMeasurementV1::Unavailable
        };
        let signature = P256Signature::from_der(signature_der)
            .map_err(|_| invalid("kagemusha.app_attest.signature"))?;
        if signature.to_der().as_bytes() != signature_der {
            return Err(invalid("kagemusha.app_attest.signature"));
        }
        let client_data_hash = Sha256::digest(self.subject.canonical_signing_bytes()?);
        let mut message = Vec::with_capacity(authenticator_data.len() + client_data_hash.len());
        message.extend_from_slice(authenticator_data);
        message.extend_from_slice(&client_data_hash);
        // App Attest signs this nonce with ECDSA-SHA256, whose verifier hashes its input again.
        let nonce = Sha256::digest(&message);
        credential
            .device_public_key
            .verifying_key()?
            .verify(&nonce, &signature)
            .map_err(|_| invalid("kagemusha.app_attest.signature"))?;
        require_encoded_size(self, KAGEMUSHA_APP_ATTEST_SELECTION_MAX_BYTES_V1)?;
        Ok(KagemushaVerifiedAppAttestSelectionV1 {
            evidence_digest: digest_bytes(
                APP_ATTEST_DIGEST_DOMAIN_V1,
                &norito::encode_canonical(self)?,
            ),
            release_measurement,
        })
    }
}

fn validate_subject_against(
    subject: &KagemushaHardwareTransitionSelectionV1,
    credential: &KagemushaHardwareCredentialV1,
    profile: &KagemushaHardwareProfileV1,
    expected: KagemushaHardwareTransitionSelectionExpectedV1,
) -> Result<(), KagemushaValidationErrorV1> {
    credential.validate_against_profile(profile)?;
    subject.validate_shape()?;
    if expected.release_id == [0; 32]
        || expected.provider_policy_root == [0; 32]
        || expected.app_policy_digest == [0; 32]
        || expected.transition_statement_digest == [0; 32]
        || subject.release_id != expected.release_id
        || subject.provider_policy_root != expected.provider_policy_root
        || subject.app_policy_digest != expected.app_policy_digest
        || subject.app_policy_digest != credential.app_policy_binding_digest
        || subject.operation_kind != expected.operation_kind
        || subject.transition_statement_digest != expected.transition_statement_digest
        || subject.candidate_envelope_digest != expected.candidate_envelope_digest
        || subject.terminal_body_commitment != expected.terminal_body_commitment
        || subject.secure_index_before != expected.secure_index_before
        || subject.credential_id != credential.credential_id
        || subject.network_id != credential.network_id
        || subject.lane_commitment != credential.lane_commitment
        || subject.hardware_profile_id != credential.hardware_profile_id
        || subject.policy_epoch != credential.policy_epoch
        || subject.hardware_epoch_id != credential.hardware_epoch_id
        || subject.hardware_epoch_generation != credential.hardware_epoch_generation
    {
        return Err(invalid("kagemusha.hardware_selection.binding"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::app_attest_extensions::app_attest_release_extensions_digest;
    use super::*;
    use crate::{
        kagemusha::{
            KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
            kagemusha_app_enrollment_v1::KagemushaAppDevicePolicyBindingV1,
            kagemusha_device_key_reference_v1, kagemusha_suite_commitment_v1,
        },
        testing::kagemusha::KagemushaFixtureSignerV1,
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

    fn app_attest_policy() -> KagemushaAppAttestationAuthorityPolicyV1 {
        let authority = KeyPair::from_seed(vec![73; 32], Algorithm::Ed25519);
        KagemushaAppAttestationAuthorityPolicyV1 {
            authority_key: authority.public_key().clone(),
            platform_class: KagemushaHardwarePlatformClassV1::AppleOemService,
            app_signing_identity_digest: [31; 32],
            app_release_digest: app_attest_release_extensions_digest(4, "1.0").unwrap(),
            maximum_lifetime_ms: 1_000,
        }
    }

    fn synthetic_release_extensions() -> Vec<u8> {
        let mut bytes = vec![0xa2, 0x72];
        bytes.extend_from_slice(b"validationCategory");
        bytes.extend_from_slice(&[0x44, 4, 0, 0, 0, 0x6d]);
        bytes.extend_from_slice(b"bundleVersion");
        bytes.extend_from_slice(&[0x63, b'1', b'.', b'0']);
        bytes
    }

    fn fixture() -> (
        KagemushaFixtureSignerV1,
        KagemushaHardwareProfileV1,
        KagemushaHardwareCredentialV1,
        KagemushaHardwareTransitionSelectionExpectedV1,
        KagemushaSignedHardwareTransitionSelectionV1,
    ) {
        let governance = KagemushaFixtureSignerV1::from_repeated_byte(17);
        let device = KagemushaFixtureSignerV1::from_repeated_byte(18);
        let app_policy = app_attest_policy();
        let suite = [20; 32];
        let profile = KagemushaHardwareProfileV1 {
            version: 1,
            protocol_version: 1,
            hardware_profile_id: [0; 32],
            provider_id: [1; 32],
            platform_class: KagemushaHardwarePlatformClassV1::AppleOemService,
            product_class_digest: [2; 32],
            firmware_policy_digest: [3; 32],
            enrollment_attestation_verifier_digest: [4; 32],
            attestation_trust_roots_digest: [5; 32],
            allowed_suite_commitment: kagemusha_suite_commitment_v1(suite),
            policy_epoch: 7,
            governance_credential_public_key: governance.device_public_key(),
            capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
            qualification_report_digest: [8; 32],
            valid_from_ms: 100,
            expires_at_ms: 10_000,
            app_attestation_authority_policy_digest: app_policy.canonical_digest().unwrap(),
        }
        .seal_hardware_profile_id()
        .unwrap();
        let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"hardware-selection-fixture",
        )));
        let mut credential = KagemushaHardwareCredentialV1 {
            version: 1,
            credential_id: [0; 32],
            network_id,
            hardware_profile_id: profile.hardware_profile_id,
            suite_id: suite,
            firmware_policy_digest: profile.firmware_policy_digest,
            policy_epoch: profile.policy_epoch,
            lane_commitment: [11; 32],
            hardware_epoch_id: [12; 32],
            hardware_epoch_generation: 1,
            device_public_key: device.device_public_key(),
            device_key_reference: kagemusha_device_key_reference_v1(&device.device_public_key()),
            issued_at_ms: 200,
            expires_at_ms: 9_000,
            app_policy_binding_digest: KagemushaAppDevicePolicyBindingV1 {
                app_signing_identity_digest: app_policy.app_signing_identity_digest,
                app_release_digest: app_policy.app_release_digest,
                release_id: [21; 32],
                hardware_profile_id: profile.hardware_profile_id,
                device_key_reference: kagemusha_device_key_reference_v1(
                    &device.device_public_key(),
                ),
                lane_id: [11; 32],
            }
            .canonical_digest()
            .unwrap(),
            governance_signature: governance.sign(b"fixture placeholder"),
        }
        .seal_credential_id()
        .unwrap();
        credential.governance_signature =
            governance.sign(&credential.canonical_signing_bytes().unwrap());
        let expected = KagemushaHardwareTransitionSelectionExpectedV1 {
            release_id: [21; 32],
            provider_policy_root: [22; 32],
            app_policy_digest: credential.app_policy_binding_digest,
            operation_kind: KagemushaOperationKindV1::SendSplit,
            transition_statement_digest: [24; 32],
            candidate_envelope_digest: [25; 32],
            terminal_body_commitment: [26; 32],
            secure_index_before: 9,
        };
        let subject = KagemushaHardwareTransitionSelectionV1 {
            version: 1,
            release_id: expected.release_id,
            provider_policy_root: expected.provider_policy_root,
            app_policy_digest: expected.app_policy_digest,
            credential_id: credential.credential_id,
            network_id,
            lane_commitment: credential.lane_commitment,
            hardware_profile_id: credential.hardware_profile_id,
            policy_epoch: credential.policy_epoch,
            hardware_epoch_id: credential.hardware_epoch_id,
            hardware_epoch_generation: credential.hardware_epoch_generation,
            operation_kind: expected.operation_kind,
            transition_statement_digest: expected.transition_statement_digest,
            candidate_envelope_digest: expected.candidate_envelope_digest,
            terminal_body_commitment: expected.terminal_body_commitment,
            secure_index_before: expected.secure_index_before,
            secure_index_after: expected.secure_index_before + 1,
        };
        let signature = device.sign(&subject.canonical_signing_bytes().unwrap());
        (
            device,
            profile,
            credential,
            expected,
            KagemushaSignedHardwareTransitionSelectionV1 { subject, signature },
        )
    }

    #[test]
    fn operation_tags_are_exact_v1() {
        assert_eq!(operation_tag_v1(KagemushaOperationKindV1::Bootstrap), 0);
        assert_eq!(operation_tag_v1(KagemushaOperationKindV1::MintFold), 1);
        assert_eq!(operation_tag_v1(KagemushaOperationKindV1::SendSplit), 2);
        assert_eq!(operation_tag_v1(KagemushaOperationKindV1::ReceiveFold), 3);
        assert_eq!(operation_tag_v1(KagemushaOperationKindV1::RedeemSplit), 4);
        assert_eq!(operation_tag_v1(KagemushaOperationKindV1::Rotate), 5);
    }

    #[test]
    fn bootstrap_has_no_signed_hardware_selection() {
        let (_, _, _, _, signed) = fixture();
        let mut subject = signed.subject;
        subject.operation_kind = KagemushaOperationKindV1::Bootstrap;
        subject.candidate_envelope_digest = [0; 32];
        subject.terminal_body_commitment = [0; 32];
        assert!(subject.validate_shape().is_err());
        assert!(subject.canonical_signing_bytes().is_err());
    }

    #[test]
    fn canonical_selection_has_exact_fixed_field_slices() {
        type Layout = KagemushaHardwareSelectionSigningLayoutV1;
        let (_, _, _, _, signed) = fixture();
        let subject = signed.subject;
        let bytes = subject.canonical_signing_bytes().unwrap();
        let ranges = [
            Layout::DOMAIN,
            Layout::BODY_LENGTH,
            Layout::VERSION,
            Layout::RELEASE_ID,
            Layout::PROVIDER_POLICY_ROOT,
            Layout::APP_POLICY_DIGEST,
            Layout::CREDENTIAL_ID,
            Layout::NETWORK_ID,
            Layout::LANE_COMMITMENT,
            Layout::HARDWARE_PROFILE_ID,
            Layout::POLICY_EPOCH,
            Layout::HARDWARE_EPOCH_ID,
            Layout::HARDWARE_EPOCH_GENERATION,
            Layout::OPERATION_TAG,
            Layout::TRANSITION_STATEMENT_DIGEST,
            Layout::CANDIDATE_ENVELOPE_DIGEST,
            Layout::TERMINAL_BODY_COMMITMENT,
            Layout::SECURE_INDEX_BEFORE,
            Layout::SECURE_INDEX_AFTER,
        ];
        assert_eq!(Layout::BODY_BYTES, 403);
        assert_eq!(Layout::TOTAL_BYTES, 460);
        assert_eq!(Layout::TOTAL_BYTES, SIGNING_DOMAIN_V1.len() + 8 + 403);
        assert_eq!(bytes.len(), Layout::TOTAL_BYTES);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges.last().unwrap().end, Layout::TOTAL_BYTES);
        for adjacent in ranges.windows(2) {
            assert_eq!(adjacent[0].end, adjacent[1].start);
        }
        assert_eq!(&bytes[Layout::DOMAIN], SIGNING_DOMAIN_V1);
        assert_eq!(&bytes[Layout::BODY_LENGTH], &(403_u64).to_le_bytes());
        assert_eq!(&bytes[Layout::VERSION], &subject.version.to_le_bytes());
        assert_eq!(&bytes[Layout::RELEASE_ID], &subject.release_id);
        assert_eq!(
            &bytes[Layout::PROVIDER_POLICY_ROOT],
            &subject.provider_policy_root
        );
        assert_eq!(
            &bytes[Layout::APP_POLICY_DIGEST],
            &subject.app_policy_digest
        );
        assert_eq!(&bytes[Layout::CREDENTIAL_ID], &subject.credential_id);
        assert_eq!(&bytes[Layout::NETWORK_ID], subject.network_id.as_bytes());
        assert_eq!(&bytes[Layout::LANE_COMMITMENT], &subject.lane_commitment);
        assert_eq!(
            &bytes[Layout::HARDWARE_PROFILE_ID],
            &subject.hardware_profile_id
        );
        assert_eq!(
            &bytes[Layout::POLICY_EPOCH],
            &subject.policy_epoch.to_le_bytes()
        );
        assert_eq!(
            &bytes[Layout::HARDWARE_EPOCH_ID],
            &subject.hardware_epoch_id
        );
        assert_eq!(
            &bytes[Layout::HARDWARE_EPOCH_GENERATION],
            &subject.hardware_epoch_generation.to_le_bytes(),
        );
        assert_eq!(&bytes[Layout::OPERATION_TAG], &[2]);
        assert_eq!(
            &bytes[Layout::TRANSITION_STATEMENT_DIGEST],
            &subject.transition_statement_digest,
        );
        assert_eq!(
            &bytes[Layout::CANDIDATE_ENVELOPE_DIGEST],
            &subject.candidate_envelope_digest
        );
        assert_eq!(
            &bytes[Layout::TERMINAL_BODY_COMMITMENT],
            &subject.terminal_body_commitment
        );
        assert_eq!(
            &bytes[Layout::SECURE_INDEX_BEFORE],
            &subject.secure_index_before.to_le_bytes()
        );
        assert_eq!(
            &bytes[Layout::SECURE_INDEX_AFTER],
            &subject.secure_index_after.to_le_bytes()
        );

        let mut changed = subject;
        changed.transition_statement_digest[0] ^= 0x80;
        let altered = changed.canonical_signing_bytes().unwrap();
        let changed_positions = bytes
            .iter()
            .zip(&altered)
            .enumerate()
            .filter_map(|(index, (left, right))| (left != right).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(
            changed_positions,
            vec![Layout::TRANSITION_STATEMENT_DIGEST.start]
        );

        changed = subject;
        changed.operation_kind = KagemushaOperationKindV1::RedeemSplit;
        let altered = changed.canonical_signing_bytes().unwrap();
        let changed_positions = bytes
            .iter()
            .zip(&altered)
            .enumerate()
            .filter_map(|(index, (left, right))| (left != right).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(changed_positions, vec![Layout::OPERATION_TAG.start]);

        changed = subject;
        changed.secure_index_before += 1;
        changed.secure_index_after += 1;
        let altered = changed.canonical_signing_bytes().unwrap();
        let changed_positions = bytes
            .iter()
            .zip(&altered)
            .enumerate()
            .filter_map(|(index, (left, right))| (left != right).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(
            changed_positions,
            vec![
                Layout::SECURE_INDEX_BEFORE.start,
                Layout::SECURE_INDEX_AFTER.start
            ]
        );
    }

    #[test]
    fn canonical_selection_roundtrips_and_verifies_governed_key() {
        let (_, profile, credential, expected, signed) = fixture();
        assert!(!signed.subject.canonical_signing_bytes().unwrap().is_empty());
        assert_ne!(
            signed
                .verify_against(&credential, &profile, &app_attest_policy(), expected)
                .unwrap(),
            [0; 32]
        );
        let decoded: KagemushaSignedHardwareTransitionSelectionV1 =
            norito::decode_canonical(&norito::encode_canonical(&signed).unwrap()).unwrap();
        assert_eq!(decoded, signed);
    }

    #[test]
    fn selection_rejects_counter_and_scope_forks() {
        let (device, profile, credential, expected, signed) = fixture();
        for after in [9, 11, u128::MAX] {
            let mut changed = signed;
            changed.subject.secure_index_after = after;
            assert!(
                changed
                    .verify_against(&credential, &profile, &app_attest_policy(), expected)
                    .is_err()
            );
        }
        let mut overflow = signed;
        overflow.subject.secure_index_before = u128::MAX;
        overflow.subject.secure_index_after = 0;
        assert!(overflow.subject.validate_shape().is_err());
        for field in 0..6 {
            let mut changed = signed;
            match field {
                0 => changed.subject.release_id = [31; 32],
                1 => changed.subject.provider_policy_root = [32; 32],
                2 => changed.subject.app_policy_digest = [33; 32],
                3 => changed.subject.transition_statement_digest = [34; 32],
                4 => changed.subject.candidate_envelope_digest = [35; 32],
                _ => changed.subject.terminal_body_commitment = [36; 32],
            }
            changed.signature = device.sign(&changed.subject.canonical_signing_bytes().unwrap());
            assert!(
                changed
                    .verify_against(&credential, &profile, &app_attest_policy(), expected)
                    .is_err()
            );
        }
        let mut changed_counter = signed;
        changed_counter.subject.secure_index_before = 8;
        changed_counter.subject.secure_index_after = 9;
        changed_counter.signature =
            device.sign(&changed_counter.subject.canonical_signing_bytes().unwrap());
        assert!(
            changed_counter
                .verify_against(&credential, &profile, &app_attest_policy(), expected)
                .is_err()
        );
        let mut wrong_credential = credential;
        wrong_credential.governance_signature = KagemushaFixtureSignerV1::from_repeated_byte(19)
            .sign(&wrong_credential.canonical_signing_bytes().unwrap());
        assert!(
            signed
                .verify_against(&wrong_credential, &profile, &app_attest_policy(), expected)
                .is_err()
        );
        let mut wrong_key = signed;
        wrong_key.signature = KagemushaFixtureSignerV1::from_repeated_byte(19)
            .sign(&wrong_key.subject.canonical_signing_bytes().unwrap());
        assert!(
            wrong_key
                .verify_against(&credential, &profile, &app_attest_policy(), expected)
                .is_err()
        );
        let mut changed_app_binding_credential = credential;
        changed_app_binding_credential.app_policy_binding_digest = [44; 32];
        changed_app_binding_credential =
            changed_app_binding_credential.seal_credential_id().unwrap();
        changed_app_binding_credential.governance_signature =
            KagemushaFixtureSignerV1::from_repeated_byte(17).sign(
                &changed_app_binding_credential
                    .canonical_signing_bytes()
                    .unwrap(),
            );
        assert!(
            changed_app_binding_credential
                .validate_against_profile(&profile)
                .is_ok()
        );
        let mut changed_app_binding_selection = signed;
        changed_app_binding_selection.subject.credential_id =
            changed_app_binding_credential.credential_id;
        changed_app_binding_selection.signature = device.sign(
            &changed_app_binding_selection
                .subject
                .canonical_signing_bytes()
                .unwrap(),
        );
        assert!(
            changed_app_binding_selection
                .verify_against(
                    &changed_app_binding_credential,
                    &profile,
                    &app_attest_policy(),
                    expected
                )
                .is_err()
        );
        let mut wrong_policy = app_attest_policy();
        wrong_policy.app_release_digest[0] ^= 1;
        assert!(
            signed
                .verify_against(&credential, &profile, &wrong_policy, expected)
                .is_err()
        );
    }

    #[test]
    fn non_outgoing_selection_requires_zero_terminal_fields() {
        let (device, profile, credential, mut expected, mut signed) = fixture();
        expected.operation_kind = KagemushaOperationKindV1::ReceiveFold;
        expected.candidate_envelope_digest = [0; 32];
        expected.terminal_body_commitment = [0; 32];
        signed.subject.operation_kind = expected.operation_kind;
        signed.subject.candidate_envelope_digest = [0; 32];
        signed.subject.terminal_body_commitment = [0; 32];
        signed.signature = device.sign(&signed.subject.canonical_signing_bytes().unwrap());
        assert!(
            signed
                .verify_against(&credential, &profile, &app_attest_policy(), expected)
                .is_ok()
        );
        signed.subject.candidate_envelope_digest = [1; 32];
        assert!(signed.subject.validate_shape().is_err());
    }

    fn cbor_byte_string(bytes: &[u8]) -> Vec<u8> {
        let mut encoded = Vec::new();
        match bytes.len() {
            0..=23 => encoded.push(0x40 | u8::try_from(bytes.len()).unwrap()),
            24..=255 => encoded.extend_from_slice(&[0x58, u8::try_from(bytes.len()).unwrap()]),
            _ => {
                encoded.push(0x59);
                encoded.extend_from_slice(&u16::try_from(bytes.len()).unwrap().to_be_bytes());
            }
        }
        encoded.extend_from_slice(bytes);
        encoded
    }

    fn assertion_bytes(authenticator_data: &[u8], signature_der: &[u8]) -> Vec<u8> {
        let mut raw = vec![0xa2, 0x71];
        raw.extend_from_slice(b"authenticatorData");
        raw.extend(cbor_byte_string(authenticator_data));
        raw.push(0x69);
        raw.extend_from_slice(b"signature");
        raw.extend(cbor_byte_string(signature_der));
        raw
    }

    fn signed_assertion(
        subject: &KagemushaHardwareTransitionSelectionV1,
        authenticator_data: &[u8],
        signer: &KagemushaFixtureSignerV1,
    ) -> Vec<u8> {
        let mut message = authenticator_data.to_vec();
        message.extend_from_slice(&Sha256::digest(subject.canonical_signing_bytes().unwrap()));
        let nonce = Sha256::digest(&message);
        let raw = signer.sign(&nonce);
        let signature_der = P256Signature::from_slice(raw.as_raw_bytes())
            .unwrap()
            .to_der();
        assertion_bytes(authenticator_data, signature_der.as_bytes())
    }

    fn app_attest_fixture() -> (
        KagemushaHardwareProfileV1,
        KagemushaHardwareCredentialV1,
        KagemushaHardwareTransitionSelectionExpectedV1,
        KagemushaAppAttestationAuthorityPolicyV1,
        KagemushaAppAttestHardwareTransitionSelectionV1,
    ) {
        let (device, mut profile, mut credential, mut expected, mut signed) = fixture();
        let governance = KagemushaFixtureSignerV1::from_repeated_byte(17);
        let mut policy = app_attest_policy();
        policy.platform_class = KagemushaHardwarePlatformClassV1::AppleAppAttest;
        profile.platform_class = KagemushaHardwarePlatformClassV1::AppleAppAttest;
        profile.capability_mask = super::super::KAGEMUSHA_APPLE_APP_ATTEST_GUARANTEES_V1;
        profile.app_attestation_authority_policy_digest = policy.canonical_digest().unwrap();
        profile = profile.seal_hardware_profile_id().unwrap();
        credential.hardware_profile_id = profile.hardware_profile_id;
        credential.app_policy_binding_digest = KagemushaAppDevicePolicyBindingV1 {
            app_signing_identity_digest: policy.app_signing_identity_digest,
            app_release_digest: policy.app_release_digest,
            release_id: expected.release_id,
            hardware_profile_id: profile.hardware_profile_id,
            device_key_reference: credential.device_key_reference,
            lane_id: credential.lane_commitment,
        }
        .canonical_digest()
        .unwrap();
        credential = credential.seal_credential_id().unwrap();
        credential.governance_signature =
            governance.sign(&credential.canonical_signing_bytes().unwrap());
        expected.app_policy_digest = credential.app_policy_binding_digest;
        signed.subject.hardware_profile_id = profile.hardware_profile_id;
        signed.subject.credential_id = credential.credential_id;
        signed.subject.app_policy_digest = credential.app_policy_binding_digest;
        let mut authenticator_data = Vec::from(policy.app_signing_identity_digest);
        authenticator_data.push(0xc0);
        authenticator_data.extend_from_slice(&10_u32.to_be_bytes());
        authenticator_data.extend_from_slice(&synthetic_release_extensions());
        let raw_assertion = signed_assertion(&signed.subject, &authenticator_data, &device);
        (
            profile,
            credential,
            expected,
            policy,
            KagemushaAppAttestHardwareTransitionSelectionV1 {
                subject: signed.subject,
                raw_assertion,
            },
        )
    }

    #[test]
    fn app_attest_assertion_binds_original_cbor_core_selection_and_strict_next_counter() {
        let (profile, credential, expected, policy, evidence) = app_attest_fixture();
        let (original_authenticator, original_signature) =
            evidence.original_assertion_components().unwrap();
        assert_eq!(
            assertion_bytes(original_authenticator, original_signature),
            evidence.raw_assertion
        );
        let mut trailing = evidence.clone();
        trailing.raw_assertion.push(0);
        assert!(trailing.original_assertion_components().is_err());
        let verify = |selection: &KagemushaAppAttestHardwareTransitionSelectionV1| {
            selection.verify_signature_and_counter_against(&credential, &profile, expected, &policy)
        };
        let verified = verify(&evidence).unwrap();
        assert_ne!(verified.evidence_digest, [0; 32]);
        assert_eq!(
            verified.release_measurement,
            KagemushaAppAttestReleaseMeasurementV1::SignedExtensions
        );
        let decoded: KagemushaAppAttestHardwareTransitionSelectionV1 =
            norito::decode_canonical(&norito::encode_canonical(&evidence).unwrap()).unwrap();
        assert_eq!(decoded, evidence);
        let (original_auth, original_der) =
            parse_app_attest_assertion(&evidence.raw_assertion).unwrap();
        let signer = KagemushaFixtureSignerV1::from_repeated_byte(18);

        let mut wrong_rp = evidence.clone();
        let mut auth = original_auth.to_vec();
        auth[0] ^= 1;
        wrong_rp.raw_assertion = signed_assertion(&wrong_rp.subject, &auth, &signer);
        assert!(verify(&wrong_rp).is_err());

        // Apple's published attestation sample appends signed extensions with ED unset.
        let mut ed_unset = evidence.clone();
        auth = original_auth.to_vec();
        auth[32] &= !0x80;
        ed_unset.raw_assertion = signed_assertion(&ed_unset.subject, &auth, &signer);
        assert!(verify(&ed_unset).is_ok());

        let mut extension_free = evidence.clone();
        auth.truncate(37);
        auth[32] = 0x40;
        extension_free.raw_assertion = signed_assertion(&extension_free.subject, &auth, &signer);
        assert_eq!(
            verify(&extension_free).unwrap().release_measurement,
            KagemushaAppAttestReleaseMeasurementV1::Unavailable
        );

        let mut wrong_flags = extension_free.clone();
        auth[32] = 0x00;
        wrong_flags.raw_assertion = signed_assertion(&wrong_flags.subject, &auth, &signer);
        assert!(verify(&wrong_flags).is_err());

        let mut malformed_suffix = extension_free.clone();
        auth[32] = 0x40;
        auth.push(0);
        malformed_suffix.raw_assertion =
            signed_assertion(&malformed_suffix.subject, &auth, &signer);
        assert!(verify(&malformed_suffix).is_err());

        let mut changed_release = evidence.clone();
        auth = original_auth.to_vec();
        *auth.last_mut().unwrap() = b'1';
        changed_release.raw_assertion = signed_assertion(&changed_release.subject, &auth, &signer);
        assert!(verify(&changed_release).is_err());

        let mut skipped = evidence.clone();
        auth = original_auth.to_vec();
        auth[36] = 11;
        skipped.raw_assertion = signed_assertion(&skipped.subject, &auth, &signer);
        assert!(verify(&skipped).is_err());

        let mut wrong_key = evidence.clone();
        let other_signer = KagemushaFixtureSignerV1::from_repeated_byte(19);
        wrong_key.raw_assertion =
            signed_assertion(&wrong_key.subject, original_auth, &other_signer);
        assert!(verify(&wrong_key).is_err());

        let mut changed_signature = evidence.clone();
        let mut bad_der = original_der.to_vec();
        *bad_der.last_mut().unwrap() ^= 1;
        changed_signature.raw_assertion = assertion_bytes(original_auth, &bad_der);
        assert!(verify(&changed_signature).is_err());

        let mut changed_subject = evidence.clone();
        changed_subject.subject.transition_statement_digest = [42; 32];
        assert!(verify(&changed_subject).is_err());

        let mut wrong_policy = policy.clone();
        wrong_policy.app_release_digest[0] ^= 1;
        assert!(
            evidence
                .verify_signature_and_counter_against(
                    &credential,
                    &profile,
                    expected,
                    &wrong_policy
                )
                .is_err()
        );

        let mut trailing = evidence.clone();
        trailing.raw_assertion.push(0);
        assert!(verify(&trailing).is_err());
        let mut duplicate = vec![0xa2, 0x71];
        duplicate.extend_from_slice(b"authenticatorData");
        duplicate.extend(cbor_byte_string(original_auth));
        duplicate.push(0x71);
        duplicate.extend_from_slice(b"authenticatorData");
        duplicate.extend(cbor_byte_string(original_auth));
        let mut duplicate_selection = evidence.clone();
        duplicate_selection.raw_assertion = duplicate;
        assert!(verify(&duplicate_selection).is_err());
    }

    #[test]
    fn app_attest_rejects_direct_selection_signature_equation() {
        let (device, profile, credential, expected, signed) = fixture();
        let policy = app_attest_policy();
        let mut authenticator_data = Vec::from(policy.app_signing_identity_digest);
        authenticator_data.push(0xc0);
        authenticator_data.extend_from_slice(&10_u32.to_be_bytes());
        authenticator_data.extend_from_slice(&synthetic_release_extensions());
        let direct = device.sign(&signed.subject.canonical_signing_bytes().unwrap());
        let signature_der = P256Signature::from_slice(direct.as_raw_bytes())
            .unwrap()
            .to_der();
        let evidence = KagemushaAppAttestHardwareTransitionSelectionV1 {
            subject: signed.subject,
            raw_assertion: assertion_bytes(&authenticator_data, signature_der.as_bytes()),
        };
        assert!(
            evidence
                .verify_signature_and_counter_against(&credential, &profile, expected, &policy)
                .is_err()
        );

        let (app_profile, app_credential, app_expected, app_policy, app_evidence) =
            app_attest_fixture();
        let signer = KagemushaFixtureSignerV1::from_repeated_byte(18);
        let direct_app_signature = KagemushaSignedHardwareTransitionSelectionV1 {
            subject: app_evidence.subject,
            signature: signer.sign(&app_evidence.subject.canonical_signing_bytes().unwrap()),
        };
        assert!(
            direct_app_signature
                .verify_against(&app_credential, &app_profile, &app_policy, app_expected)
                .is_err()
        );
    }

    #[test]
    fn direct_selection_der_conversion_normalizes_high_s_without_changing_signature() {
        const P256_ORDER: [u8; 32] = [
            0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2,
            0xfc, 0x63, 0x25, 0x51,
        ];
        let (_, profile, credential, expected, signed) = fixture();
        let low_der = P256Signature::from_slice(signed.signature.as_raw_bytes())
            .unwrap()
            .to_der();
        assert_eq!(
            KagemushaDeviceSignatureV1::from_der_normalizing_low_s(low_der.as_bytes()).unwrap(),
            signed.signature
        );

        let mut high_raw = *signed.signature.as_raw_bytes();
        let mut borrow = 0_i16;
        for index in (0..32).rev() {
            let difference =
                i16::from(P256_ORDER[index]) - i16::from(high_raw[32 + index]) - borrow;
            high_raw[32 + index] = (difference & 0xff) as u8;
            borrow = if difference < 0 { 1 } else { 0 };
        }
        assert_eq!(borrow, 0);
        let high_der = P256Signature::from_slice(&high_raw).unwrap().to_der();
        assert_eq!(
            KagemushaDeviceSignatureV1::from_der_normalizing_low_s(high_der.as_bytes()).unwrap(),
            signed.signature
        );
        let mut padded_der = high_der.as_bytes().to_vec();
        padded_der.push(0);
        assert!(KagemushaDeviceSignatureV1::from_der_normalizing_low_s(&padded_der).is_err());
        assert!(
            signed
                .verify_against(&credential, &profile, &app_attest_policy(), expected)
                .is_ok()
        );
    }
}
