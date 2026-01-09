//! UAID portfolio aggregation helpers.

use std::collections::BTreeMap;

use iroha_data_model::{
    account::AccountValue,
    asset::AssetValue,
    nexus::{
        DataSpaceCatalog, DataSpaceId, UniversalAccountId,
        portfolio::{
            AccountPortfolio, AssetPosition, DataspacePortfolio, PortfolioTotals,
            UniversalPortfolio,
        },
    },
};
use mv::storage::StorageReadOnly;

use crate::{
    nexus::space_directory::UaidDataspaceBindings,
    state::{AsAssetIdAccountCompare, AssetByAccountBounds, StateReadOnly, WorldReadOnly},
};

/// Collect a deterministic UAID portfolio snapshot from the given state view.
pub fn collect_portfolio(
    state: &impl StateReadOnly,
    uaid: UniversalAccountId,
) -> UniversalPortfolio {
    let dataspace_lookup = DataspaceAliasLookup::new(&state.nexus().dataspace_catalog);
    let default_dataspace = state.nexus().routing_policy.default_dataspace;
    let directory = state.world().uaid_dataspaces().get(&uaid);
    let mut grouped: BTreeMap<DataSpaceId, Vec<AccountPortfolio>> = BTreeMap::new();
    let mut totals = PortfolioTotals::default();

    for (account_id, stored) in state.world().accounts().iter() {
        if !account_has_uaid(stored, uaid) {
            continue;
        }

        let label = stored.clone().into_inner().label;
        let mut assets = account_assets(state.world(), account_id);
        assets.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));

        let dataspace_id = resolve_account_dataspace(directory, default_dataspace, account_id);

        totals.accounts += 1;
        totals.positions += assets.len() as u64;

        grouped
            .entry(dataspace_id)
            .or_default()
            .push(AccountPortfolio {
                account_id: account_id.clone(),
                label,
                assets,
            });
    }

    let dataspaces = grouped
        .into_iter()
        .map(|(dataspace_id, mut accounts)| {
            accounts.sort_by(|a, b| a.account_id.cmp(&b.account_id));
            DataspacePortfolio {
                dataspace_id,
                dataspace_alias: dataspace_lookup
                    .alias_for(dataspace_id)
                    .map(ToString::to_string),
                accounts,
            }
        })
        .collect();

    UniversalPortfolio {
        uaid,
        dataspaces,
        totals,
    }
}

fn resolve_account_dataspace(
    directory: Option<&UaidDataspaceBindings>,
    default_dataspace: DataSpaceId,
    account_id: &iroha_data_model::account::AccountId,
) -> DataSpaceId {
    directory
        .and_then(|bindings| bindings.dataspace_for_account(account_id))
        .unwrap_or(default_dataspace)
}

fn account_has_uaid(stored: &AccountValue, uaid: UniversalAccountId) -> bool {
    stored
        .as_ref()
        .uaid()
        .is_some_and(|present| *present == uaid)
}

fn account_assets(
    world: &impl WorldReadOnly,
    account_id: &iroha_data_model::account::AccountId,
) -> Vec<AssetPosition> {
    world
        .assets()
        .range::<dyn AsAssetIdAccountCompare>(AssetByAccountBounds::new(account_id))
        .filter_map(|(asset_id, value)| asset_position(asset_id, value))
        .collect()
}

fn asset_position(
    asset_id: &iroha_data_model::asset::AssetId,
    value: &AssetValue,
) -> Option<AssetPosition> {
    if (**value).is_zero() {
        return None;
    }

    Some(AssetPosition {
        asset_id: asset_id.clone(),
        asset_definition_id: asset_id.definition().clone(),
        quantity: value.clone().into_inner(),
    })
}

struct DataspaceAliasLookup {
    aliases: BTreeMap<DataSpaceId, String>,
}

impl DataspaceAliasLookup {
    fn new(catalog: &DataSpaceCatalog) -> Self {
        let aliases = catalog
            .entries()
            .iter()
            .map(|entry| (entry.id, entry.alias.clone()))
            .collect();
        Self { aliases }
    }

    fn alias_for(&self, id: DataSpaceId) -> Option<&str> {
        self.aliases.get(&id).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::Hash;
    use iroha_data_model::{
        account::AccountDetails,
        asset::{AssetDefinition, AssetId},
        block::BlockHeader,
        common::Owned,
        domain::prelude::*,
        metadata::Metadata,
        nexus::DataSpaceMetadata,
        prelude::*,
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    #[test]
    fn aggregates_accounts_by_uaid() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(World::default(), kura, query);

        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::portfolio"));
        let first_account = iroha_test_samples::ALICE_ID.clone();
        let second_account = BOB_ID.clone();
        let domain_id = first_account.domain().clone();
        let def_id: AssetDefinitionId = format!("cash#{domain_id}").parse().unwrap();

        seed_world(
            &mut state,
            &domain_id,
            &def_id,
            &[
                (first_account.clone(), uaid, 777u64),
                (second_account.clone(), uaid, 42u64),
            ],
            None,
        );

        let snapshot = collect_portfolio(&state.view(), uaid);
        assert_eq!(snapshot.totals.accounts, 2);
        assert_eq!(snapshot.totals.positions, 2);
        assert_eq!(snapshot.dataspaces.len(), 1);
        let dataspace = &snapshot.dataspaces[0];
        assert_eq!(dataspace.dataspace_id, DataSpaceId::GLOBAL);
        assert_eq!(dataspace.accounts.len(), 2);
        let observed_ids: Vec<_> = dataspace
            .accounts
            .iter()
            .map(|entry| entry.account_id.clone())
            .collect();
        let mut sorted_ids = observed_ids.clone();
        sorted_ids.sort();
        assert_eq!(
            observed_ids, sorted_ids,
            "accounts sorted lexicographically"
        );

        let holdings: std::collections::BTreeMap<_, _> = dataspace
            .accounts
            .iter()
            .map(|entry| (entry.account_id.clone(), entry.assets[0].quantity.clone()))
            .collect();
        assert_eq!(holdings.get(&first_account), Some(&Numeric::from(777u64)));
        assert_eq!(holdings.get(&second_account), Some(&Numeric::from(42u64)));
    }

    #[test]
    fn splits_accounts_by_dataspace_when_directory_is_present() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(World::default(), kura, query);

        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::split"));
        let first_account = iroha_test_samples::ALICE_ID.clone();
        let second_account = BOB_ID.clone();
        let domain_id = first_account.domain().clone();
        let def_id: AssetDefinitionId = format!("cash#{domain_id}").parse().unwrap();

        let second_dataspace = DataSpaceId::new(11);
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata {
                id: DataSpaceId::GLOBAL,
                alias: "universal".to_string(),
                ..DataSpaceMetadata::default()
            },
            DataSpaceMetadata {
                id: second_dataspace,
                alias: "cbdc".to_string(),
                ..DataSpaceMetadata::default()
            },
        ])
        .expect("dataspace catalog");
        state.nexus.get_mut().dataspace_catalog = dataspace_catalog;

        seed_world(
            &mut state,
            &domain_id,
            &def_id,
            &[
                (first_account.clone(), uaid, 5u64),
                (second_account.clone(), uaid, 10u64),
            ],
            Some(&[
                (first_account.clone(), uaid, DataSpaceId::GLOBAL),
                (second_account.clone(), uaid, second_dataspace),
            ]),
        );

        let snapshot = collect_portfolio(&state.view(), uaid);
        assert_eq!(snapshot.totals.accounts, 2);
        assert_eq!(snapshot.dataspaces.len(), 2);

        let global_slice = &snapshot.dataspaces[0];
        assert_eq!(global_slice.dataspace_id, DataSpaceId::GLOBAL);
        assert_eq!(global_slice.accounts.len(), 1);
        assert_eq!(global_slice.accounts[0].account_id, first_account);

        let second_slice = &snapshot.dataspaces[1];
        assert_eq!(second_slice.dataspace_id, second_dataspace);
        assert_eq!(second_slice.dataspace_alias.as_deref(), Some("cbdc"));
        assert_eq!(second_slice.accounts.len(), 1);
        assert_eq!(second_slice.accounts[0].account_id, second_account);
    }

    fn seed_world(
        state: &mut State,
        domain_id: &DomainId,
        def_id: &AssetDefinitionId,
        accounts: &[(AccountId, UniversalAccountId, u64)],
        bindings: Option<&[(AccountId, UniversalAccountId, DataSpaceId)]>,
    ) {
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        {
            let world = tx.world_mut_for_testing();
            if world.domains.get(domain_id).is_none() {
                world.domains.insert(
                    domain_id.clone(),
                    Domain::new(domain_id.clone()).build(&ALICE_ID),
                );
            }
            let definition = AssetDefinition::numeric(def_id.clone()).build(&ALICE_ID);
            world.asset_definitions.insert(def_id.clone(), definition);

            for (account_id, uaid, amount) in accounts {
                let details = AccountDetails::new(Metadata::default(), None, Some(*uaid));
                world
                    .accounts
                    .insert(account_id.clone(), Owned::new(details));
                world.assets.insert(
                    AssetId::new(def_id.clone(), account_id.clone()),
                    Owned::new(Numeric::from(*amount)),
                );
            }

            if let Some(entries) = bindings {
                for (account_id, binding_uaid, dataspace) in entries {
                    if let Some(existing) = world.uaid_dataspaces.get_mut(binding_uaid) {
                        existing.bind_account(*dataspace, account_id.clone());
                    } else {
                        let mut entry = UaidDataspaceBindings::default();
                        entry.bind_account(*dataspace, account_id.clone());
                        world.uaid_dataspaces.insert(*binding_uaid, entry);
                    }
                }
            }
        }
        tx.apply();
        block.commit().expect("apply seeded block");
    }
}
