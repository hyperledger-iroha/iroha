//! Marker-selected immutable roots for future wallet time observations and protected debit approvals.
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletArchiveAccessV1, KagemushaWalletSlotIdV1 as Slot,
    kagemusha_wallet_archive_object_digest_v1 as object_digest,
};
use crate::kagemusha_wallet_state_v1::{self as state, IndexRoot, ObjectStore};

pub(super) const MAX_DEBIT_APPROVAL_BYTES: usize = 1024;

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.spartan.CustodyArchiveV4")]
pub(super) struct Checkpoint {
    version: u16,
    parameters: [u8; 32],
    scheme: [u8; 32],
    wallet: [u8; 32],
    anchors: [u8; 32],
    approvals: [u8; 32],
    pub(super) requests: [u8; 32],
    pub(super) epochs: [u8;32],
    pub(super) epoch_tip: Option<u64>,
    pub(super) settlements: [u8;32],
    pub(super) transfer_descriptors: [u8; 32],
    pub(super) transfer_actions: [u8; 32],
    /// The exact fresh receipt selected by this checkpoint. The A4 financial marker
    /// inherits this root; a Pending/Selected marker cannot publish a replacement.
    chosen_debit: Option<([u8; 32], [u8; 32])>,
    pub(super) blacklist: Option<state::LocalBlacklistV1>,
    /// The one unselected regulatory record allowed during publication or cleanup.
    pub(super) unselected_blacklist: Option<[u8; 32]>,
    pub(super) apple_key: Option<(
        [u8; 32],
        iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1,
    )>,
}

impl Checkpoint {
    pub(super) fn open(
        existing: Option<([u8; 32], Vec<u8>)>,
        parameters: [u8; 32],
        scheme: [u8; 32],
        wallet: [u8; 32],
    ) -> Result<([u8; 32], Self), ProviderError> {
        let Some((selected, bytes)) = existing else {
            return Ok((
                [0; 32],
                Self {
                    version: 4,
                    parameters,
                    scheme,
                    wallet,
                    anchors: [0; 32],
                    approvals: [0; 32],
                    requests: [0; 32],
                    epochs: [0;32],
                    epoch_tip: None,
                    settlements: [0;32],
                    transfer_descriptors: [0; 32],
                    transfer_actions: [0; 32],
                    chosen_debit: None,
                    apple_key: None,
                    blacklist: None,
                    unselected_blacklist: None,
                },
            ));
        };
        let checkpoint: Self = norito::core::classify_decode_attempt(|| {
            norito::decode_canonical_with_limits(
                &bytes,
                norito::canonical_decode_limits(bytes.len()),
            )
        })
        .map_err(|error| {
            storage_error(state::decode_attempt_error(
                error,
                state::Error::WitnessLost("future custody checkpoint"),
            ))
        })?;
        if checkpoint.version != 4
            || (checkpoint.epoch_tip.is_some() != (checkpoint.epochs != [0; 32]))
            || checkpoint.parameters != parameters
            || checkpoint.scheme != scheme
            || checkpoint.wallet != wallet
            || checkpoint.chosen_debit.is_some_and(|(capsule, original)| {
                capsule == [0; 32] || original == [0; 32] || checkpoint.approvals == [0; 32]
            })
            || checkpoint.apple_key.is_some_and(|(tag, _)| tag == [0; 32])
            || checkpoint.unselected_blacklist.is_some_and(|address| {
                address == [0; 32]
                    || checkpoint
                        .blacklist
                        .as_ref()
                        .is_some_and(|active| active.original() == address)
            })
            || (checkpoint.anchors == [0; 32]
                && checkpoint.approvals == [0; 32]
                && checkpoint.requests == [0; 32]
                && checkpoint.epochs == [0; 32]
                && checkpoint.settlements == [0; 32]
                && checkpoint.transfer_descriptors == [0; 32]
                && checkpoint.transfer_actions == [0; 32]
                && checkpoint.apple_key.is_none()
                && checkpoint.blacklist.is_none()
                && checkpoint.unselected_blacklist.is_none())
        {
            return Err(ProviderError::Invalid {
                field: "future custody checkpoint scope",
            });
        }
        Ok((selected, checkpoint))
    }

    pub(super) fn retain<F: KagemushaWalletFsV1>(
        &mut self,
        archive: &KagemushaWalletArchiveAccessV1<'_, F>,
        anchored: &KagemushaWalletAnchoredTimeV1,
    ) -> Result<(), ProviderError> {
        self.anchors =
            state::retain_anchor(&mut Objects(archive), IndexRoot(self.anchors), anchored)
                .map_err(storage_error)?
                .0;
        Ok(())
    }

    pub(super) fn observation<F: KagemushaWalletFsV1>(
        &self,
        archive: &KagemushaWalletArchiveAccessV1<'_, F>,
        held: &[u8; 32],
        limit: u64,
    ) -> Result<KagemushaWalletAnchoredTimeV1, ProviderError> {
        state::read_direct_anchor(
            &mut Objects(archive),
            IndexRoot(self.anchors),
            held,
            &self.scheme,
            &self.wallet,
            limit,
        )
        .map_err(storage_error)
    }
}

pub(super) fn storage_error(error: state::Error) -> ProviderError {
    use crate::kagemusha_wallet_advance_v1::KagemushaWalletDecodeResourceV1 as Resource;
    use norito::core::DecodeLocalRefusalKind as Refusal;
    match error {
        state::Error::Provider(error) => error,
        state::Error::NotEnoughMemory => ProviderError::NoMemory,
        state::Error::DecodeResource(resource) => {
            ProviderError::DecodeResource(match resource.kind() {
                Refusal::Allocator => Resource::Allocator,
                Refusal::EnclosingLimit => Resource::EnclosingLimit,
            })
        }
        state::Error::WitnessLost(object) => ProviderError::UnavailableCustodyData { object },
        _ => ProviderError::Invalid {
            field: "future custody archive input",
        },
    }
}

// Nodes and values are immutable and are read only through a marker-selected root.
pub(super) struct Objects<'a, 'b, F: KagemushaWalletFsV1>(
    pub(super) &'a KagemushaWalletArchiveAccessV1<'b, F>,
);
impl<F: KagemushaWalletFsV1> ObjectStore for Objects<'_, '_, F> {
    fn read_object(&mut self, key: &[u8; 32], limit: usize) -> Result<Vec<u8>, state::Error> {
        let bytes = self
            .0
            .read_record(key, limit)?
            .ok_or(state::Error::WitnessLost("future custody archive object"))?;
        if object_digest(&bytes) != *key {
            return Err(state::Error::WitnessLost("future custody archive digest"));
        }
        Ok(bytes.as_slice().to_vec())
    }
    fn write_object(&mut self, bytes: &[u8], limit: usize) -> Result<[u8; 32], state::Error> {
        if bytes.is_empty() || bytes.len() > limit {
            return Err(state::Error::Invalid("future custody archive size"));
        }
        let key = object_digest(bytes);
        self.0.write_record(&key, bytes)?;
        Ok(key)
    }
}

impl Checkpoint {
    fn retain_approval<F: KagemushaWalletFsV1>(
        &mut self,
        archive: &KagemushaWalletArchiveAccessV1<'_, F>,
        capsule: [u8; 32],
        original: &[u8],
    ) -> Result<(), ProviderError> {
        if original.is_empty() || original.len() > MAX_DEBIT_APPROVAL_BYTES || capsule == [0; 32] {
            return Err(ProviderError::Invalid {
                field: "future debit approval bounds",
            });
        }
        let mut objects = Objects(archive);
        let root = IndexRoot(self.approvals);
        let address = object_digest(original);
        if let Some(prior) = root.get(&mut objects, &capsule).map_err(storage_error)? {
            if prior != address {
                return Err(ProviderError::Invalid {
                    field: "conflicting future debit approval",
                });
            }
            // Re-publish the exact bytes through the provider so an uncertain copy is adopted.
            objects
                .write_object(original, MAX_DEBIT_APPROVAL_BYTES)
                .map_err(storage_error)?;
            return Ok(());
        }
        objects
            .write_object(original, MAX_DEBIT_APPROVAL_BYTES)
            .map_err(storage_error)?;
        self.approvals = root
            .set(&mut objects, capsule, &address)
            .map_err(storage_error)?
            .0;
        Ok(())
    }

    fn approval<F: KagemushaWalletFsV1>(
        &self,
        archive: &KagemushaWalletArchiveAccessV1<'_, F>,
        capsule: &[u8; 32],
    ) -> Result<Option<Vec<u8>>, ProviderError> {
        let mut objects = Objects(archive);
        let Some(address) = IndexRoot(self.approvals)
            .get(&mut objects, capsule)
            .map_err(storage_error)?
        else {
            return Ok(None);
        };
        let key = address
            .try_into()
            .map_err(|_| ProviderError::UnavailableCustodyData {
                object: "future debit approval address",
            })?;
        objects
            .read_object(&key, MAX_DEBIT_APPROVAL_BYTES)
            .map(Some)
            .map_err(storage_error)
    }
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    pub(super) fn custody_checkpoint(
        &mut self,
        slot: &Slot,
        wallet: [u8; 32],
    ) -> Result<([u8; 32], Checkpoint), ProviderError> {
        let marker = self.provider.status(slot)?;
        if !marker.marker().is_some_and(|record| {
            record.marker().wallet_id == wallet
                && record.marker().scheme_id == self.scheme.scheme_id()
        }) {
            return Err(ProviderError::Invalid {
                field: "future custody archive marker",
            });
        }
        Checkpoint::open(
            self.provider.archive_checkpoint(slot)?,
            *self.parameters.manifest_digest(),
            *self.provider.scheme_id(),
            wallet,
        )
    }

    pub(super) fn publish_custody_checkpoint(
        &mut self,
        slot: &Slot,
        expected: [u8; 32],
        checkpoint: &Checkpoint,
    ) -> Result<(), ProviderError> {
        let bytes = norito::encode_canonical(checkpoint).map_err(|_| ProviderError::Invalid {
            field: "future custody checkpoint encoding",
        })?;
        self.provider
            .publish_archive_checkpoint(slot, expected, &bytes)?;
        Ok(())
    }
    /// Publish both the first historical approval and the exact newly authenticated choice.
    pub(super) fn choose_debit_authorization(
        &mut self,
        slot: &Slot,
        wallet: [u8; 32],
        capsule: [u8; 32],
        first: &[u8],
        chosen: &[u8],
    ) -> Result<(), ProviderError> {
        if chosen.is_empty() || chosen.len() > MAX_DEBIT_APPROVAL_BYTES {
            return Err(ProviderError::Invalid {
                field: "chosen debit approval length",
            });
        }
        let (expected, mut checkpoint) = self.custody_checkpoint(slot, wallet)?;
        self.provider.with_archive(slot, |archive| {
            checkpoint.retain_approval(archive, capsule, first)?;
            let address = Objects(archive)
                .write_object(chosen, MAX_DEBIT_APPROVAL_BYTES)
                .map_err(storage_error)?;
            checkpoint.chosen_debit = Some((capsule, address));
            Ok(())
        })?;
        self.publish_custody_checkpoint(slot, expected, &checkpoint)
    }
    pub(super) fn chosen_debit_authorization(
        &mut self,
        slot: &Slot,
        wallet: [u8; 32],
        capsule: [u8; 32],
    ) -> Result<Option<([u8; 32], Vec<u8>)>, ProviderError> {
        let (root, checkpoint) = self.custody_checkpoint(slot, wallet)?;
        let Some((held, address)) = checkpoint.chosen_debit else {
            return Ok(None);
        };
        if held != capsule {
            return Ok(None);
        }
        self.provider.with_archive(slot, |archive| {
            Objects(archive)
                .read_object(&address, MAX_DEBIT_APPROVAL_BYTES)
                .map(|bytes| Some((root, bytes)))
                .map_err(storage_error)
        })
    }
    pub(super) fn retained_debit_authorization(
        &mut self,
        slot: &Slot,
        wallet: [u8; 32],
        capsule: [u8; 32],
    ) -> Result<Option<Vec<u8>>, ProviderError> {
        let (_, checkpoint) = self.custody_checkpoint(slot, wallet)?;
        self.provider
            .with_archive(slot, |archive| checkpoint.approval(archive, &capsule))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_wallet_advance_v1::{
        KagemushaWalletAnchorPolicyV1, KagemushaWalletProviderV1, KagemushaWalletSimFsV1,
        test_support::bootstrapped_provider,
    };
    use iroha_data_model::kagemusha::{
        KagemushaWalletTimeAnchorBodyV1, KagemushaWalletTimeAnchorV1,
    };

    #[test]
    fn marker_selected_observations_survive_reopen_and_prevent_clock_replacement() {
        let fs = KagemushaWalletSimFsV1::new();
        let (mut provider, platform, slot, capsule) =
            bootstrapped_provider(fs.clone(), KagemushaWalletAnchorPolicyV1::Keychain, 0x38);
        let scheme = *provider.scheme_id();
        let wallet = capsule.body().successor_state.core.wallet_id;
        let parameters = [0x43; 32];
        let options = *provider.options();
        // Signature authentication is covered by the shared exchange tests. Here exercise both
        // durable copies, source-selected checkpoint publication and a new exclusive owner.
        let anchored = KagemushaWalletAnchoredTimeV1::new(
            KagemushaWalletTimeAnchorV1 {
                body: KagemushaWalletTimeAnchorBodyV1 {
                    version: 1,
                    scheme_id: scheme,
                    wallet_id: wallet,
                    nonce: [7; 32],
                    issuer_time_ms: 1000,
                    signer_certificate: [5; 32],
                },
                signature: KagemushaDeviceSignatureV1::from_raw_bytes(&[1; 64]).unwrap(),
            },
            [0x52; 32],
            100,
            107,
            7,
        )
        .unwrap();
        let held = anchored.anchor.time_anchor_digest();
        let (expected, mut checkpoint) = Checkpoint::open(
            provider.archive_checkpoint(&slot).unwrap(),
            parameters,
            scheme,
            wallet,
        )
        .unwrap();
        assert!(
            provider
                .with_archive(&slot, |store| checkpoint.observation(store, &held, 7))
                .is_err()
        );
        provider
            .with_archive(&slot, |store| checkpoint.retain(store, &anchored))
            .unwrap();
        let capsule = [0x61; 32];
        let receipt = b"exact protected approval bytes";
        provider
            .with_archive(&slot, |store| {
                assert_eq!(checkpoint.approval(store, &capsule)?, None);
                checkpoint.retain_approval(store, capsule, receipt)?;
                checkpoint.retain_approval(store, capsule, receipt)
            })
            .unwrap();
        assert!(
            provider
                .with_archive(&slot, |store| checkpoint.retain_approval(
                    store,
                    capsule,
                    b"changed receipt"
                ))
                .is_err()
        );
        assert!(
            provider
                .with_archive(&slot, |store| checkpoint.retain_approval(
                    store,
                    [0x62; 32],
                    &[1; MAX_DEBIT_APPROVAL_BYTES + 1]
                ))
                .is_err()
        );
        let signer = p256::ecdsa::SigningKey::from_slice(&[7; 32]).unwrap();
        let point = iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signer.verifying_key().to_sec1_point(false).as_bytes(),
        )
        .unwrap();
        let selected_pin = ([0x77; 32], point);
        checkpoint.apple_key = Some(selected_pin);
        let selected = provider
            .publish_archive_checkpoint(
                &slot,
                expected,
                &norito::encode_canonical(&checkpoint).unwrap(),
            )
            .unwrap();
        drop(provider);

        let mut provider: KagemushaWalletProviderV1<_, _> =
            KagemushaWalletProviderV1::open(fs, platform, scheme, options).unwrap();
        let original = provider.archive_checkpoint(&slot).unwrap();
        assert!(Checkpoint::open(original.clone(), [9; 32], scheme, wallet).is_err());
        let (digest, mut checkpoint) =
            Checkpoint::open(original, parameters, scheme, wallet).unwrap();
        assert_eq!(digest, selected);
        assert_eq!(checkpoint.apple_key, Some(selected_pin));
        assert_eq!(
            provider
                .with_archive(&slot, |store| checkpoint.approval(store, &capsule))
                .unwrap()
                .as_deref(),
            Some(&receipt[..])
        );
        let restored = provider
            .with_archive(&slot, |store| checkpoint.observation(store, &held, 7))
            .unwrap();
        assert_eq!(restored, anchored);
        assert!(
            provider
                .with_archive(&slot, |store| checkpoint.observation(store, &held, 6))
                .is_err()
        );
        let changed = KagemushaWalletAnchoredTimeV1 {
            request_monotonic_ms: 101,
            ..anchored
        };
        assert!(
            provider
                .with_archive(&slot, |store| checkpoint.retain(store, &changed))
                .is_err()
        );
        assert_eq!(
            provider.archive_checkpoint(&slot).unwrap().unwrap().0,
            selected
        );
    }
}
