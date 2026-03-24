//! SNS-backed ownership query handlers.

use iroha_data_model::query::{error::QueryExecutionFail as Error, sns::prelude::*};
use iroha_telemetry::metrics;

use super::prelude::*;
use crate::prelude::ValidSingularQuery;

impl ValidSingularQuery for FindDataspaceNameOwnerById {
    #[metrics(+"find_dataspace_name_owner_by_id")]
    fn execute(&self, state_ro: &impl StateReadOnly) -> Result<AccountId, Error> {
        let now_ms = state_ro.latest_block().map_or(0, |block| {
            u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
        });
        crate::sns::active_dataspace_owner_by_id(
            state_ro.world(),
            &state_ro.nexus().dataspace_catalog,
            self.dataspace_id(),
            now_ms,
        )
        .ok_or(Error::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        account::AccountAddress,
        metadata::Metadata,
        nexus::{DataSpaceCatalog, DataSpaceId, DataSpaceMetadata},
        query::sns::prelude::FindDataspaceNameOwnerById,
        sns::{NameControllerV1, NameRecordV1},
    };
    use mv::storage::StorageReadOnly;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn owner() -> AccountId {
        let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key");
        AccountId::new(public_key)
    }

    #[test]
    fn find_dataspace_name_owner_by_id_returns_active_owner() {
        let mut state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(9),
                alias: "trade".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("catalog");

        let selector = crate::sns::selector_for_dataspace_alias("trade").expect("selector");
        let owner = owner();
        let address = AccountAddress::from_account_id(&owner).expect("address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            10,
            4_000_000_000_000,
            4_100_000_000_000,
            4_200_000_000_000,
            Metadata::default(),
        );
        state.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );

        let view = state.view();
        let key = crate::sns::record_storage_key(&selector);
        assert!(
            view.world().smart_contract_state().get(&key).is_some(),
            "seeded SNS record must be present in raw state storage"
        );
        assert_eq!(
            crate::sns::active_dataspace_owner_by_id(
                view.world(),
                &view.nexus.dataspace_catalog,
                DataSpaceId::new(9),
                0,
            ),
            Some(owner.clone()),
            "SNS helper should resolve the active owner from the state view"
        );
        let resolved = FindDataspaceNameOwnerById::new(DataSpaceId::new(9))
            .execute(&view)
            .expect("query succeeds");
        assert_eq!(resolved, owner);
    }
}
