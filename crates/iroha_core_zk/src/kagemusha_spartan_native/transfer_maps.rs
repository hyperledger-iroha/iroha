//! Root-addressed IMT witnesses for transfers. A staged map descriptor has no monetary
//! authority. Only a root in the proved wallet state can select its contents.
use super::custody_archive::{Objects, storage_error};
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletSlotIdV1 as Slot, kagemusha_wallet_archive_object_digest_v1 as digest,
};
use crate::kagemusha_wallet_state_v1::{
    self as state, IndexRoot, ObjectWorkspace, PersistentMapV1,
};
use iroha_data_model::kagemusha::{
    KagemushaWalletIndexedInsertV1, KagemushaWalletIndexedLeafV1, KagemushaWalletIndexedOpeningV1,
};
use native::{SpartanWalletMap as Role, SpartanWalletMaps};

const DESCRIPTOR_LIMIT: usize = 1024;

struct Maps<'a, W> {
    store: &'a mut W,
    root: IndexRoot,
    failure: Option<state::Error>,
}

fn key(role: Role, root: [u8; 32]) -> [u8; 32] {
    let tag = match role {
        Role::Pending => 1,
        Role::Fee => 2,
        Role::Consumed => 3,
        Role::Credit => 4,
        Role::Recovery => 5,
    };
    digest(
        &[
            b"future-transfer-map-descriptor-v1".as_slice(),
            &[tag],
            &root,
        ]
        .concat(),
    )
}

impl<W: ObjectWorkspace> Maps<'_, W> {
    fn descriptor(&mut self, role: Role, root: [u8; 32]) -> Result<PersistentMapV1, state::Error> {
        let empty = PersistentMapV1::default();
        if empty.root() == root {
            return Ok(empty);
        }
        let address = self
            .root
            .get(self.store, &key(role, root))?
            .ok_or(state::Error::WitnessLost("future transfer map descriptor"))?;
        let address: [u8; 32] = address
            .try_into()
            .map_err(|_| state::Error::WitnessLost("future transfer map descriptor address"))?;
        let encoded = self.store.read_staged_object(&address, DESCRIPTOR_LIMIT)?;
        let map: PersistentMapV1 = norito::core::classify_decode_attempt(|| {
            norito::decode_canonical_with_limits(
                &encoded,
                norito::canonical_decode_limits(encoded.len()),
            )
        })
        .map_err(|e| {
            state::decode_attempt_error(
                e,
                state::Error::WitnessLost("future transfer map descriptor"),
            )
        })?;
        map.validate()?;
        if map.root() != root {
            return Err(state::Error::WitnessLost("future transfer map root"));
        }
        Ok(map)
    }

    fn retain(&mut self, role: Role, map: &PersistentMapV1) -> Result<(), state::Error> {
        let bytes = norito::encode_canonical(map)
            .map_err(|_| state::Error::Invalid("future transfer descriptor encoding"))?;
        let address = self.store.stage_object(&bytes, DESCRIPTOR_LIMIT)?;
        let key = key(role, map.root());
        if let Some(existing) = self.root.get(self.store, &key)? {
            if existing != address {
                return Err(state::Error::WitnessLost(
                    "future transfer descriptor conflict",
                ));
            }
        } else {
            self.root = self.root.set(self.store, key, &address)?;
        }
        Ok(())
    }

    fn map_result<T>(&mut self, result: Result<T, state::Error>) -> native::Result<T> {
        result.map_err(|err| {
            self.failure = Some(err);
            native::Error::Unavailable("protected transfer map")
        })
    }
}
impl<W: ObjectWorkspace> SpartanWalletMaps for Maps<'_, W> {
    fn member_or_low(
        &mut self,
        role: Role,
        root: [u8; 32],
        key: [u8; 32],
    ) -> native::Result<(
        KagemushaWalletIndexedLeafV1,
        KagemushaWalletIndexedOpeningV1,
    )> {
        let result = self
            .descriptor(role, root)
            .and_then(|map| map.member_or_low(self.store, &key));
        self.map_result(result)
    }
    fn insert(
        &mut self,
        role: Role,
        root: [u8; 32],
        key: [u8; 32],
        value: [u8; 32],
    ) -> native::Result<KagemushaWalletIndexedInsertV1> {
        let result = self.descriptor(role, root).and_then(|mut map| {
            let inserted = map.insert(self.store, key, value)?;
            self.retain(role, &map)?;
            Ok(inserted)
        });
        self.map_result(result)
    }
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    pub(super) fn with_transfer_maps<T>(
        &mut self,
        slot: &Slot,
        build: impl FnOnce(
            &native::NativeEngine,
            &AuthenticatedParameters,
            &mut dyn SpartanWalletMaps,
        ) -> native::Result<T>,
    ) -> Result<T, ProviderError> {
        let wallet = self.authorized_wallet_id(slot)?;
        let (expected, mut checkpoint) = self.custody_checkpoint(slot, wallet)?;
        let ((built, next), objects) = self.provider.with_archive(slot, |archive| {
            state::with_transfer_draft(&mut Objects(archive), |store| {
                let mut maps = Maps {
                    store,
                    root: IndexRoot(checkpoint.transfer_descriptors),
                    failure: None,
                };
                let built = build(&self.engine, &self.parameters, &mut maps);
                if let Some(err) = maps.failure {
                    return Err(err);
                }
                Ok((
                    built.map_err(|e| state::Error::Provider(provider_error(e)))?,
                    maps.root.0,
                ))
            })
            .map_err(storage_error)
        })?;
        if next != checkpoint.transfer_descriptors {
            let payload = objects.iter().map(|(_, bytes)| bytes.len() as u64).sum();
            let required = state::transfer_storage_required(payload, objects.len() as u64);
            if self.provider.runtime_resources()?.storage_bytes < required {
                return Err(ProviderError::NoSpace);
            }
            checkpoint.transfer_descriptors = next;
            self.provider.open_archive_call(slot);
            let published = (|| {
                for (key, bytes) in objects {
                    self.provider
                        .stage_archive_record(slot, &key, &bytes, None)?;
                }
                self.publish_custody_checkpoint(slot, expected, &checkpoint)
            })();
            let closed = self.provider.close_archive_call(slot, published.is_ok());
            published?;
            closed?;
        }
        Ok(built)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use state::ObjectStore;
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct Memory(BTreeMap<[u8; 32], Vec<u8>>);
    impl ObjectStore for Memory {
        fn read_object(&mut self, key: &[u8; 32], limit: usize) -> Result<Vec<u8>, state::Error> {
            let value = self
                .0
                .get(key)
                .ok_or(state::Error::WitnessLost("map object"))?;
            if value.len() > limit || digest(value) != *key {
                return Err(state::Error::WitnessLost("map object content"));
            }
            Ok(value.clone())
        }
        fn write_object(&mut self, value: &[u8], limit: usize) -> Result<[u8; 32], state::Error> {
            assert!(value.len() <= limit);
            let address = digest(value);
            self.0.insert(address, value.to_vec());
            Ok(address)
        }
    }
    fn word(n: u8) -> [u8; 32] {
        let mut w = [0; 32];
        w[0] = n;
        w
    }
    #[test]
    fn unpublished_map_drafts_leave_no_writes_and_distinct_roots_remain_openable_after_reopen() {
        let mut store = Memory::default();
        let empty = PersistentMapV1::default().root();
        let failed = state::with_transfer_draft(&mut store, |draft| {
            let mut maps = Maps {
                store: draft,
                root: IndexRoot::default(),
                failure: None,
            };
            maps.insert(Role::Credit, empty, word(4), word(12)).unwrap();
            Err::<(), _>(state::Error::Invalid("later proof validation refused"))
        });
        assert!(failed.is_err() && store.0.is_empty());
        let ((first, second, index), objects) = state::with_transfer_draft(&mut store, |draft| {
            let mut maps = Maps {
                store: draft,
                root: IndexRoot::default(),
                failure: None,
            };
            let one = maps
                .insert(Role::Credit, empty, word(4), word(12))
                .unwrap()
                .verify(&empty, &word(4), &word(12))
                .unwrap();
            let two = maps
                .insert(Role::Credit, one, word(7), word(21))
                .unwrap()
                .verify(&one, &word(7), &word(21))
                .unwrap();
            Ok((one, two, maps.root))
        })
        .unwrap();
        assert!(store.0.is_empty());
        for (key, bytes) in objects {
            assert_eq!(store.write_object(&bytes, bytes.len()).unwrap(), key);
        }
        let mut reopened = Maps {
            store: &mut store,
            root: index,
            failure: None,
        };
        assert_eq!(
            reopened
                .member_or_low(Role::Credit, first, word(4))
                .unwrap()
                .0
                .value,
            word(12)
        );
        assert_ne!(
            reopened
                .member_or_low(Role::Credit, first, word(7))
                .unwrap()
                .0
                .key,
            word(7)
        );
        assert_eq!(
            reopened
                .member_or_low(Role::Credit, second, word(7))
                .unwrap()
                .0
                .value,
            word(21)
        );
        assert!(
            reopened
                .member_or_low(Role::Consumed, second, word(7))
                .is_err()
        );
        let address: [u8; 32] = index
            .get(reopened.store, &key(Role::Credit, first))
            .unwrap()
            .unwrap()
            .try_into()
            .unwrap();
        reopened.store.0.remove(&address);
        assert!(
            reopened
                .member_or_low(Role::Credit, first, word(4))
                .is_err()
        );
    }
}
