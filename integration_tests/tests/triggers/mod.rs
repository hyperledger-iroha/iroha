//! Trigger lifecycle regression tests.
use eyre::Result;
use iroha::{
    blocking::Client,
    data_model::{
        asset::AssetId,
        prelude::{FindAssetById, Quantity},
    },
};
mod by_call_trigger;
mod data_trigger;
mod execution_log;
mod orphans;
mod time_trigger;
mod trigger_rollback;
fn get_asset_value(client: &Client, asset_id: &AssetId) -> Result<Quantity> {
    let asset = client
        .client()
        .query_single(FindAssetById::new(asset_id.clone()))?;
    Ok(asset.value().clone())
}
