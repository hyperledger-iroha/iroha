//! Concrete ordinary identity intake before key generation; no financial authority is created.
//!
//! The native provisioner holds the independently approved account, release and issuer policies.
//! Mobile fields cannot select a signing subject, key, issuer, financial witness or trusted time.
//! C is admitted separately from OEM/one-use state-protection qualification. The actual raw
//! attestation admission and durable E must follow before a final identity credential is published.

use iroha_data_model::kagemusha::*;
use sha2::{Digest as _, Sha256};
use std::sync::Arc;

#[path = "ordinary_app_identity/journal.rs"]
mod journal;
pub use journal::KagemushaOrdinaryAppEnrollmentAttemptV1;

/// Confidential closed ordinary identity failures; private inputs are never formatted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KagemushaOrdinaryIdentityErrorV1 {
    /// Original account, issuer, release, policy, point or transcript differs.
    #[error("ordinary app identity original rejected")]
    Rejected,
    /// Original native interval, storage or current provisioner custody is unavailable.
    #[error("ordinary app identity custody unavailable")]
    Custody,
    /// An invocation may have happened without a retained original; never invoke it again.
    #[error("ordinary app identity invocation outcome unknown")]
    UnknownOutcome,
}
type Result<T> = core::result::Result<T, KagemushaOrdinaryIdentityErrorV1>;
use KagemushaOrdinaryIdentityErrorV1::{Custody, Rejected};

/// Native-held original C preparation before an app key exists.
/// No decoder, public bytes constructor, `Clone` or platform callback recreates this owner.
/// Construction requires actual signed issuer bytes and independently selected originals.
pub struct KagemushaPreparedOrdinaryAppEnrollmentV1 {
    preparation: KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    owner: KagemushaRetailEnrollmentOwnerV1,
    trust: KagemushaOrdinaryAppTrustPolicyV1,
    authority: KagemushaAppAttestationAuthorityPolicyV1,
    issuer: KagemushaRetailEnrollmentIssuerPolicyV1,
    allowed_levels_mask: u8,
    native_scope: [u8; 32],
}
impl KagemushaPreparedOrdinaryAppEnrollmentV1 {
    /// Admit signed C under the independently installed native account and governed policies.
    ///
    /// `reserved_client_nonce` and the financial-secret commitment originate in the native
    /// attempt owner before the issuer request. They cannot be read from the offered response.
    /// The financial commitment is a scope selector only; no state proof, monetary lease,
    /// hardware monotonicity or non-forking capability is returned. No handset clock is accepted.
    /// # Errors
    /// Rejects any substituted account, original reservation, issuer/runtime, profile or interval.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate_pre_key(
        preparation: KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
        owner: KagemushaRetailEnrollmentOwnerV1,
        release: Arc<KagemushaAuthenticatedReleaseV1>,
        trust: KagemushaOrdinaryAppTrustPolicyV1,
        authority: KagemushaAppAttestationAuthorityPolicyV1,
        issuer: KagemushaRetailEnrollmentIssuerPolicyV1,
        selected_profile_id: [u8; 32],
        reserved_client_nonce: [u8; 32],
        original_financial_authority_commitment: [u8; 32],
        original_financial_epoch: u64,
        trusted_native_reference_ms: u64,
    ) -> Result<Self> {
        let c = &preparation.challenge;
        let enabled = release
            .enabled_profile(selected_profile_id)
            .ok_or(Rejected)?;
        trust
            .validate_for_profile(&enabled.hardware_profile, &authority)
            .map_err(|_| Rejected)?;
        issuer.validate().map_err(|_| Rejected)?;
        if release.purpose() != KagemushaReleasePurposeV1::Production
            || reserved_client_nonce == [0; 32]
            || original_financial_authority_commitment == [0; 32]
            || original_financial_epoch == 0
            || c.hardware_epoch != original_financial_epoch
            || c.enrollment_id != owner.enrollment_id().map_err(|_| Rejected)?
            || c.account_binding != kagemusha_ordinary_app_account_binding_v1(&owner.account_id)
            || c.client_nonce != reserved_client_nonce
            || c.financial_authority_commitment != original_financial_authority_commitment
            || c.network_id != *owner.runtime.network_id.as_bytes()
            || c.network_id != *release.network_id().as_bytes()
            || c.lane_id != owner.lane_id
            || c.release_id != release.release_id()
            || c.hardware_profile_id != selected_profile_id
            || c.suite_id != enabled.suite_id
            || c.policy_epoch != enabled.policy_epoch
            || c.platform_class != enabled.hardware_profile.platform_class
            || c.platform_class != trust.platform_class
            || c.trust_policy_digest != trust.canonical_digest().map_err(|_| Rejected)?
            || c.app_authority_policy_digest
                != authority.canonical_digest().map_err(|_| Rejected)?
            || c.issuer_policy_digest
                != kagemusha_ordinary_retail_issuer_policy_digest_v1(&issuer)
                    .map_err(|_| Rejected)?
            || issuer.runtime != owner.runtime
            || c.issued_at_ms < issuer.valid_from_ms
            || c.expires_at_ms > issuer.expires_at_ms
            || c.issued_at_ms < enabled.hardware_profile.valid_from_ms
            || c.expires_at_ms > enabled.hardware_profile.expires_at_ms
        {
            return Err(Rejected);
        }
        // Scope validation does not call the financial hardware-qualification corridor.
        preparation
            .authenticate(&issuer.issuer_public_key, c, trusted_native_reference_ms)
            .map_err(|_| Rejected)?;
        let allowed_levels_mask = allowed_mask(&trust)?;
        let original = preparation.to_transport_bytes().map_err(|_| Rejected)?;
        let mut hash = Sha256::new();
        hash.update(b"iroha:kagemusha:v1:ordinary-app-pre-key-native-scope\0");
        for bytes in [
            original.as_slice(),
            reserved_client_nonce.as_slice(),
            original_financial_authority_commitment.as_slice(),
        ] {
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        Ok(Self {
            preparation,
            release,
            owner,
            trust,
            authority,
            issuer,
            allowed_levels_mask,
            native_scope: hash.finalize().into(),
        })
    }

    /// Recheck original signed C and its exact account/release/policy without renewing it.
    /// The source owner must additionally recheck its current account approval and held journal.
    /// # Errors
    /// Rejects expired or substituted original policy or release custody.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<()> {
        let c = &self.preparation.challenge;
        self.preparation
            .authenticate(&self.issuer.issuer_public_key, c, now)
            .map_err(|_| Custody)?;
        if c.account_binding != kagemusha_ordinary_app_account_binding_v1(&self.owner.account_id)
            || self.issuer.runtime != self.owner.runtime
            || self.release.release_id() != c.release_id
            || now < self.issuer.valid_from_ms
            || now >= self.issuer.expires_at_ms
        {
            return Err(Custody);
        }
        let enabled = self
            .release
            .enabled_profile(c.hardware_profile_id)
            .ok_or(Custody)?;
        self.trust
            .validate_for_profile(&enabled.hardware_profile, &self.authority)
            .map_err(|_| Custody)
    }

    /// Borrow C only after actual native recheck; this projection is not a factory.
    /// # Errors
    /// Rejects unavailable original interval/custody.
    pub fn original_preparation(
        &self,
        now: u64,
    ) -> Result<&KagemushaSignedOrdinaryAppEnrollmentChallengeV1> {
        self.recheck_at_trusted_time(now)?;
        Ok(&self.preparation)
    }

    /// Exact C, SHA(C), native platform and admitted mask for the method21 preparation intake.
    /// This is read-only projection, not authority to generate another key or retry an invocation.
    /// # Errors
    /// Rejects another platform, alias or original interval.
    pub fn key_generation_projection(&self, now: u64) -> Result<Vec<Vec<u8>>> {
        self.recheck_at_trusted_time(now)?;
        let c = &self.preparation.challenge;
        let platform = match c.platform_class {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint => 5,
            KagemushaHardwarePlatformClassV1::AppleAppAttest => 4,
            _ => return Err(Rejected),
        };
        let alias = if platform == 5 {
            kagemusha_ordinary_android_app_key_alias_v1(c)
                .map_err(|_| Rejected)?
                .into_bytes()
        } else {
            // Apple's actual generated key ID is retained after the one-time generation;
            // it cannot be guessed or chosen before DCAppAttestService generates it.
            Vec::new()
        };
        let fields = vec![
            self.preparation
                .to_transport_bytes()
                .map_err(|_| Rejected)?,
            c.canonical_signing_bytes().map_err(|_| Rejected)?,
            c.attestation_challenge().map_err(|_| Rejected)?.to_vec(),
            vec![platform],
            alias,
            vec![self.allowed_levels_mask],
            self.native_scope.to_vec(),
        ];
        self.recheck_at_trusted_time(now)?;
        Ok(fields)
    }

    /// Retain a genuinely issuer-authenticated pending raw-attestation original before E.
    ///
    /// Raw chain/assertion bytes remain intact and must hash to the signed admission. No final
    /// credential is accepted at this phase, and the original native preparation is not renewed.
    /// # Errors
    /// Rejects oversized raw bytes, another point/alias, policy or original admission interval.
    pub fn admit_raw_attestation(
        self: Arc<Self>,
        admission: KagemushaRawAppAttestationAdmissionV1,
        raw_attestation: Vec<u8>,
        independently_held_key: &KagemushaDevicePublicKeyV1,
        original_alias: String,
        trusted_native_reference_ms: u64,
    ) -> Result<KagemushaPendingAppIdentityV1> {
        self.recheck_at_trusted_time(trusted_native_reference_ms)?;
        if raw_attestation.is_empty() || raw_attestation.len() > 128 * 1024 {
            return Err(Rejected);
        }
        let raw = admission
            .authenticate(
                &self.release,
                &self.trust,
                &self.authority,
                &self.preparation.challenge,
                trusted_native_reference_ms,
            )
            .map_err(|_| Rejected)?;
        let subject = raw.subject();
        if subject.app_public_key != *independently_held_key
            || subject.raw_platform_evidence_digest
                != <[u8; 32]>::from(Sha256::digest(&raw_attestation))
        {
            return Err(Rejected);
        }
        validate_original_alias(
            &self.preparation.challenge,
            subject.attested_key_id,
            &original_alias,
        )?;
        Ok(KagemushaPendingAppIdentityV1 {
            preparation: self,
            raw,
            raw_attestation,
            original_alias,
        })
    }
}

/// Actual pending raw-attestation original before possession and final credential publication.
/// Decoded admission fields alone never reconstruct it.
pub struct KagemushaPendingAppIdentityV1 {
    preparation: Arc<KagemushaPreparedOrdinaryAppEnrollmentV1>,
    raw: KagemushaVerifiedRawAppAttestationAdmissionV1,
    raw_attestation: Vec<u8>,
    original_alias: String,
}
impl KagemushaPendingAppIdentityV1 {
    /// Exact pending raw-admission scope for E and its non-monetary receipt.
    /// This digest is distinct from an absent final credential and does not grant authority.
    pub fn native_scope(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"iroha:kagemusha:v1:pending-raw-app-identity-scope\0");
        h.update(self.preparation.native_scope);
        h.update((self.raw.original().len() as u64).to_le_bytes());
        h.update(self.raw.original());
        h.finalize().into()
    }
    /// Recheck the original raw admission and native C without requesting a new issuer scope.
    /// # Errors
    /// Rejects expired/changed original custody.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<()> {
        self.preparation.recheck_at_trusted_time(now)?;
        self.raw.recheck_at_trusted_time(now).map_err(|_| Custody)
    }
    /// Derive the sole E371 from genuine raw admission, original C and original point.
    /// # Errors
    /// Rejects unavailable original time or scope.
    pub fn possession_challenge(
        &self,
        now: u64,
    ) -> Result<KagemushaAppEnrollmentPossessionChallengeV1> {
        self.recheck_at_trusted_time(now)?;
        KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
            &self.preparation.preparation.challenge,
            &self.raw.subject().app_public_key,
            self.raw.subject().raw_platform_evidence_digest,
        )
        .map_err(|_| Rejected)
    }
    /// Borrow independently authenticated point and raw selectors, not a final credential.
    pub fn raw_admission(&self) -> &KagemushaVerifiedRawAppAttestationAdmissionV1 {
        &self.raw
    }
    /// Borrow the exact native-selected generation alias.
    pub fn original_alias(&self) -> &str {
        &self.original_alias
    }
    /// Borrow full original platform attestation for final issuer digest correlation.
    pub fn raw_attestation(&self) -> &[u8] {
        &self.raw_attestation
    }
    /// Borrow original independently admitted pre-key owner; no authority is transferred.
    pub fn preparation(&self) -> &KagemushaPreparedOrdinaryAppEnrollmentV1 {
        &self.preparation
    }
}

fn allowed_mask(trust: &KagemushaOrdinaryAppTrustPolicyV1) -> Result<u8> {
    match trust.platform_class {
        KagemushaHardwarePlatformClassV1::AppleAppAttest => Ok(0),
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => trust
            .allowed_android_security_levels
            .iter()
            .try_fold(0, |mask, level| match level {
                KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment => Ok(mask | 1),
                KagemushaAppKeySecurityLevelV1::StrongBox => Ok(mask | 2),
                _ => Err(Rejected),
            }),
        _ => Err(Rejected),
    }
}
fn validate_original_alias(
    c: &KagemushaOrdinaryAppEnrollmentChallengeV1,
    key_id: [u8; 32],
    alias: &str,
) -> Result<()> {
    if alias.is_empty() || alias.len() > 255 || alias.as_bytes().contains(&0) {
        return Err(Rejected);
    }
    match c.platform_class {
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => {
            if alias != kagemusha_ordinary_android_app_key_alias_v1(c).map_err(|_| Rejected)? {
                return Err(Rejected);
            }
        }
        KagemushaHardwarePlatformClassV1::AppleAppAttest => {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let decoded = STANDARD.decode(alias).map_err(|_| Rejected)?;
            if decoded != key_id || STANDARD.encode(decoded) != alias {
                return Err(Rejected);
            }
        }
        _ => return Err(Rejected),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    fn prepared(f: &Fixture) -> Result<KagemushaPreparedOrdinaryAppEnrollmentV1> {
        let c = &f.selection.preparation.challenge;
        KagemushaPreparedOrdinaryAppEnrollmentV1::authenticate_pre_key(
            f.selection.preparation.clone(),
            f.selection.owner.clone(),
            f.release.clone(),
            f.trust.clone(),
            f.app_authority.clone(),
            f.issuer_policy.clone(),
            c.hardware_profile_id,
            c.client_nonce,
            c.financial_authority_commitment,
            c.hardware_epoch,
            300,
        )
    }
    #[test]
    fn pre_key_owner_uses_real_signed_original_without_a_financial_owner() {
        for apple in [false, true] {
            let f = Fixture::new(apple);
            let p = prepared(&f).unwrap();
            let fields = p.key_generation_projection(300).unwrap();
            assert_eq!(fields.len(), 7);
            assert_eq!(
                fields[0],
                f.selection.preparation.to_transport_bytes().unwrap()
            );
            assert_eq!(
                fields[1],
                f.selection
                    .preparation
                    .challenge
                    .canonical_signing_bytes()
                    .unwrap()
            );
            assert_eq!(
                fields[2],
                f.selection
                    .preparation
                    .challenge
                    .attestation_challenge()
                    .unwrap()
            );
            assert_eq!(fields[3], vec![if apple { 4 } else { 5 }]);
            assert_eq!(fields[5], vec![if apple { 0 } else { 3 }]);
            assert_eq!(fields[4].is_empty(), apple);
            assert!(
                p.key_generation_projection(f.selection.preparation.challenge.expires_at_ms)
                    .is_err()
            );
        }
    }
    #[test]
    fn pre_key_owner_refuses_offered_nonce_epoch_and_account_scope() {
        let f = Fixture::new(false);
        let c = &f.selection.preparation.challenge;
        let attempt = |nonce, epoch, owner| {
            KagemushaPreparedOrdinaryAppEnrollmentV1::authenticate_pre_key(
                f.selection.preparation.clone(),
                owner,
                f.release.clone(),
                f.trust.clone(),
                f.app_authority.clone(),
                f.issuer_policy.clone(),
                c.hardware_profile_id,
                nonce,
                c.financial_authority_commitment,
                epoch,
                300,
            )
        };
        assert!(attempt([99; 32], c.hardware_epoch, f.selection.owner.clone()).is_err());
        assert!(
            attempt(
                c.client_nonce,
                c.hardware_epoch + 1,
                f.selection.owner.clone()
            )
            .is_err()
        );
        let mut foreign = f.selection.owner.clone();
        foreign.lane_id[0] ^= 1;
        assert!(attempt(c.client_nonce, c.hardware_epoch, foreign).is_err());
        let mut signature = f.selection.preparation.clone();
        let key = iroha_crypto::KeyPair::from_seed(vec![99; 32], iroha_crypto::Algorithm::Ed25519);
        signature.signature = iroha_crypto::Signature::try_new(
            key.private_key(),
            &c.canonical_signing_bytes().unwrap(),
        )
        .unwrap();
        assert!(
            KagemushaPreparedOrdinaryAppEnrollmentV1::authenticate_pre_key(
                signature,
                f.selection.owner.clone(),
                f.release.clone(),
                f.trust.clone(),
                f.app_authority.clone(),
                f.issuer_policy.clone(),
                c.hardware_profile_id,
                c.client_nonce,
                c.financial_authority_commitment,
                c.hardware_epoch,
                300
            )
            .is_err()
        );
    }
}
