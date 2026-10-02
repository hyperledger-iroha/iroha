//! Join the independently authenticated Core identity purpose to the separate FI retail purpose.
//! This retains public policy originals only. It constructs no Native, Current, signer or money owner.

use super::*;
use crate::account::AccountId;
use std::sync::Arc;

/// Checked public policy originals for the two distinct ordinary enrollment signing roles.
/// No decoder, public fields or Clone implementation recreates this policy owner.
pub struct KagemushaOrdinaryRetailIdentityPolicyOriginalsV1 {
    identity: Arc<KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1>,
    issuer: Arc<KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1>,
    namespace: [u8; 32],
    retail: KagemushaRetailEnrollmentIssuerPolicyV1,
    release_id: [u8; 32],
    profile_id: [u8; 32],
}

impl KagemushaOrdinaryRetailIdentityPolicyOriginalsV1 {
    /// Join actual threshold-authenticated Core originals with the independently installed FI policy.
    /// The installing Native/FI owner must retain the FI policy's independent descriptor custody.
    /// This policy-only admission grants neither account/current ownership nor trusted time.
    /// # Errors
    /// Refuses substituted release/profile/trust/application/namespace/network or reused signing roles.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate(
        identity: Arc<KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1>,
        issuer: Arc<KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1>,
        independently_installed_namespace: [u8; 32],
        independently_installed_retail_policy: KagemushaRetailEnrollmentIssuerPolicyV1,
        release: &KagemushaAuthenticatedReleaseV1,
        independently_selected_profile: [u8; 32],
        now: u64,
    ) -> Result<Self, String> {
        let result = Self {
            identity,
            issuer,
            namespace: independently_installed_namespace,
            retail: independently_installed_retail_policy,
            release_id: release.release_id(),
            profile_id: independently_selected_profile,
        };
        result.recheck_current(release, now)?;
        Ok(result)
    }

    /// Recheck the same complete public originals under the actual owner's current clock interval.
    /// # Errors
    /// Refuses stale or changed exact policy purpose, profile, key, network or original interval.
    pub fn recheck_current(
        &self,
        release: &KagemushaAuthenticatedReleaseV1,
        now: u64,
    ) -> Result<(), String> {
        self.issuer
            .recheck_current(&self.identity, self.namespace, now)?;
        self.retail.validate().map_err(|error| error.to_string())?;
        let enabled = release
            .enabled_profile(self.profile_id)
            .ok_or("ordinary retail identity selected profile is absent")?;
        let policy = self.identity.policy();
        let planned = &policy.profile;
        if release.purpose() != KagemushaReleasePurposeV1::Production
            || release.release_id() != self.release_id
            || !enabled.hardware_profile.platform_class.is_ordinary_app()
            || self.retail.runtime.network_id != release.network_id()
            || policy.network_id != release.network_id()
            || planned.planned_release_id != self.release_id
            || planned.planned_hardware_profile_id != self.profile_id
            || planned.planned_suite_id != enabled.suite_id
            || planned.policy_epoch != enabled.policy_epoch
            || planned.platform_class != enabled.hardware_profile.platform_class
            || planned.trust_policy_digest != enabled.hardware_profile.firmware_policy_digest
            || planned.app_authority_policy_digest
                != enabled
                    .hardware_profile
                    .app_attestation_authority_policy_digest
            || planned.platform_trust_roots_digest
                != enabled.hardware_profile.attestation_trust_roots_digest
            || policy.enrollment_issuer_p256_key
                != enabled.hardware_profile.governance_credential_public_key
            || policy.enrollment_issuer_key == self.retail.issuer_public_key
            || policy.app_authority_key == self.retail.issuer_public_key
            || now < self.retail.valid_from_ms
            || now >= self.retail.expires_at_ms
            || now < enabled.hardware_profile.valid_from_ms
            || now >= enabled.hardware_profile.expires_at_ms
        {
            return Err("ordinary Core identity and FI retail policy originals differ".into());
        }
        Ok(())
    }

    /// Exact threshold-checked Core policy; no signer, Current or Native owner is exposed.
    #[must_use]
    pub fn identity_policy(&self) -> &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1 {
        &self.identity
    }
    /// Exact threshold-checked complete Core issuer policy, distinct from the FI retail key.
    #[must_use]
    pub fn issuer_policy(&self) -> &KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1 {
        &self.issuer
    }
    /// Complete independently installed FI retail policy DATA, without private signer custody.
    #[must_use]
    pub fn retail_policy(&self) -> &KagemushaRetailEnrollmentIssuerPolicyV1 {
        &self.retail
    }
    /// Derive the sole account/FI lane from the mandatory threshold issuer namespace.
    /// This returns DATA; the Native caller must independently hold its actual account.
    /// # Errors
    /// Refuses current policy or exact namespace/runtime drift.
    pub fn enrollment_lane(
        &self,
        release: &KagemushaAuthenticatedReleaseV1,
        account: &AccountId,
        now: u64,
    ) -> Result<[u8; 32], String> {
        self.recheck_current(release, now)?;
        self.issuer.derive_enrollment_lane(
            &self.identity,
            self.namespace,
            &self.retail.runtime.fi_id,
            account,
            now,
        )
    }
    /// Join a full DATA owner to the exact independently installed policy/runtime/lane.
    /// This does not construct or authenticate a Native account/current owner.
    /// # Errors
    /// Refuses changed full FI runtime, account-specific lane or derived stable enrollment ID.
    pub fn require_owner_data(
        &self,
        release: &KagemushaAuthenticatedReleaseV1,
        owner: &KagemushaRetailEnrollmentOwnerV1,
        now: u64,
    ) -> Result<(), String> {
        if owner.runtime != self.retail.runtime
            || owner.lane_id != self.enrollment_lane(release, &owner.account_id, now)?
            || owner.enrollment_id().map_err(|error| error.to_string())? == [0; 32]
        {
            return Err("ordinary selected full owner/runtime/lane differs".into());
        }
        Ok(())
    }
    /// Authenticate the actual Core-signed C under its distinct original purpose and full owner.
    /// No fresh C, clock anchor, Native reservation or current grant is constructed.
    /// # Errors
    /// Refuses substituted C, governed epochs, original lifetime, owner or expired preparation.
    pub fn authenticate_preparation(
        &self,
        release: &KagemushaAuthenticatedReleaseV1,
        owner: &KagemushaRetailEnrollmentOwnerV1,
        original: &KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
        now: u64,
    ) -> Result<(), String> {
        self.require_owner_data(release, owner, now)?;
        let challenge = &original.challenge;
        let issuer = self.issuer.policy();
        if challenge.enrollment_id != owner.enrollment_id().map_err(|error| error.to_string())?
            || challenge.account_binding
                != kagemusha_ordinary_app_account_binding_v1(&owner.account_id)
            || challenge.lane_id != owner.lane_id
            || challenge.hardware_epoch != issuer.planned_hardware_epoch
            || challenge.issuer_policy_digest != issuer.canonical_digest()?
            || challenge
                .expires_at_ms
                .checked_sub(challenge.issued_at_ms)
                .filter(|value| *value > 0 && *value <= issuer.maximum_pending_lifetime_ms)
                .is_none()
        {
            return Err("ordinary Core C differs from selected issuer/owner originals".into());
        }
        self.identity
            .authenticate_preparation(original, challenge, now)
            .map(|_| ())
    }
}

impl core::fmt::Debug for KagemushaOrdinaryRetailIdentityPolicyOriginalsV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("KagemushaOrdinaryRetailIdentityPolicyOriginalsV1")
            .finish_non_exhaustive()
    }
}
impl PartialEq for KagemushaOrdinaryRetailIdentityPolicyOriginalsV1 {
    fn eq(&self, other: &Self) -> bool {
        self.identity.original() == other.identity.original()
            && self.identity.authority_original() == other.identity.authority_original()
            && self.issuer.original() == other.issuer.original()
            && self.namespace == other.namespace
            && self.retail == other.retail
            && self.release_id == other.release_id
            && self.profile_id == other.profile_id
    }
}
impl Eq for KagemushaOrdinaryRetailIdentityPolicyOriginalsV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use p256::ecdsa::SigningKey;

    #[test]
    fn dual_policy_keeps_threshold_core_app_and_fi_purposes_and_exact_lane() {
        for apple in [false, true] {
            let f = Fixture::new(apple);
            let p = &f.ordinary_policy;
            p.recheck_current(&f.release, 300).unwrap();
            p.require_owner_data(&f.release, &f.selection.owner, 300)
                .unwrap();
            p.authenticate_preparation(
                &f.release,
                &f.selection.owner,
                &f.selection.preparation,
                300,
            )
            .unwrap();
            assert_eq!(
                p.enrollment_lane(&f.release, &f.selection.owner.account_id, 300)
                    .unwrap(),
                f.selection.owner.lane_id
            );
            assert_ne!(
                p.identity_policy().policy().enrollment_issuer_key,
                f.issuer_policy.issuer_public_key
            );
            assert_ne!(
                p.identity_policy().policy().app_authority_key,
                f.issuer_policy.issuer_public_key
            );
            assert_ne!(
                p.identity_policy().policy().enrollment_issuer_key,
                p.identity_policy().policy().app_authority_key
            );
            assert_eq!(
                p.issuer_policy().policy().canonical_digest().unwrap(),
                f.selection.preparation.challenge.issuer_policy_digest
            );
            assert_ne!(
                f.selection.preparation.challenge.issuer_policy_digest,
                kagemusha_ordinary_retail_issuer_policy_digest_v1(&f.issuer_policy).unwrap()
            );
        }
    }

    #[test]
    fn dual_policy_rejects_fi_key_reuse_and_valid_foreign_runtime_and_owner_data() {
        let f = Fixture::new(false);
        let p = &f.ordinary_policy;
        for key in [
            &p.identity_policy().policy().enrollment_issuer_key,
            &p.identity_policy().policy().app_authority_key,
        ] {
            let mut retail = f.issuer_policy.clone();
            retail.issuer_public_key = key.clone();
            assert!(
                KagemushaOrdinaryRetailIdentityPolicyOriginalsV1::authenticate(
                    Arc::clone(&p.identity),
                    Arc::clone(&p.issuer),
                    p.namespace,
                    retail,
                    &f.release,
                    p.profile_id,
                    300,
                )
                .is_err()
            );
        }
        for selector in 0..4 {
            let mut owner = f.selection.owner.clone();
            match selector {
                0 => owner.lane_id[0] ^= 1,
                1 => owner.runtime.scale += 1,
                2 => owner.runtime.fi_id = "foreign".parse().unwrap(),
                _ => {
                    owner.account_id = crate::account::AccountId::new(
                        KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519)
                            .public_key()
                            .clone(),
                    )
                }
            }
            assert!(p.require_owner_data(&f.release, &owner, 300).is_err());
        }
    }

    #[test]
    fn dual_policy_genuine_threshold_wrong_p256_pin_cannot_replace_release_pin() {
        let f = Fixture::new(false);
        let p = &f.ordinary_policy;
        let mut signed = KagemushaSignedOrdinaryAppIdentityPolicyV1::decode_canonical_exact(
            p.identity.original(),
        )
        .unwrap();
        let wrong = SigningKey::from_bytes((&[8; 32]).into()).unwrap();
        signed.policy.enrollment_issuer_p256_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            wrong.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let message = signed.policy.approval_signing_bytes().unwrap();
        signed.approvals = [81, 82]
            .into_iter()
            .map(|seed| {
                let key = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
                KagemushaOrdinaryAppIdentityPolicyApprovalV1 {
                    public_key: key.public_key().clone(),
                    signature: Signature::try_new(key.private_key(), &message).unwrap(),
                }
            })
            .collect();
        signed
            .approvals
            .sort_by(|a, b| a.public_key.cmp(&b.public_key));
        let mut roots: KagemushaOrdinaryAppIdentityAuthorityPolicyV1 =
            norito::decode_canonical_with_limits(
                p.identity.authority_original(),
                norito::canonical_decode_limits(p.identity.authority_original().len()),
            )
            .unwrap();
        roots.expected_identity_policy_id = signed.policy.canonical_digest().unwrap();
        let changed = Arc::new(signed.authenticate(&roots, 100).unwrap());
        let issuer = Arc::new(
            p.issuer
                .policy()
                .authenticate_under_policy(&changed, p.namespace, 100)
                .unwrap(),
        );
        assert!(
            KagemushaOrdinaryRetailIdentityPolicyOriginalsV1::authenticate(
                changed,
                issuer,
                p.namespace,
                f.issuer_policy.clone(),
                &f.release,
                p.profile_id,
                300
            )
            .is_err()
        );
    }

    #[test]
    fn dual_policy_original_c_cannot_choose_epoch_namespace_signature_or_new_expiry() {
        let f = Fixture::new(false);
        let p = &f.ordinary_policy;
        for selector in 0..4 {
            let mut c = f.selection.preparation.clone();
            match selector {
                0 => c.challenge.hardware_epoch += 1,
                1 => c.challenge.lane_id[0] ^= 1,
                2 => {
                    c.challenge.expires_at_ms = c.challenge.issued_at_ms
                        + p.issuer_policy().policy().maximum_pending_lifetime_ms
                        + 1
                }
                _ => c.challenge.client_nonce[0] ^= 1,
            }
            let signer = KeyPair::from_seed(
                vec![if selector == 3 { 64 } else { 63 }; 32],
                Algorithm::Ed25519,
            );
            c.signature = Signature::try_new(
                signer.private_key(),
                &c.challenge.canonical_signing_bytes().unwrap(),
            )
            .unwrap();
            assert!(
                p.authenticate_preparation(&f.release, &f.selection.owner, &c, 300)
                    .is_err()
            );
        }
        assert!(
            p.authenticate_preparation(
                &f.release,
                &f.selection.owner,
                &f.selection.preparation,
                f.selection.preparation.challenge.expires_at_ms
            )
            .is_err()
        );
        assert!(
            p.recheck_current(&f.release, f.issuer_policy.expires_at_ms)
                .is_err()
        );
    }
}
