use iroha::{
    client::Client,
    data_model::{
        asset::AssetId,
        prelude::{FindAssets, Numeric, QueryBuilderExt},
    },
};

mod by_call_trigger;
mod data_trigger;
mod orphans;
// FIXME: rewrite all in async and with shorter timings
mod time_trigger;
mod trigger_rollback;

fn get_asset_value(client: &Client, asset_id: AssetId) -> Numeric {
    let asset = client
        .query(FindAssets::new())
        .filter_with(|asset| asset.id.eq(asset_id))
        .execute_single()
        .unwrap();

    *asset.value()
}
