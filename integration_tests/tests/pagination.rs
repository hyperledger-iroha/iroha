//! Pagination behaviour integration tests for query iterators.
use eyre::Result;
use integration_tests::sandbox;
use iroha::{
    blocking::Client,
    collections::{Collection, ListQuery, field},
    data_model::{asset::AssetDefinition, isi::InstructionBox, prelude::*},
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::name::Name;
use iroha_test_network::*;
#[test]
fn pagination_behaves() -> Result<()> {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new().with_block_cadence(std::time::Duration::from_secs(2)),
        stringify!(pagination_behaves),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    register_assets(&client)?;
    let collection = Collection::AssetDefinitions;
    let fixture_ids = pagination_asset_definitions()
        .into_iter()
        .map(|(id, _)| id.to_string())
        .collect::<Vec<_>>();
    let bounded = ListQuery::new()
        .filter(field("id").is_in(fixture_ids))
        .limit(7);
    let page = client.list_page(&collection, &bounded)?;
    assert_eq!(page.items.len(), 7);
    assert!(
        page.next_cursor.is_some(),
        "ten fixtures require another bounded page"
    );
    assert_eq!(
        page.total, None,
        "default pages do not scan for exact totals"
    );

    // Restrict the fixture to the seven selected identities, then ask the server
    // for three-row pages. The lazy SDK iterator retains only its current batch.
    let selected = page
        .items
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let query = ListQuery::new()
        .filter(field("id").is_in(selected.clone()))
        .limit(3);
    let mut iter = client.list(collection.clone(), query.clone()).peekable();
    assert_eq!(iter.size_hint(), (0, None));
    assert!(iter.peek().is_some());
    assert_eq!(iter.size_hint(), (3, None));
    let mut observed = Vec::new();
    for _ in 0..4 {
        observed.push(iter.next().unwrap()?);
    }
    assert_eq!(iter.size_hint(), (2, None));
    let remaining = iter.collect::<iroha::Result<Vec<_>>>()?;
    assert_eq!(remaining.len(), 3);
    observed.extend(remaining);
    assert_eq!(
        observed
            .iter()
            .map(|row| row["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        selected.iter().map(String::as_str).collect::<Vec<_>>()
    );

    let first = client.list_page(&collection, &query)?;
    assert_eq!(first.items.len(), 3);
    assert_eq!(first.total, None);
    let next = query
        .next_page(&first)
        .expect("seven matches have another page");
    let second = client.list_page(&collection, &next)?;
    assert_eq!(second.items.len(), 3);
    assert!(second.items.iter().all(|row| !first.items.contains(row)));
    let last = client.list_page(
        &collection,
        &query.next_page(&second).expect("one final row"),
    )?;
    assert_eq!(last.items.len(), 1);
    assert!(last.next_cursor.is_none());
    Ok(())
}
fn register_assets(client: &Client) -> Result<()> {
    const MAX_INSTRUCTIONS_PER_TX: usize = 5;
    let register: Vec<InstructionBox> = pagination_asset_definitions()
        .into_iter()
        .map(|(asset_definition_id, name)| {
            Register::asset_definition({
                let __asset_definition_id = asset_definition_id;
                AssetDefinition::numeric(
                    __asset_definition_id.clone(),
                    name,
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    Some(DomainId::try_new("wonderland", "universal").expect("fixture domain")),
                )
            })
            .into()
        })
        .collect();
    for chunk in register.chunks(MAX_INSTRUCTIONS_PER_TX) {
        client.submit_all::<InstructionBox>(
            chunk.iter().cloned(),
            iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
    }
    Ok(())
}
fn pagination_asset_definitions() -> Vec<(AssetDefinitionId, String)> {
    ('a'..='j')
        .map(|c| c.to_string())
        .map(|name| {
            let id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").expect("wonderland domain is valid"),
                name.parse::<Name>().expect("single-letter name is valid"),
            );
            (id, name)
        })
        .collect()
}
#[test]
fn pagination_asset_definition_ids_are_canonical_base58_literals() {
    let definitions = pagination_asset_definitions();
    assert_eq!(definitions.len(), 10);
    for (id, _) in definitions {
        let literal = id.to_string();
        assert!(!literal.contains(':'));
        assert_eq!(literal.parse::<AssetDefinitionId>().expect("canonical"), id);
    }
}
