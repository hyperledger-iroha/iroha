//! Release-authenticated public policy originals for ordinary native provisioning.
//! This admission supplies policy identity only, never account, clock, key or monetary custody.

use super::{Rejected, Result};
use iroha_data_model::kagemusha::*;
use std::sync::Arc;

/// Immutable exact ordinary policies bound to one enabled threshold-authenticated release profile.
///
/// The release must already be authenticated through the model's governed release admission.
/// Offered policy bytes cannot choose an account, runtime, issuer, Core key or trusted clock.
/// No decoder, `Clone`, public fields or application ABI recreates this checked owner.
pub struct KagemushaOrdinaryGovernedPolicyOriginalsV1 {
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    profile_id: [u8; 32],
    trust_original: Vec<u8>,
    authority_original: Vec<u8>,
    trust: KagemushaOrdinaryAppTrustPolicyV1,
    authority: KagemushaAppAttestationAuthorityPolicyV1,
}

impl KagemushaOrdinaryGovernedPolicyOriginalsV1 {
    /// Admit exact canonical public originals under a previously authenticated release.
    ///
    /// This uses the model's existing trust/profile/authority digest relationship. It does not
    /// authenticate a new release or establish current validity; Selected separately requires
    /// independent native account/runtime, issuer, Core public key and trusted-time originals.
    /// # Errors
    /// Rejects malformed or noncanonical originals, a nonproduction release, disabled/nonordinary
    /// profile, or any trust, app, authority, platform or governed digest substitution.
    pub fn authenticate(
        release: Arc<KagemushaAuthenticatedReleaseV1>,
        profile_id: [u8; 32],
        trust_original: &[u8],
        authority_original: &[u8],
    ) -> Result<Self> {
        if trust_original.is_empty()
            || trust_original.len() > KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1
        {
            return Err(Rejected);
        }
        let trust: KagemushaOrdinaryAppTrustPolicyV1 = norito::decode_canonical_with_limits(
            trust_original,
            norito::canonical_decode_limits(trust_original.len()),
        )
        .map_err(|_| Rejected)?;
        if norito::encode_canonical(&trust).map_err(|_| Rejected)? != trust_original {
            return Err(Rejected);
        }
        let authority =
            KagemushaAppAttestationAuthorityPolicyV1::decode_canonical_digest_preimage_v1(
                authority_original,
            )
            .map_err(|_| Rejected)?;
        let this = Self {
            release,
            profile_id,
            trust_original: trust_original.to_vec(),
            authority_original: authority_original.to_vec(),
            trust,
            authority,
        };
        this.recheck()?;
        Ok(this)
    }

    /// Exact admitted canonical trust-policy bytes, retained independently of caller buffers.
    #[must_use]
    pub fn original_trust_policy_bytes(&self) -> &[u8] {
        &self.trust_original
    }

    /// Exact admitted app-authority digest preimage, retained independently of caller buffers.
    #[must_use]
    pub fn original_app_authority_bytes(&self) -> &[u8] {
        &self.authority_original
    }

    pub(super) fn release(&self) -> &Arc<KagemushaAuthenticatedReleaseV1> {
        &self.release
    }
    pub(super) fn profile_id(&self) -> [u8; 32] {
        self.profile_id
    }
    pub(super) fn trust(&self) -> &KagemushaOrdinaryAppTrustPolicyV1 {
        &self.trust
    }
    pub(super) fn authority(&self) -> &KagemushaAppAttestationAuthorityPolicyV1 {
        &self.authority
    }
    pub(super) fn recheck(&self) -> Result<()> {
        let enabled = self
            .release
            .enabled_profile(self.profile_id)
            .ok_or(Rejected)?;
        if self.release.purpose() != KagemushaReleasePurposeV1::Production
            || !enabled.hardware_profile.platform_class.is_ordinary_app()
        {
            return Err(Rejected);
        }
        self.trust
            .validate_for_profile(&enabled.hardware_profile, &self.authority)
            .map_err(|_| Rejected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;

    // These maintained fixtures authenticate actual signatures under known-public test keys and
    // explicit synthetic qualification reports. They grant no installed or monetary authority.
    fn admit(
        f: &Fixture,
        trust: &[u8],
        authority: &[u8],
    ) -> Result<KagemushaOrdinaryGovernedPolicyOriginalsV1> {
        KagemushaOrdinaryGovernedPolicyOriginalsV1::authenticate(
            f.release.clone(),
            f.selection.preparation.challenge.hardware_profile_id,
            trust,
            authority,
        )
    }

    #[test]
    fn retains_exact_originals_for_both_platforms_and_governed_optional_integrity() {
        for f in [
            Fixture::new(true),
            Fixture::new(false),
            Fixture::android_with_integrity(),
        ] {
            let mut trust = norito::encode_canonical(&f.trust).unwrap();
            let mut authority = f
                .app_authority
                .canonical_digest_preimage_v1()
                .unwrap()
                .bytes;
            let governed = admit(&f, &trust, &authority).unwrap();
            assert!(Arc::ptr_eq(governed.release(), &f.release));
            assert_eq!(
                governed.profile_id(),
                f.selection.preparation.challenge.hardware_profile_id
            );
            assert_eq!(governed.trust(), &f.trust);
            assert_eq!(governed.authority(), &f.app_authority);
            trust.fill(0);
            authority.fill(0);
            assert_eq!(
                governed.original_trust_policy_bytes(),
                norito::encode_canonical(&f.trust).unwrap()
            );
            assert_eq!(
                governed.original_app_authority_bytes(),
                f.app_authority
                    .canonical_digest_preimage_v1()
                    .unwrap()
                    .bytes
            );
            governed.recheck().unwrap();
        }
    }

    #[test]
    fn rejects_wrong_profile_and_independently_signed_other_release_policies() {
        let f = Fixture::new(true);
        let trust = norito::encode_canonical(&f.trust).unwrap();
        let authority = f
            .app_authority
            .canonical_digest_preimage_v1()
            .unwrap()
            .bytes;
        assert!(
            KagemushaOrdinaryGovernedPolicyOriginalsV1::authenticate(
                f.release.clone(),
                [0; 32],
                &trust,
                &authority
            )
            .is_err()
        );
        let other = Fixture::measured_apple_with_financial_commitment(3, "1", [19; 32]).unwrap();
        assert_ne!(
            other.app_authority.canonical_digest().unwrap(),
            f.app_authority.canonical_digest().unwrap()
        );
        let other_trust = norito::encode_canonical(&other.trust).unwrap();
        let other_authority = other
            .app_authority
            .canonical_digest_preimage_v1()
            .unwrap()
            .bytes;
        admit(&other, &other_trust, &other_authority).unwrap();
        assert!(admit(&f, &other_trust, &other_authority).is_err());
    }

    #[test]
    fn rejects_structurally_valid_authority_substitutions_against_original_release() {
        let f = Fixture::new(true);
        for changed in 0..4 {
            let mut authority = f.app_authority.clone();
            match changed {
                0 => {
                    authority.authority_key = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519)
                        .public_key()
                        .clone()
                }
                1 => authority.app_signing_identity_digest[0] ^= 1,
                2 => authority.app_release_digest[0] ^= 1,
                _ => authority.maximum_lifetime_ms += 1,
            }
            let bytes = authority.canonical_digest_preimage_v1().unwrap().bytes;
            assert_eq!(
                KagemushaAppAttestationAuthorityPolicyV1::decode_canonical_digest_preimage_v1(
                    &bytes
                )
                .unwrap(),
                authority
            );
            assert!(admit(&f, &norito::encode_canonical(&f.trust).unwrap(), &bytes).is_err());
            // Altering both supplied policies still cannot alter the authenticated profile.
            let mut trust = f.trust.clone();
            trust.app_authority_policy_digest = authority.canonical_digest().unwrap();
            assert!(admit(&f, &norito::encode_canonical(&trust).unwrap(), &bytes).is_err());
        }
    }

    #[test]
    fn rejects_trust_lifetime_and_integrity_downgrade_substitutions() {
        let f = Fixture::android_with_integrity();
        let authority = f
            .app_authority
            .canonical_digest_preimage_v1()
            .unwrap()
            .bytes;
        for remove_integrity in [false, true] {
            let mut trust = f.trust.clone();
            if remove_integrity {
                trust.play_integrity_policy = None;
            } else {
                trust.maximum_credential_lifetime_ms -= 1;
            }
            trust.validate().unwrap();
            assert!(admit(&f, &norito::encode_canonical(&trust).unwrap(), &authority).is_err());
        }
    }

    #[test]
    fn rejects_noncanonical_trust_and_authority_before_retaining_an_owner() {
        let f = Fixture::new(true);
        let trust = norito::encode_canonical(&f.trust).unwrap();
        let authority = f
            .app_authority
            .canonical_digest_preimage_v1()
            .unwrap()
            .bytes;
        let mut suffix = trust.clone();
        suffix.push(0);
        let mut damaged = trust.clone();
        damaged[0] ^= 1;
        for invalid in [
            Vec::new(),
            trust[..trust.len() - 1].to_vec(),
            suffix,
            damaged,
            vec![0; KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1 + 1],
        ] {
            assert!(admit(&f, &invalid, &authority).is_err());
        }
        let mut suffix = authority.clone();
        suffix.push(0);
        for invalid in [
            Vec::new(),
            authority[..authority.len() - 1].to_vec(),
            suffix,
        ] {
            assert!(admit(&f, &trust, &invalid).is_err());
        }
    }
}
