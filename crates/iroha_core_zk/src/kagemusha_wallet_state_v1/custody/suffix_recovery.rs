//! Resume only the exact provider-selected suffix capsule, under one provider lock.
//! This path cannot select a new monetary head from caller-supplied suffix DATA.
use super::native_frames::{NativeCapsuleV1, NativeCompletionV1};
use super::suffix_owner::{NativeTransitionOwnerV1, SuffixTransitionOwnerV1};
use super::*;
use crate::kagemusha_wallet_state_v1::{Error, ExpectedHead, SuffixFrozenTransitionV1, archive};
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::SuffixCompletionRecordV1;

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1> AdvanceHandle<F, P> {
    pub(in crate::kagemusha_wallet_state_v1) fn recover_selected_suffix(
        &mut self,
        frozen: &SuffixFrozenTransitionV1,
    ) -> Result<AdvanceOutcome<SuffixCompletionRecordV1>, Error> {
        frozen.validate()?;
        let certificates: KagemushaWalletCertificateSetV1 = archive::decode(frozen.certificates())?;
        let body = frozen.capsule.body();
        let sequence = body.statement.sequence;
        let previous = sequence.checked_sub(1).ok_or(ProviderError::Invalid {
            field: "suffix recovery sequence",
        })?;
        let request = AdvanceRequest {
            expected: ExpectedHead::Released {
                sequence: previous,
                head: body.statement.predecessor,
                capsule_digest: body.predecessor_capsule_digest,
            },
            operation_id: body.operation_id,
            new_head: body.statement.successor,
            proof_digest: body.authority_digest,
            capsule: NativeCapsuleV1::Suffix(frozen.capsule.clone()),
            growth_bytes: 0,
        };
        let owner = SuffixTransitionOwnerV1::new(frozen.credential, certificates);
        let mut provider = self.lock()?;
        let status = provider.status(&self.slot)?;
        let marker = match status {
            SlotStatus::Pending(marker) | SlotStatus::Released(marker) => marker,
            _ => {
                return Err(Error::Provider(ProviderError::Invalid {
                    field: "suffix recovery requires selected head",
                }));
            }
        };
        if marker.head() != Some((sequence, body.operation_id, frozen.capsule.custody_digest()))
            || marker.marker().scheme_id != body.scheme_id
            || marker.marker().wallet_id != body.wallet_id
            || marker.marker().state != frozen.capsule.head_marker_state()
            || marker.payment_key() != &frozen.credential.body.payment_key
        {
            return Err(Error::Provider(ProviderError::Invalid {
                field: "suffix recovery selected binding",
            }));
        }
        let outcome = provider.advance(
            &self.slot,
            &NativeTransitionOwnerV1::Suffix(&owner),
            &request,
        )?;
        super::native_frames::map_outcome(outcome, NativeCompletionV1::into_suffix)
            .map_err(Error::from)
    }
}
