//! Source-bound nonmonetary setup signing through the exclusive provider.

use super::{
    advance::KagemushaWalletAdvanceCapsuleV1, completion::KagemushaWalletCompletionFrameV1, *,
};
use iroha_data_model::kagemusha::{
    KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaWalletSigningDomainV1,
};

impl<
    F: KagemushaWalletFsV1,
    P: KagemushaWalletPlatformV1,
    C: KagemushaWalletAdvanceCapsuleV1,
    R: KagemushaWalletCompletionFrameV1,
> KagemushaWalletProviderV1<F, P, C, R>
{
    /// Native setup owner has authenticated and derived this exact body from its selected head.
    /// This private path grants neither receipt signing nor foreign signing access.
    pub(crate) fn sign_setup(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        source: [u8; 32],
        key: &KagemushaDevicePublicKeyV1,
        domain: KagemushaWalletSigningDomainV1,
        body: &[u8],
    ) -> Result<KagemushaDeviceSignatureV1, KagemushaWalletProviderErrorV1> {
        if !matches!(
            domain,
            KagemushaWalletSigningDomainV1::Offer | KagemushaWalletSigningDomainV1::Request
        ) {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "setup.domain",
            });
        }
        let KagemushaWalletSlotStatusV1::Released(marker) = self.status(slot)? else {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "setup.requires_released",
            });
        };
        if marker.head().map(|(_, _, capsule)| capsule) != Some(source)
            || marker.payment_key() != key
        {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "setup.source",
            });
        }
        self.require_storage()?;
        let result = kagemusha_wallet_sign_domain_v1(&self.platform, slot, key, domain, body)
            .map_err(Into::into);
        self.require_storage().and(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_wallet_advance_v1::test_support::{bootstrapped_device, enrolled_device};
    #[test]
    fn setup_signing_requires_exact_released_source_key_domain_and_available_storage() {
        let (device, fixture, slot, _) =
            bootstrapped_device(KagemushaWalletAnchorPolicyV1::NotRequired, 0x37);
        let mut provider = device.open();
        let source = provider
            .status(&slot)
            .unwrap()
            .marker()
            .unwrap()
            .head()
            .unwrap()
            .2;
        let body = vec![0; KagemushaWalletSigningDomainV1::Offer.transcript_bytes()];
        let before = device.platform.with(|p| p.sign_calls);
        assert!(
            provider
                .sign_setup(
                    &slot,
                    [99; 32],
                    &fixture.payment_key,
                    KagemushaWalletSigningDomainV1::Offer,
                    &body
                )
                .is_err()
        );
        assert!(
            provider
                .sign_setup(
                    &slot,
                    source,
                    &fixture.payment_key,
                    KagemushaWalletSigningDomainV1::Receipt,
                    &body
                )
                .is_err()
        );
        assert_eq!(device.platform.with(|p| p.sign_calls), before);
        provider
            .sign_setup(
                &slot,
                source,
                &fixture.payment_key,
                KagemushaWalletSigningDomainV1::Offer,
                &body,
            )
            .unwrap();
        assert_eq!(device.platform.with(|p| p.sign_calls), before + 1);
        device
            .platform
            .with(|p| p.storage = Err(KagemushaWalletUnavailableV1::Locked));
        assert!(matches!(
            provider.sign_setup(
                &slot,
                source,
                &fixture.payment_key,
                KagemushaWalletSigningDomainV1::Offer,
                &body
            ),
            Err(KagemushaWalletProviderErrorV1::Unavailable(
                KagemushaWalletUnavailableV1::Locked
            ))
        ));
        assert_eq!(device.platform.with(|p| p.sign_calls), before + 1);
    }
    #[test]
    fn setup_signing_never_uses_an_enrollment_as_a_released_head() {
        let (device, fixture, slot) =
            enrolled_device(KagemushaWalletAnchorPolicyV1::NotRequired, 0x38);
        let mut provider = device.open();
        assert!(
            provider
                .sign_setup(
                    &slot,
                    [1; 32],
                    &fixture.payment_key,
                    KagemushaWalletSigningDomainV1::Request,
                    &vec![0; KagemushaWalletSigningDomainV1::Request.transcript_bytes()]
                )
                .is_err()
        );
        assert_eq!(device.platform.with(|p| p.sign_calls), 0);
    }
}
