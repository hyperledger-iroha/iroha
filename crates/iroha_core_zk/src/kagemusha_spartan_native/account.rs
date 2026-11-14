//! Account admission for future typed custody. Account decoding and signatures use
//! the canonical intake helpers; only the selected proof frame is backend-specific.
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletLookupV1 as Lookup, KagemushaWalletMarkerRecordV1,
    KagemushaWalletProbeV1 as Probe, KagemushaWalletSlotIdV1 as Slot,
    KagemushaWalletSlotStatusV1 as Status,
};
use crate::kagemusha_wallet_intake_v1 as intake;
use iroha_data_model::{account::AccountId, kagemusha::KagemushaDevicePublicKeyV1};
use rand::rand_core::TryRng as _;

pub(super) struct AccountAdmission {
    originals: [Vec<u8>; 4],
    credential: native::Credential,
    account: AccountId,
    slot: Slot,
    marker_file_digest: [u8; 32],
    // Zero means nonce generation did not complete. The selected source remains pinned.
    challenge: [u8; 32],
    authorized: bool,
}
fn rejected(field: &'static str) -> ProviderError {
    ProviderError::Invalid { field }
}
fn original_error(error: intake::Error) -> ProviderError {
    use crate::kagemusha_wallet_advance_v1::KagemushaWalletDecodeResourceV1 as Resource;
    match error {
        intake::Error::DecodeResource(original) => {
            ProviderError::DecodeResource(match original.kind() {
                norito::core::DecodeLocalRefusalKind::Allocator => Resource::Allocator,
                norito::core::DecodeLocalRefusalKind::EnclosingLimit => Resource::EnclosingLimit,
            })
        }
        intake::Error::Provider(error) => error,
        _ => rejected("future original account admission"),
    }
}
fn same_credential(a: &native::Credential, b: &native::Credential) -> bool {
    a.body == b.body && a.signature == b.signature
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    /// Authenticate original credential/certificate/account/asset frames and derive one
    /// fresh challenge from the actual enrolled slot and current selected marker.
    /// Ordinary refusal retains any already issued challenge and selected source.
    ///
    /// # Errors
    /// Rejects foreign originals, unproven custody, an ambiguous slot or unavailable key.
    pub fn begin_account(
        &mut self,
        credential: &[u8],
        certificate: &[u8],
        account: &[u8],
        asset: &[u8],
    ) -> Result<[u8; 32], ProviderError> {
        let originals = [credential, certificate, account, asset];
        if credential.len() != 596
            || certificate.len() != 172
            || account.is_empty()
            || account.len() > intake::ACCOUNT_ORIGINAL_MAX_BYTES_V1
            || asset.is_empty()
            || asset.len() > intake::ASSET_SCOPE_ORIGINAL_MAX_BYTES_V1
        {
            return Err(rejected("future account original bounds"));
        }
        if let Some(held) = &self.account {
            if held
                .originals
                .iter()
                .zip(originals)
                .any(|(a, b)| a.as_slice() != b)
            {
                return Err(rejected("retained future account originals changed"));
            }
            if held.challenge != [0; 32] {
                return Ok(held.challenge);
            }
        } else {
            let credential = self
                .engine
                .authenticate_credential_originals(credential, certificate, &self.parameters)
                .map_err(provider_error)?;
            let account = intake::account(account, credential.body[98..130].try_into().unwrap())
                .map_err(original_error)?;
            intake::asset_scope(asset, credential.body[34..66].try_into().unwrap())
                .map_err(original_error)?;
            let (slot, marker_file_digest) = self.select_account_source(&credential)?;
            self.account = Some(AccountAdmission {
                originals: originals.map(<[u8]>::to_vec),
                credential,
                account,
                slot,
                marker_file_digest,
                challenge: [0; 32],
                authorized: false,
            });
        }
        let held = self
            .account
            .as_ref()
            .ok_or_else(|| rejected("missing future account attempt"))?;
        let slot = held.slot;
        let credential = held.credential.clone();
        let marker = self.account_source(&slot, &credential)?;
        let held = self
            .account
            .as_mut()
            .ok_or_else(|| rejected("missing future account attempt"))?;
        if marker.marker_file_digest() != &held.marker_file_digest {
            return Err(rejected("future account source changed"));
        }
        let mut nonce = [0; 32];
        rand::rngs::SysRng
            .try_fill_bytes(&mut nonce)
            .map_err(|_| ProviderError::Unavailable(KagemushaWalletUnavailableV1::Platform(0)))?;
        if nonce == [0; 32] {
            return Err(rejected("future account nonce"));
        }
        held.challenge = intake::open_message(
            &nonce,
            self.parameters.id(),
            &held.slot,
            &held.marker_file_digest,
            std::array::from_fn(|i| held.originals[i].as_slice()),
        )
        .try_into()
        .map_err(|_| rejected("future account challenge"))?;
        Ok(held.challenge)
    }

    /// Authorize the same existing Ed25519 account and recheck the exact source
    /// and positive hardware key before exposing any monetary operation.
    ///
    /// # Errors
    /// Refusal preserves the original challenge, source pin and provider owner.
    pub fn authorize_account(&mut self, signature: &[u8]) -> Result<[u8; 32], ProviderError> {
        let held = self
            .account
            .as_ref()
            .ok_or_else(|| rejected("future account begin required"))?;
        if held.challenge == [0; 32] {
            return Err(rejected("future account challenge missing"));
        }
        intake::authorize_account(&held.account, &held.challenge, signature)
            .map_err(original_error)?;
        if held.authorized {
            return Ok(held.slot.0);
        }
        let credential = held.credential.clone();
        let slot = held.slot;
        let digest = held.marker_file_digest;
        let marker = self.account_source(&slot, &credential)?;
        if marker.marker_file_digest() != &digest {
            return Err(rejected("future account source changed"));
        }
        self.account
            .as_mut()
            .ok_or_else(|| rejected("future account attempt lost"))?
            .authorized = true;
        Ok(slot.0)
    }

    pub(super) fn require_account(
        &self,
        slot: &Slot,
        credential: Option<&native::Credential>,
    ) -> Result<(), ProviderError> {
        let held = self
            .account
            .as_ref()
            .filter(|held| held.authorized && &held.slot == slot)
            .ok_or_else(|| rejected("future account authorization required"))?;
        if credential.is_some_and(|credential| !same_credential(credential, &held.credential)) {
            return Err(rejected("future account credential changed"));
        }
        Ok(())
    }
    pub(super) fn require_account_wallet(&self, wallet: &[u8; 32]) -> Result<(), ProviderError> {
        self.account
            .as_ref()
            .filter(|held| held.authorized && held.credential.body[66..98] == *wallet)
            .ok_or_else(|| rejected("future account wallet authorization required"))?;
        Ok(())
    }

    fn select_account_source(
        &mut self,
        credential: &native::Credential,
    ) -> Result<(Slot, [u8; 32]), ProviderError> {
        let mut selected = None;
        for slot in self.provider.slots()? {
            if let Some(intent) = self.provider.read_intent_for_enrollment_search(&slot)?
                && intent.challenge.scheme_id != *self.provider.scheme_id()
            {
                continue;
            }
            let status = self.provider.status(&slot)?;
            if status
                .marker()
                .is_some_and(|m| m.marker().wallet_id == credential.body[66..98])
            {
                if selected.is_some() {
                    return Err(rejected("ambiguous future account custody"));
                }
                let marker = self.account_source(&slot, credential)?;
                selected = Some((slot, *marker.marker_file_digest()));
            }
        }
        selected.ok_or_else(|| rejected("future account has no enrolled custody"))
    }

    fn account_source(
        &mut self,
        slot: &Slot,
        credential: &native::Credential,
    ) -> Result<KagemushaWalletMarkerRecordV1, ProviderError> {
        let record = match self.provider.status(slot)? {
            Status::Enrollment(record) | Status::Pending(record) | Status::Released(record) => {
                record
            }
            Status::Terminal(_) => return Err(ProviderError::Terminal),
            _ => return Err(rejected("future account enrolled marker")),
        };
        let body = &credential.body;
        let key = KagemushaDevicePublicKeyV1::try_from(&body[130..195])
            .map_err(|_| rejected("future account payment key"))?;
        let intent = self
            .provider
            .read_intent(slot)?
            .ok_or_else(|| rejected("future account enrollment intent"))?;
        let challenge = &intent.challenge;
        challenge
            .validate()
            .map_err(|_| rejected("future account enrollment challenge"))?;
        let marker = record.marker();
        if challenge.scheme_id != body[2..34]
            || challenge.asset_digest != body[34..66]
            || challenge.account_digest != body[98..130]
            || challenge.app_policy != body[404..436]
            || challenge.enrollment_id(&key) != body[448..480]
            || challenge.wallet_id(&key) != body[66..98]
            || marker.scheme_id != body[2..34]
            || marker.asset_digest != body[34..66]
            || marker.wallet_id != body[66..98]
            || marker.payment_key != key
        {
            return Err(rejected("future account retained owner key intent"));
        }
        let enrollment = self
            .provider
            .enrollment_record(slot)?
            .ok_or_else(|| rejected("future account enrollment request"))?;
        let original_marker = KagemushaWalletMarkerV1::enrollment(challenge, key)
            .map_err(|_| rejected("future account enrollment marker"))?;
        if enrollment.request.is_empty()
            || enrollment.enrollment_marker_digest
                != original_marker
                    .marker_digest()
                    .map_err(|_| rejected("future account enrollment marker digest"))?
        {
            return Err(rejected("future account retained enrollment"));
        }
        match self.provider.probe_payment_key(slot)? {
            Probe::Present(actual) if actual == key => {}
            Probe::Unavailable(reason) => return Err(ProviderError::Unavailable(reason)),
            _ => return Err(ProviderError::KeyLost),
        }
        if let Some(capsule) = self.provider.current_capsule(slot)? {
            capsule.require_marker(record.marker())?;
            let prepared = self
                .engine
                .resume(&capsule, &self.parameters)
                .map_err(provider_error)?;
            if !same_credential(prepared.credential(), credential) {
                return Err(rejected("future selected source credential"));
            }
            match self.provider.lookup(slot, capsule.operation_id())? {
                Lookup::Retained(retained) => {
                    let completed = self
                        .engine
                        .verify_completion(&capsule, &retained.record, &self.parameters)
                        .map_err(provider_error)?;
                    if !same_credential(completed.head().credential(), credential) {
                        return Err(rejected("future completed source credential"));
                    }
                }
                Lookup::SelectedUnsigned { .. } => {}
                _ => return Err(rejected("future selected source unavailable")),
            }
        } else {
            let original = [credential.body.as_slice(), credential.signature.as_slice()].concat();
            if body[488..492] != [0; 4]
                || self.provider.credential(slot, 0)?.as_deref() != Some(original.as_slice())
            {
                return Err(rejected("future retained enrollment credential"));
            }
        }
        if self
            .provider
            .status(slot)?
            .marker()
            .map(|m| m.marker_file_digest())
            != Some(record.marker_file_digest())
        {
            return Err(rejected("future account source changed"));
        }
        Ok(record)
    }
}
