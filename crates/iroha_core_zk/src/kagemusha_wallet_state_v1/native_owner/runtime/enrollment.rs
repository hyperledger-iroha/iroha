//! Enrollment borrows the actual exclusive provider; failures never move or drop its custody.
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletChallengeLivenessV1, KagemushaWalletEnrollmentStepV1,
    KagemushaWalletKeyProfileV1, KagemushaWalletProviderV1, KagemushaWalletSlotIdV1,
    KagemushaWalletUnavailableV1,
};

fn invalid() -> ProviderError {
    ProviderError::Invalid {
        field: "native enrollment originals",
    }
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    NativeWalletRuntimeV1<F, P, S>
{
    fn enrollment_provider(
        &mut self,
    ) -> Result<&mut KagemushaWalletProviderV1<F, P>, ProviderError> {
        match &mut self.custody {
            RuntimeCustodyV1::Exclusive(provider) => Ok(provider),
            _ => Err(ProviderError::Unavailable(
                KagemushaWalletUnavailableV1::Busy,
            )),
        }
    }
    fn enrollment_intent(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        profile: KagemushaWalletKeyProfileV1,
    ) -> Result<(), ProviderError> {
        challenge.validate().map_err(|_| invalid())?;
        let provider = self.enrollment_provider()?;
        let intent = provider.read_intent(slot)?.ok_or_else(invalid)?;
        if intent.slot != slot.0
            || intent.challenge != *challenge
            || intent.profile != profile.tag()
            || challenge.scheme_id != *provider.scheme_id()
        {
            return Err(invalid());
        }
        Ok(())
    }

    /// Begin or resume the same exact E1 under Native-authenticated enrollment policy.
    /// # Errors
    /// Nonexclusive custody, invalid E1, unavailable storage or an interrupted provider step.
    pub fn begin_enrollment(
        &mut self,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        profile: KagemushaWalletKeyProfileV1,
    ) -> Result<KagemushaWalletEnrollmentStepV1, ProviderError> {
        self.enrollment_provider()?
            .begin_or_resume_enrollment(challenge, profile)
    }

    /// Resume only the slot whose durable intent contains the exact retained E1 and profile.
    /// A loaded intent cannot recreate the private original fresh-generation grant.
    /// # Errors
    /// Changed intent, completed enrollment, nonexclusive custody or provider unavailability.
    pub fn resume_enrollment(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        profile: KagemushaWalletKeyProfileV1,
    ) -> Result<KagemushaWalletEnrollmentStepV1, ProviderError> {
        self.enrollment_intent(slot, challenge, profile)?;
        self.enrollment_provider()?
            .resume_enrollment(slot, KagemushaWalletChallengeLivenessV1::Live)
    }

    /// Retain the exact transport request before sending; an earlier retained request wins.
    /// This is durable DATA, not issuer or account authorization.
    /// # Errors
    /// Changed intent, invalid/unbounded request or provider publication failure.
    pub fn retain_enrollment_request(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        profile: KagemushaWalletKeyProfileV1,
        request: &[u8],
    ) -> Result<Vec<u8>, ProviderError> {
        self.enrollment_intent(slot, challenge, profile)?;
        self.enrollment_provider()?
            .retain_enrollment_request(slot, request)
    }

    /// Verify the genuine initial credential against the actual enrollment marker and
    /// rooted issuer originals before the provider publishes its immutable credential.
    /// # Errors
    /// Changed issuer/E1/key/regulator, missing retained request or publication failure.
    pub fn store_enrollment_credential(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        profile: KagemushaWalletKeyProfileV1,
        regulatory_policy: &KagemushaWalletRegulatoryPolicyV1,
        original: &[u8],
        certificates: &[u8],
    ) -> Result<(), ProviderError> {
        self.enrollment_intent(slot, challenge, profile)?;
        if certificates.is_empty() || certificates.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 {
            return Err(invalid());
        }
        let credential =
            KagemushaWalletCredentialV1::decode_canonical(original, &challenge.scheme_id)
                .map_err(|_| invalid())?;
        let certificates: KagemushaWalletCertificateSetV1 = norito::decode_canonical_with_limits(
            certificates,
            norito::canonical_decode_limits(certificates.len()),
        )
        .map_err(|_| invalid())?;
        let scheme = *self.installed.verifier().scheme();
        certificates.verify(&scheme).map_err(|_| invalid())?;
        if certificates.certificates.len() != 1
            || credential.body.regulatory_policy != *regulatory_policy
        {
            return Err(invalid());
        }
        let certificate = certificates
            .certificate(
                &credential.body.issuer_certificate,
                KagemushaWalletSignerRoleV1::Enrollment,
            )
            .map_err(|_| invalid())?;
        let provider = self.enrollment_provider()?;
        let SlotStatus::Enrollment(marker) = provider.status(slot)? else {
            return Err(invalid());
        };
        credential
            .verify_enrollment(&scheme, certificate, challenge, marker.payment_key())
            .map_err(|_| invalid())?;
        match provider.probe_payment_key(slot)? {
            crate::kagemusha_wallet_advance_v1::KagemushaWalletProbeV1::Present(key)
                if key == *marker.payment_key() => {}
            crate::kagemusha_wallet_advance_v1::KagemushaWalletProbeV1::Unavailable(reason) => {
                return Err(ProviderError::Unavailable(reason));
            }
            _ => return Err(ProviderError::KeyLost),
        }
        provider.store_credential(slot, 0, original)
    }
}
