//! Issuer-independent, signed app-to-device enrollment evidence for KAGEMUSHA V1.
//!
//! The authority named by the native deployment policy must first verify the platform's raw
//! app identity attestation and independently trustworthy build/release provenance. This assertion
//! carries the result into the one-use account/device enrollment ceremony. It is not a device
//! counter, an offline spend authorization, or proof that an app can access a Secure Element.

use super::{
    KagemushaHardwareCredentialV1, KagemushaHardwarePlatformClassV1, KagemushaHardwareProfileV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{Algorithm, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

const APP_ASSERTION_DOMAIN: &str = "iroha:kagemusha:v1:app-device-enrollment";
const APP_ASSERTION_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:app-device-enrollment-digest\0";
const APP_STATIC_BINDING_DOMAIN: &[u8] = b"iroha:kagemusha:v1:app-device-static-binding\0";
const APP_AUTHORITY_POLICY_DOMAIN: &[u8] = b"iroha:kagemusha:v1:app-attestation-authority-policy\0";

/// Deployment-owned app-attestation authority and exact first-release app identity.
///
/// The wallet, issuer response, or device response must never select this policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaAppAttestationAuthorityPolicyV1 {
    /// Independently pinned signer of verified platform app-attestation results.
    pub authority_key: PublicKey,
    /// Exact attestation/key-service class verified by that signer.
    pub platform_class: KagemushaHardwarePlatformClassV1,
    /// Exact app signing identity admitted to the monetary release.
    pub app_signing_identity_digest: [u8; 32],
    /// Exact app build/release admitted to the monetary release.
    pub app_release_digest: [u8; 32],
    /// Longest allowed lifetime of the authority assertion.
    pub maximum_lifetime_ms: u64,
}

impl KagemushaAppAttestationAuthorityPolicyV1 {
    /// Return the exact governance-committed verifier authority and app allowlist identity.
    ///
    /// # Errors
    /// Rejects incomplete policy or an encoding failure.
    pub fn canonical_digest(&self) -> Result<[u8; 32], String> {
        if self.authority_key.algorithm() != Algorithm::Ed25519
            || self.app_signing_identity_digest == [0; 32]
            || self.app_release_digest == [0; 32]
            || self.maximum_lifetime_ms == 0
        {
            return Err("Kagemusha app authority policy is incomplete".to_owned());
        }
        let key = norito::encode_canonical(&self.authority_key)
            .map_err(|error| format!("Kagemusha app authority key encoding failed: {error}"))?;
        let key_len = u64::try_from(key.len())
            .map_err(|_| "Kagemusha app authority key is too long".to_owned())?;
        let mut digest = Sha256::new();
        digest.update(APP_AUTHORITY_POLICY_DOMAIN);
        digest.update(key_len.to_le_bytes());
        digest.update(key);
        digest.update([self.platform_class as u8]);
        digest.update(self.app_signing_identity_digest);
        digest.update(self.app_release_digest);
        digest.update(self.maximum_lifetime_ms.to_le_bytes());
        Ok(digest.finalize().into())
    }
}

impl KagemushaHardwareCredentialV1 {
    /// Check the signed credential's stable app binding against an authenticated release profile.
    ///
    /// The caller must select `release_id` from an authenticated release, not the client.
    /// # Errors
    /// Rejects issuer signature, exact approved app policy, release, profile, key or lane mismatch.
    pub fn validate_app_policy_binding_for_release(
        &self,
        profile: &KagemushaHardwareProfileV1,
        release_id: [u8; 32],
        policy: &KagemushaAppAttestationAuthorityPolicyV1,
    ) -> Result<(), String> {
        self.validate_against_profile(profile)
            .map_err(|error| format!("Kagemusha governed credential is invalid: {error}"))?;
        if release_id == [0; 32]
            || profile.platform_class != policy.platform_class
            || profile.app_attestation_authority_policy_digest != policy.canonical_digest()?
        {
            return Err("Kagemusha app authority differs from governed profile".to_owned());
        }
        let binding = KagemushaAppDevicePolicyBindingV1 {
            app_signing_identity_digest: policy.app_signing_identity_digest,
            app_release_digest: policy.app_release_digest,
            release_id,
            hardware_profile_id: self.hardware_profile_id,
            device_key_reference: self.device_key_reference,
            lane_id: self.lane_commitment,
        }
        .canonical_digest()?;
        if self.app_policy_binding_digest != binding {
            return Err(
                "Kagemusha governed credential app binding differs from approval".to_owned(),
            );
        }
        Ok(())
    }
}

/// Exact native/issuer selection for one challenge and governed device credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaAppEnrollmentSelectionV1 {
    /// The client nonce retained by the native enrollment owner.
    pub client_nonce: [u8; 32],
    /// The issuer nonce retained durably until one-use completion.
    pub server_nonce: [u8; 32],
    /// The admitted monetary release, never taken from the assertion itself.
    pub release_id: [u8; 32],
    /// The governed hardware profile selected before verification.
    /// This is stable before a credential embeds the app-policy binding.
    pub hardware_profile_id: [u8; 32],
    /// The governed device key selected before verification.
    pub device_key_reference: [u8; 32],
    /// The hardware-controlled lane selected before verification.
    pub lane_id: [u8; 32],
}

impl KagemushaAppEnrollmentSelectionV1 {
    /// Derive exact device fields from an independently selected credential.
    #[must_use]
    pub fn for_credential(
        client_nonce: [u8; 32],
        server_nonce: [u8; 32],
        release_id: [u8; 32],
        credential: &KagemushaHardwareCredentialV1,
    ) -> Self {
        Self {
            client_nonce,
            server_nonce,
            release_id,
            hardware_profile_id: credential.hardware_profile_id,
            device_key_reference: credential.device_key_reference,
            lane_id: credential.lane_commitment,
        }
    }
}

/// Stable app/device policy approved before the one-use enrollment challenge.
///
/// The governance signer can include this digest in a device credential without referring to
/// that credential's ID or to challenge nonces, so its identity preimage remains acyclic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaAppDevicePolicyBindingV1 {
    /// Exact app signing identity.
    pub app_signing_identity_digest: [u8; 32],
    /// Exact app build/release.
    pub app_release_digest: [u8; 32],
    /// Exact monetary release.
    pub release_id: [u8; 32],
    /// Governed hardware profile.
    pub hardware_profile_id: [u8; 32],
    /// Governed device key reference.
    pub device_key_reference: [u8; 32],
    /// Non-forking monetary lane.
    pub lane_id: [u8; 32],
}

impl KagemushaAppDevicePolicyBindingV1 {
    /// Compute the fixed, domain-separated policy digest.
    ///
    /// # Errors
    /// Rejects a reserved component before computing a credential-binding digest.
    pub fn canonical_digest(&self) -> Result<[u8; 32], String> {
        if [
            self.app_signing_identity_digest,
            self.app_release_digest,
            self.release_id,
            self.hardware_profile_id,
            self.device_key_reference,
            self.lane_id,
        ]
        .contains(&[0; 32])
        {
            return Err("Kagemusha app/device policy binding is incomplete".to_owned());
        }
        let mut digest = Sha256::new();
        digest.update(APP_STATIC_BINDING_DOMAIN);
        digest.update(self.app_signing_identity_digest);
        digest.update(self.app_release_digest);
        digest.update(self.release_id);
        digest.update(self.hardware_profile_id);
        digest.update(self.device_key_reference);
        digest.update(self.lane_id);
        Ok(digest.finalize().into())
    }
}

/// Platform-verifier assertion signed after checking raw app attestation.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_app_enrollment_v1::KagemushaAppEnrollmentAssertionV1",
    frame = "iroha.kagemusha.v1.app-enrollment-assertion"
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaAppEnrollmentAssertionV1 {
    /// Sole first-release assertion format.
    pub version: u16,
    /// Exact purpose, checked before accepting the signature.
    pub domain: String,
    /// Native and issuer one-use challenge nonces.
    pub client_nonce: [u8; 32],
    /// Issuer's one-use challenge nonce.
    pub server_nonce: [u8; 32],
    /// Exact app signing identity established from platform attestation.
    pub app_signing_identity_digest: [u8; 32],
    /// Exact app build/release established by trustworthy distribution evidence.
    pub app_release_digest: [u8; 32],
    /// Full raw platform-attestation evidence commitment retained by the authority.
    pub platform_evidence_digest: [u8; 32],
    /// Exact monetary release for this enrollment.
    pub release_id: [u8; 32],
    /// Exact governed hardware profile attested with this app.
    pub hardware_profile_id: [u8; 32],
    /// Exact device key attested with this app.
    pub device_key_reference: [u8; 32],
    /// Exact non-forking monetary lane attested with this app.
    pub lane_id: [u8; 32],
    /// Inclusive trusted issuance time.
    pub issued_at_ms: u64,
    /// Exclusive trusted expiry time.
    pub expires_at_ms: u64,
}

/// Canonical signed result of the independent platform app-attestation verifier.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_app_enrollment_v1::KagemushaAppEnrollmentCertificateV1",
    frame = "iroha.kagemusha.v1.app-enrollment-certificate"
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaAppEnrollmentCertificateV1 {
    /// Complete attested scope.
    pub assertion: KagemushaAppEnrollmentAssertionV1,
    /// Signature under the independently configured verifier authority.
    pub signature: SignatureOf<KagemushaAppEnrollmentAssertionV1>,
}

/// Opaque result of the exact signature, app, device, release and nonce checks.
///
/// A decoder cannot create this type. An issuer may sign its digest into the enrollment
/// certificate only after `authenticate` succeeds under its independently selected policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaVerifiedAppEnrollmentV1 {
    digest: [u8; 32],
    static_binding_digest: [u8; 32],
    selection: KagemushaAppEnrollmentSelectionV1,
    authority_policy: KagemushaAppAttestationAuthorityPolicyV1,
}

impl KagemushaVerifiedAppEnrollmentV1 {
    /// Canonical digest to include in both the account/device challenge and issuer certificate.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// Stable app/device policy binding for a governance-signed credential.
    /// This excludes both challenge nonces and the credential ID to avoid an issuance cycle.
    #[must_use]
    pub const fn static_binding_digest(&self) -> [u8; 32] {
        self.static_binding_digest
    }

    /// Exact verified selection for native one-use enrollment admission.
    #[must_use]
    pub const fn selection(&self) -> KagemushaAppEnrollmentSelectionV1 {
        self.selection
    }

    /// Exact independent policy under which the signature and app identity were checked.
    #[must_use]
    pub fn authority_policy(&self) -> &KagemushaAppAttestationAuthorityPolicyV1 {
        &self.authority_policy
    }
}

impl KagemushaAppEnrollmentCertificateV1 {
    /// Authenticate the signed platform-verifier result against independent exact selection.
    ///
    /// This authenticates the verifier's assertion; the named verifier remains responsible for
    /// validating raw Apple/Android app identity attestation and separately proving the exact
    /// build/release through a trustworthy distribution channel. Platform app attestation alone
    /// is not evidence of an exact binary hash.
    /// The caller must independently trust `policy` and supply an authoritative `trusted_time_ms`.
    ///
    /// # Errors
    /// Rejects a bad signature, stale/replayed nonce, other app, release, profile, key or lane.
    pub fn authenticate(
        &self,
        policy: &KagemushaAppAttestationAuthorityPolicyV1,
        expected: KagemushaAppEnrollmentSelectionV1,
        trusted_time_ms: u64,
    ) -> Result<KagemushaVerifiedAppEnrollmentV1, String> {
        let assertion = &self.assertion;
        if assertion.version != 1
            || assertion.domain != APP_ASSERTION_DOMAIN
            || policy.authority_key.algorithm() != Algorithm::Ed25519
            || policy.maximum_lifetime_ms == 0
            || [
                policy.app_signing_identity_digest,
                policy.app_release_digest,
                assertion.platform_evidence_digest,
                expected.client_nonce,
                expected.server_nonce,
                expected.release_id,
                expected.hardware_profile_id,
                expected.device_key_reference,
                expected.lane_id,
            ]
            .contains(&[0; 32])
            || expected.client_nonce == expected.server_nonce
            || assertion.client_nonce != expected.client_nonce
            || assertion.server_nonce != expected.server_nonce
            || assertion.release_id != expected.release_id
            || assertion.hardware_profile_id != expected.hardware_profile_id
            || assertion.device_key_reference != expected.device_key_reference
            || assertion.lane_id != expected.lane_id
            || assertion.app_signing_identity_digest != policy.app_signing_identity_digest
            || assertion.app_release_digest != policy.app_release_digest
            || assertion.issued_at_ms == 0
            || assertion.expires_at_ms <= assertion.issued_at_ms
            || assertion.expires_at_ms - assertion.issued_at_ms > policy.maximum_lifetime_ms
            || trusted_time_ms < assertion.issued_at_ms
            || trusted_time_ms >= assertion.expires_at_ms
        {
            return Err("Kagemusha app attestation scope or validity mismatch".to_owned());
        }
        self.signature
            .verify(&policy.authority_key, assertion)
            .map_err(|_| "Kagemusha app attestation authority signature rejected".to_owned())?;
        let canonical = norito::encode_canonical(self).map_err(|error| error.to_string())?;
        let mut digest = Sha256::new();
        digest.update(APP_ASSERTION_DIGEST_DOMAIN);
        digest.update((canonical.len() as u64).to_le_bytes());
        digest.update(canonical);
        let static_binding_digest = static_binding(policy, expected).canonical_digest()?;
        Ok(KagemushaVerifiedAppEnrollmentV1 {
            digest: digest.finalize().into(),
            static_binding_digest,
            selection: expected,
            authority_policy: policy.clone(),
        })
    }
}

fn static_binding(
    policy: &KagemushaAppAttestationAuthorityPolicyV1,
    selected: KagemushaAppEnrollmentSelectionV1,
) -> KagemushaAppDevicePolicyBindingV1 {
    KagemushaAppDevicePolicyBindingV1 {
        app_signing_identity_digest: policy.app_signing_identity_digest,
        app_release_digest: policy.app_release_digest,
        release_id: selected.release_id,
        hardware_profile_id: selected.hardware_profile_id,
        device_key_reference: selected.device_key_reference,
        lane_id: selected.lane_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};

    fn signed() -> (
        KagemushaAppEnrollmentCertificateV1,
        KagemushaAppAttestationAuthorityPolicyV1,
        KagemushaAppEnrollmentSelectionV1,
    ) {
        let key = KeyPair::from_seed(vec![73; 32], Algorithm::Ed25519);
        let policy = KagemushaAppAttestationAuthorityPolicyV1 {
            authority_key: key.public_key().clone(),
            platform_class: KagemushaHardwarePlatformClassV1::AppleAppAttest,
            app_signing_identity_digest: [1; 32],
            app_release_digest: [2; 32],
            maximum_lifetime_ms: 1_000,
        };
        let selection = KagemushaAppEnrollmentSelectionV1 {
            client_nonce: [3; 32],
            server_nonce: [4; 32],
            release_id: [5; 32],
            hardware_profile_id: [6; 32],
            device_key_reference: [7; 32],
            lane_id: [8; 32],
        };
        let assertion = KagemushaAppEnrollmentAssertionV1 {
            version: 1,
            domain: APP_ASSERTION_DOMAIN.to_owned(),
            client_nonce: selection.client_nonce,
            server_nonce: selection.server_nonce,
            app_signing_identity_digest: policy.app_signing_identity_digest,
            app_release_digest: policy.app_release_digest,
            platform_evidence_digest: [9; 32],
            release_id: selection.release_id,
            hardware_profile_id: selection.hardware_profile_id,
            device_key_reference: selection.device_key_reference,
            lane_id: selection.lane_id,
            issued_at_ms: 100,
            expires_at_ms: 500,
        };
        let signature = SignatureOf::try_new(key.private_key(), &assertion).unwrap();
        (
            KagemushaAppEnrollmentCertificateV1 {
                assertion,
                signature,
            },
            policy,
            selection,
        )
    }

    #[test]
    fn signed_app_attestation_is_exactly_selected() {
        let (certificate, policy, selection) = signed();
        let verified = certificate.authenticate(&policy, selection, 200).unwrap();
        assert_ne!(verified.digest(), [0; 32]);
        assert_ne!(verified.static_binding_digest(), [0; 32]);
        assert_eq!(verified.selection(), selection);
        assert_eq!(verified.authority_policy(), &policy);
    }

    #[test]
    fn governed_authority_digest_changes_for_other_issuer_app_or_release() {
        let (_, policy, _) = signed();
        let original = policy.canonical_digest().unwrap();
        let mut changed = policy.clone();
        changed.authority_key = KeyPair::from_seed(vec![74; 32], Algorithm::Ed25519)
            .public_key()
            .clone();
        assert_ne!(changed.canonical_digest().unwrap(), original);
        changed = policy.clone();
        changed.platform_class = KagemushaHardwarePlatformClassV1::AndroidKeyMint;
        assert_ne!(changed.canonical_digest().unwrap(), original);
        changed = policy.clone();
        changed.app_signing_identity_digest[0] ^= 1;
        assert_ne!(changed.canonical_digest().unwrap(), original);
        changed = policy.clone();
        changed.app_release_digest[0] ^= 1;
        assert_ne!(changed.canonical_digest().unwrap(), original);
        changed = policy.clone();
        changed.maximum_lifetime_ms += 1;
        assert_ne!(changed.canonical_digest().unwrap(), original);
        changed.app_release_digest = [0; 32];
        assert!(changed.canonical_digest().is_err());
    }

    #[test]
    fn credential_binding_is_stable_across_one_use_nonces_but_changes_with_app_or_device() {
        let (certificate, mut policy, selection) = signed();
        let verified = certificate.authenticate(&policy, selection, 200).unwrap();
        let mut later = selection;
        later.client_nonce = [21; 32];
        later.server_nonce = [22; 32];
        assert_eq!(
            static_binding(&policy, later).canonical_digest().unwrap(),
            verified.static_binding_digest()
        );
        policy.app_release_digest = [23; 32];
        assert_ne!(
            static_binding(&policy, later).canonical_digest().unwrap(),
            verified.static_binding_digest()
        );
        policy.app_release_digest = [2; 32];
        later.device_key_reference = [24; 32];
        assert_ne!(
            static_binding(&policy, later).canonical_digest().unwrap(),
            verified.static_binding_digest()
        );
    }

    #[test]
    fn static_app_device_binding_matches_cross_sdk_vector() {
        let binding = KagemushaAppDevicePolicyBindingV1 {
            app_signing_identity_digest: [1; 32],
            app_release_digest: [2; 32],
            release_id: [3; 32],
            hardware_profile_id: [4; 32],
            device_key_reference: [5; 32],
            lane_id: [6; 32],
        };
        assert_eq!(
            hex::encode(binding.canonical_digest().unwrap()),
            "d57622e75f9a50b61596d18accaeffe2e0f64531b6e7a57ec7cfde1c1b352243"
        );
    }

    #[test]
    fn other_app_or_release_is_rejected() {
        let (certificate, mut policy, selection) = signed();
        policy.app_signing_identity_digest = [10; 32];
        assert!(certificate.authenticate(&policy, selection, 200).is_err());
        policy.app_signing_identity_digest = [1; 32];
        policy.app_release_digest = [11; 32];
        assert!(certificate.authenticate(&policy, selection, 200).is_err());
        policy.app_release_digest = [2; 32];
        let mut wrong_release = selection;
        wrong_release.release_id = [12; 32];
        assert!(
            certificate
                .authenticate(&policy, wrong_release, 200)
                .is_err()
        );
    }

    #[test]
    fn other_device_key_or_lane_is_rejected() {
        let (certificate, policy, selection) = signed();
        let mut other_profile = selection;
        other_profile.hardware_profile_id = [14; 32];
        assert!(
            certificate
                .authenticate(&policy, other_profile, 200)
                .is_err()
        );
        let mut other_device = selection;
        other_device.device_key_reference = [10; 32];
        assert!(
            certificate
                .authenticate(&policy, other_device, 200)
                .is_err()
        );
        let mut other_lane = selection;
        other_lane.lane_id = [11; 32];
        assert!(certificate.authenticate(&policy, other_lane, 200).is_err());
    }

    #[test]
    fn nonce_replay_and_unsigned_substitution_are_rejected() {
        let (certificate, policy, selection) = signed();
        let mut other_client = selection;
        other_client.client_nonce = [15; 32];
        assert!(
            certificate
                .authenticate(&policy, other_client, 200)
                .is_err()
        );
        let mut next_challenge = selection;
        next_challenge.server_nonce = [12; 32];
        assert!(
            certificate
                .authenticate(&policy, next_challenge, 200)
                .is_err()
        );
        let mut tampered = certificate;
        tampered.assertion.app_release_digest = [13; 32];
        let mut substituted_policy = policy;
        substituted_policy.app_release_digest = [13; 32];
        assert!(
            tampered
                .authenticate(&substituted_policy, selection, 200)
                .is_err()
        );
    }
}
