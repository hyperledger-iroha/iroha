//! Smart contract which executes [`FindAssets`] and saves cursor to the owner's metadata.

#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

extern crate alloc;

use alloc::collections::BTreeSet;

use dlmalloc::GlobalDlmalloc;
use iroha_smart_contract::{prelude::*, Iroha};

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

/// Create two asset definitions in the looking_glass domain, query all asset definitions, filter them to only be in the looking_glass domain, check that the results are consistent
#[iroha_smart_contract::main]
fn main(host: Iroha, _context: Context) {
    let domain_id: DomainId = "looking_glass".parse().unwrap();
    host.submit(&Register::domain(Domain::new(domain_id.clone())))
        .dbg_unwrap();

    // create two asset definitions inside the `looking_glass` domain
    let time_id: AssetDefinitionId = "time#looking_glass".parse().dbg_unwrap();
    let space_id: AssetDefinitionId = "space#looking_glass".parse().dbg_unwrap();

    host.submit(&Register::asset_definition(AssetDefinition::new(
        time_id.clone(),
        NumericSpec::default(),
    )))
    .dbg_unwrap();

    host.submit(&Register::asset_definition(AssetDefinition::new(
        space_id.clone(),
        NumericSpec::default(),
    )))
    .dbg_unwrap();

    // genesis registers some more asset definitions, but we apply a filter to find only the ones from the `looking_glass` domain
    let cursor = host
        .query(FindAssetsDefinitions)
        .filter_with(|asset_definition| asset_definition.id.domain.eq(domain_id.clone()))
        .execute()
        .dbg_unwrap();

    let mut asset_definition_ids = BTreeSet::new();

    for asset_definition in cursor {
        let asset_definition = asset_definition.dbg_unwrap();
        asset_definition_ids.insert(asset_definition.id().clone());
    }

    let expected_asset_definition_ids = [time_id.clone(), space_id.clone()].into_iter().collect();

    assert_eq!(asset_definition_ids, expected_asset_definition_ids);

    // do the same as above, but utilizing server-side projections
    let asset_definition_ids = host
        .query(FindAssetsDefinitions)
        .filter_with(|asset_definition| asset_definition.id.domain.eq(domain_id.clone()))
        .select_with(|asset_definition| asset_definition.id)
        .execute()
        .dbg_unwrap()
        .map(|v| v.dbg_unwrap())
        .collect::<BTreeSet<_>>();

    assert_eq!(asset_definition_ids, expected_asset_definition_ids);

    // do the same as above, but passing the asset definition id as a 2-tuple
    let asset_definition_ids = host
        .query(FindAssetsDefinitions)
        .filter_with(|asset_definition| asset_definition.id.domain.eq(domain_id))
        .select_with(|asset_definition| (asset_definition.id.domain, asset_definition.id.name))
        .execute()
        .dbg_unwrap()
        .map(|v| v.dbg_unwrap())
        .map(|(domain, name)| AssetDefinitionId::new(domain, name))
        .collect::<BTreeSet<_>>();

    assert_eq!(asset_definition_ids, expected_asset_definition_ids);
}
