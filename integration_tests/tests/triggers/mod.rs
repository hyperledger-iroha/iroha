#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Trigger lifecycle regression tests.

use eyre::{Result, eyre};
use iroha::{
    client::Client,
    data_model::{
        asset::AssetId,
        prelude::{FindAssets, Identifiable, Numeric, QueryBuilderExt},
    },
};

mod by_call_trigger;
mod data_trigger;
mod execution_log;
mod orphans;
mod time_trigger;
mod trigger_rollback;

fn get_asset_value(client: &Client, asset_id: &AssetId) -> Result<Numeric> {
    let assets = client
        .query(FindAssets::new())
        .execute_all()
        .map_err(|err| eyre!(err))?;
    let asset = assets
        .into_iter()
        .find(|asset| asset.id() == asset_id)
        .ok_or_else(|| eyre!("asset {asset_id} not found"))?;

    Ok(asset.value().clone())
}
