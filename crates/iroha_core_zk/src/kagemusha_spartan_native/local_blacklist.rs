//! A signed local regulatory list is wallet custody. It never enters a peer proof or Payment.
use super::custody_archive::{Checkpoint, Objects, storage_error};
use super::*;
use crate::kagemusha_wallet_advance_v1::KagemushaWalletSlotIdV1 as Slot;
use crate::kagemusha_wallet_state_v1 as state;
use iroha_data_model::kagemusha::kagemusha_wallet_local_block_v1;

pub(super) fn check<F, P, C, R>(
    provider: &mut KagemushaWalletProviderV1<F, P, C, R>,
    slot: &Slot,
    manifest: [u8; 32],
    wallet: [u8; 32],
    account: Option<[u8; 32]>,
) -> Result<(), ProviderError>
where
    F: KagemushaWalletFsV1,
    P: KagemushaWalletPlatformV1,
    C: KagemushaWalletAdvanceCapsuleV1,
    R: KagemushaWalletCompletionFrameV1,
{
    check_accounts(provider, slot, manifest, wallet, [account, None])
}

pub(super) fn check_accounts<F, P, C, R>(
    provider: &mut KagemushaWalletProviderV1<F, P, C, R>,
    slot: &Slot,
    manifest: [u8; 32],
    wallet: [u8; 32],
    accounts: [Option<[u8; 32]>; 2],
) -> Result<(), ProviderError>
where
    F: KagemushaWalletFsV1,
    P: KagemushaWalletPlatformV1,
    C: KagemushaWalletAdvanceCapsuleV1,
    R: KagemushaWalletCompletionFrameV1,
{
    if accounts == [None, None] {
        return Ok(());
    }
    let scheme = *provider.scheme_id();
    let (_, checkpoint) =
        Checkpoint::open(provider.archive_checkpoint(slot)?, manifest, scheme, wallet)?;
    if let Some(held) = checkpoint.blacklist {
        let list = provider.with_archive(slot, |archive| {
            held.read(&mut Objects(archive), &scheme)
                .map_err(storage_error)
        })?;
        for account in accounts.into_iter().flatten() {
            kagemusha_wallet_local_block_v1(Some(&list), "future counterparty", &account).map_err(
                |_| ProviderError::Invalid {
                    field: "future locally blocked counterparty",
                },
            )?;
        }
    }
    Ok(())
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    /// Authenticate the complete scheme-scoped regulatory original and update this admitted
    /// wallet's marker-selected list. Equal originals repair both copies. Only a higher version
    /// can replace an installed list, and a missing selected original never implies permission.
    ///
    /// # Errors
    /// Refuses invalid signatures, rollback, foreign custody, missing storage and uncertain writes.
    pub fn install_local_blacklist(
        &mut self,
        slot: &Slot,
        original: &[u8],
        certificates: &[u8],
    ) -> Result<[u8; 32], ProviderError> {
        self.require_account(slot, None)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let list = state::authenticate_local_blacklist(&self.scheme, original, certificates)
            .map_err(storage_error)?;
        let (mut expected, mut checkpoint) = self.custody_checkpoint(slot, wallet)?;
        let new = state::LocalBlacklistV1::select(&list, original).map_err(storage_error)?;
        let reinstall = checkpoint
            .blacklist
            .as_ref()
            .map(|old| old.reinstall(&new))
            .transpose()
            .map_err(storage_error)?
            .unwrap_or(false);
        let address = new.original();
        if let Some(previous) = checkpoint
            .unselected_blacklist
            .filter(|old| *old != address)
        {
            // May be either a superseded intent or an interrupted post-publication cleanup.
            // The marker proves this address is not the active list.
            self.provider
                .with_archive(slot, |archive| archive.remove_record(&previous))?;
        }
        if !reinstall {
            let required =
                state::blacklist_storage_required(0, original.len()).map_err(storage_error)?;
            if self.provider.runtime_resources()?.storage_bytes < required {
                return Err(ProviderError::NoSpace);
            }
            // Before creating any large file, record its address so a crash followed by
            // a different authenticated install can reclaim the interrupted original.
            if checkpoint.unselected_blacklist != Some(address) {
                checkpoint.unselected_blacklist = Some(address);
                self.publish_custody_checkpoint(slot, expected, &checkpoint)?;
                (expected, checkpoint) = self.custody_checkpoint(slot, wallet)?;
                if checkpoint.unselected_blacklist != Some(address) {
                    return Err(ProviderError::UnavailableCustodyData {
                        object: "future regulatory installation intent",
                    });
                }
            }
        }
        let digest = new.digest();
        let scheme = *self.provider.scheme_id();
        self.provider.with_archive(slot, |archive| {
            archive.restore_content_record(
                &address,
                original,
                iroha_data_model::kagemusha::KAGEMUSHA_WALLET_BLACKLIST_MAX_BYTES_V1,
            )?;
            let read = new
                .read(&mut Objects(archive), &scheme)
                .map_err(storage_error)?;
            if read != list {
                return Err(ProviderError::UnavailableCustodyData {
                    object: "future local blacklist readback",
                });
            }
            Ok(())
        })?;
        if !reinstall {
            checkpoint.unselected_blacklist =
                checkpoint.blacklist.replace(new).map(|old| old.original());
            self.publish_custody_checkpoint(slot, expected, &checkpoint)?;
            if let Some(previous) = checkpoint.unselected_blacklist {
                self.provider
                    .with_archive(slot, |archive| archive.remove_record(&previous))?;
            }
        }
        Ok(digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_wallet_advance_v1::{
        KagemushaWalletAnchorPolicyV1, KagemushaWalletSimFsV1, test_support::bootstrapped_provider,
    };
    use iroha_data_model::kagemusha::{
        KagemushaWalletBlacklistBodyV1, KagemushaWalletBlacklistEntryV1,
        KagemushaWalletBlacklistV1, kagemusha_wallet_blacklist_root_v1,
    };

    #[test]
    fn selected_list_is_enforced_after_reopen_and_missing_original_refuses() {
        let fs = KagemushaWalletSimFsV1::new();
        let (mut provider, platform, slot, capsule) =
            bootstrapped_provider(fs.clone(), KagemushaWalletAnchorPolicyV1::Keychain, 0x53);
        let scheme = *provider.scheme_id();
        let wallet = capsule.body().successor_state.core.wallet_id;
        let manifest = [0x66; 32];
        let options = *provider.options();
        let blocked = [9; 32];
        let allowed = [10; 32];
        // Signature verification and monotone replacement are exercised with real signed
        // vectors in manifest::blacklists. This fixture isolates the protected selection.
        let entries = vec![KagemushaWalletBlacklistEntryV1 {
            account_digest: blocked,
        }];
        let list = KagemushaWalletBlacklistV1 {
            body: KagemushaWalletBlacklistBodyV1 {
                version: 1,
                scheme_id: scheme,
                list_version: 1,
                issued_at_ms: 1,
                entry_count: 1,
                entries_root: kagemusha_wallet_blacklist_root_v1(&entries).unwrap(),
                signer_certificate: [3; 32],
            },
            signature: KagemushaDeviceSignatureV1::from_raw_bytes(&[1; 64]).unwrap(),
            entries,
        };
        let original = list.to_canonical_bytes().unwrap();
        let held = state::LocalBlacklistV1::select(&list, &original).unwrap();
        let address = held.original();
        check(&mut provider, &slot, manifest, wallet, Some(blocked)).unwrap();
        let (expected, mut checkpoint) = Checkpoint::open(None, manifest, scheme, wallet).unwrap();
        checkpoint.blacklist = Some(held);
        provider
            .with_archive(&slot, |store| store.write_record(&address, &original))
            .unwrap();
        provider
            .publish_archive_checkpoint(
                &slot,
                expected,
                &norito::encode_canonical(&checkpoint).unwrap(),
            )
            .unwrap();
        drop(provider);

        let mut provider: KagemushaWalletProviderV1<_, _> =
            KagemushaWalletProviderV1::open(fs, platform, scheme, options).unwrap();
        assert!(matches!(
            check(&mut provider, &slot, manifest, wallet, Some(blocked)),
            Err(ProviderError::Invalid {
                field: "future locally blocked counterparty"
            })
        ));
        check(&mut provider, &slot, manifest, wallet, Some(allowed)).unwrap();
        assert!(check(&mut provider, &slot, [0x77; 32], wallet, Some(allowed)).is_err());
        provider
            .with_archive(&slot, |store| store.remove_record(&address))
            .unwrap();
        assert!(matches!(
            check(&mut provider, &slot, manifest, wallet, Some(allowed)),
            Err(ProviderError::UnavailableCustodyData { .. })
        ));
        provider
            .with_archive(&slot, |store| store.write_record(&address, &original))
            .unwrap();
        check(&mut provider, &slot, manifest, wallet, Some(allowed)).unwrap();
    }
}
