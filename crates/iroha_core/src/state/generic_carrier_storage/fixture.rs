//! Fixture-only raw insertion through the actual table's original admitted owners.

use super::*;

pub(crate) trait CarrierFixtureStorage<K> {
    fn insert(&mut self, key: K, bytes: Vec<u8>) -> Option<RetainedCarrierBytes>;
    fn remove(&mut self, key: K) -> Option<RetainedCarrierBytes>;
}

fn key(key: CarrierStorageKeyV1, budget: &AllocationBudget) -> RetainedCarrierKey {
    RetainedCarrierKey::admit(&key, budget).expect("fixture original key pool")
}
fn artifact(
    key: iroha_data_model::smart_contract::ContractArtifactId,
    _: &AllocationBudget,
) -> iroha_data_model::smart_contract::ContractArtifactId {
    key
}

macro_rules! fixtures {
    ($storage:ty, $native:ty, $key:ty, $mode:ty, $prepare:ident) => {
        impl CarrierFixtureStorage<$native> for $storage {
            fn insert(&mut self, key: $native, bytes: Vec<u8>) -> Option<RetainedCarrierBytes> {
                let key = $prepare(key, self.allocation_budget());
                let value = RetainedCarrierBytes::admit(&bytes, self.allocation_budget()).unwrap();
                self.try_with_admitted_block(|block| {
                    block.try_insert_admitted(key, value).map_err(|(_, e)| e)
                })
                .unwrap()
            }
            fn remove(&mut self, key: $native) -> Option<RetainedCarrierBytes> {
                let key = $prepare(key, self.allocation_budget());
                self.try_with_admitted_block(|block| {
                    block.try_remove_admitted(key).map_err(|(_, e)| e)
                })
                .unwrap()
            }
        }
        impl CarrierFixtureStorage<$native>
            for mv::storage::Block<'_, $key, RetainedCarrierBytes, $mode>
        {
            fn insert(&mut self, key: $native, bytes: Vec<u8>) -> Option<RetainedCarrierBytes> {
                let key = $prepare(key, self.allocation_budget());
                let value = RetainedCarrierBytes::admit(&bytes, self.allocation_budget()).unwrap();
                self.try_insert_admitted(key, value)
                    .map_err(|(_, e)| e)
                    .unwrap()
            }
            fn remove(&mut self, key: $native) -> Option<RetainedCarrierBytes> {
                let key = $prepare(key, self.allocation_budget());
                self.try_remove_admitted(key).map_err(|(_, e)| e).unwrap()
            }
        }
        impl CarrierFixtureStorage<$native>
            for mv::storage::Transaction<'_, $key, RetainedCarrierBytes, $mode>
        {
            fn insert(&mut self, key: $native, bytes: Vec<u8>) -> Option<RetainedCarrierBytes> {
                let key = $prepare(key, self.allocation_budget());
                let value = RetainedCarrierBytes::admit(&bytes, self.allocation_budget()).unwrap();
                self.try_insert_admitted(key, value)
                    .map_err(|(_, e)| e)
                    .unwrap()
            }
            fn remove(&mut self, key: $native) -> Option<RetainedCarrierBytes> {
                let key = $prepare(key, self.allocation_budget());
                self.try_remove_admitted(key).map_err(|(_, e)| e).unwrap()
            }
        }
    };
}
fixtures!(
    CarrierStorage,
    CarrierStorageKeyV1,
    RetainedCarrierKey,
    CarrierStorageMode,
    key
);
fixtures!(
    ArtifactStorage,
    iroha_data_model::smart_contract::ContractArtifactId,
    iroha_data_model::smart_contract::ContractArtifactId,
    ArtifactStorageMode,
    artifact
);
