//! Pagination behaviour integration tests for query iterators.

use eyre::Result;
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{asset::AssetDefinition, prelude::*},
};
use iroha_data_model::query::dsl::SelectorTuple;
use iroha_test_network::*;
use nonzero_ext::nonzero;

#[test]
fn pagination_behaves() -> Result<()> {
    // Stored cursors are required to validate multi-batch pagination behavior.
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new()
            .with_pipeline_time(std::time::Duration::from_secs(2))
            .with_config_layer(|layer| {
                layer.write(["pipeline", "query_default_cursor_mode"], "stored");
            }),
        stringify!(pagination_behaves),
    )?
    else {
        return Ok(());
    };
    let client = network.client();

    register_assets(&client)?;

    // limits_should_work
    let vec = client
        .query(FindAssetsDefinitions::new())
        .with_pagination(Pagination::new(Some(nonzero!(7_u64)), 1))
        .execute_all()?;
    assert_eq!(vec.len(), 7);

    // reported_length_should_be_accurate
    let mut iter = client
        .query(FindAssetsDefinitions::new())
        .with_pagination(Pagination::new(Some(nonzero!(7_u64)), 1))
        .with_fetch_size(FetchSize::new(Some(nonzero!(3_u64))))
        .execute()?;
    assert_eq!(iter.len(), 7);
    for _ in 0..4 {
        iter.next().unwrap().unwrap();
    }
    assert_eq!(iter.len(), 3);

    // fetch_size_should_work
    {
        use iroha::data_model::query::{
            QueryBox, QueryOutputBatchBox, QueryWithFilter, QueryWithParams,
            builder::QueryExecutor as _,
            dsl::CompoundPredicate,
            parameters::{FetchSize, QueryParams, Sorting},
        };

        let with_filter: QueryWithFilter<AssetDefinition> =
            QueryWithFilter::new_with_query((), CompoundPredicate::PASS, SelectorTuple::default());
        let qbox: QueryBox<QueryOutputBatchBox> = with_filter.into();
        let query = QueryWithParams::new(
            &qbox,
            QueryParams::new(
                Pagination::new(Some(nonzero!(7_u64)), 1),
                Sorting::default(),
                FetchSize::new(Some(nonzero!(3_u64))),
            ),
        );
        let (first_batch, remaining_items, _continue_cursor) = client.start_query(query)?;

        assert_eq!(first_batch.len(), 3);
        assert_eq!(remaining_items, 4);
    }

    Ok(())
}

fn register_assets(client: &Client) -> Result<()> {
    const MAX_INSTRUCTIONS_PER_TX: usize = 5;

    let register: Vec<_> = ('a'..='j')
        .map(|c| c.to_string())
        .map(|name| (name + "#wonderland").parse().expect("Valid"))
        .map(|asset_definition_id| {
            Register::asset_definition(AssetDefinition::numeric(asset_definition_id))
        })
        .collect();

    for chunk in register.chunks(MAX_INSTRUCTIONS_PER_TX) {
        client.submit_all_blocking(chunk.iter().cloned())?;
    }

    Ok(())
}
