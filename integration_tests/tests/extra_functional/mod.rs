//! Extra-functional soak and churn scenarios.
mod connected_peers;
mod genesis;
mod multiple_blocks_created;
mod normal;
mod offline_peers;
mod restart_peer;
mod seven_peer_consistency;
mod unregister_peer;
mod unstable_network;

fn find_asset(
    client: &iroha::blocking::Client,
    asset_id: &iroha::data_model::asset::AssetId,
) -> eyre::Result<Option<iroha::data_model::asset::Asset>> {
    use iroha::data_model::{
        ValidationFail,
        query::{
            asset::prelude::FindAssetById,
            error::{FindError, QueryExecutionFail},
        },
    };
    match client
        .client()
        .query_single(FindAssetById::new(asset_id.clone()))
    {
        Ok(asset) => Ok(Some(asset)),
        Err(iroha::query::QueryError::Validation(ValidationFail::QueryFailed(
            QueryExecutionFail::Find(FindError::Asset(missing)),
        ))) if missing.as_ref() == asset_id => Ok(None),
        Err(error) => Err(error.into()),
    }
}
