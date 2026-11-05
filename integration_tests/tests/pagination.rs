//! Pagination behaviour integration tests for bounded, authenticated collections.
use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::{
    blocking::Client,
    collections::{Collection, ListQuery},
    data_model::{asset::AssetDefinition, isi::InstructionBox, prelude::*},
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::name::Name;
use iroha_test_network::*;
use std::collections::BTreeSet;
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
    // Native limits bound each page. Compare the seven-row page with the same
    // seven-row window streamed through three-row pages.
    let collection = Collection::ExplorerAssetDefinitions;
    let seven = client.list_page(&collection, &ListQuery::new().limit(7))?;
    assert_eq!(seven.items.len(), 7);
    assert_eq!(seven.total, None);
    assert!(
        seven.has_more(),
        "ten registered fixtures require continuation"
    );
    let seven_ids = asset_definition_ids(&seven.items)?;
    assert!(seven_ids.windows(2).all(|pair| pair[0] < pair[1]));

    let query = ListQuery::new().limit(3);
    let first = client.list_page(&collection, &query)?;
    assert_eq!(first.items.len(), 3);
    assert_eq!(first.total, None);
    assert!(first.has_more());
    assert_eq!(asset_definition_ids(&first.items)?, seven_ids[..3]);

    // The SDK iterator fetches lazily; the page response above tests its first
    // batch directly. After four rows, two rows of its second page stay buffered.
    let mut iter = client.list(collection.clone(), query.clone());
    assert_eq!(iter.size_hint(), (0, None));
    let mut window = Vec::new();
    for _ in 0..4 {
        window.push(iter.next().expect("row within seven-row window")?);
    }
    assert_eq!(iter.size_hint(), (2, None));
    assert_eq!(iter.total(), None);
    let remaining = iter.take(3).collect::<Result<Vec<_>, _>>()?;
    assert_eq!(remaining.len(), 3);
    window.extend(remaining);
    assert_eq!(asset_definition_ids(&window)?, seven_ids);

    // Walk the actual server continuations independently of the local window.
    // A missing page, duplicate row, or repeated cursor must fail explicitly.
    let mut page = first;
    let mut query = query;
    let mut all_ids = Vec::new();
    let mut cursors = BTreeSet::new();
    let mut completed = false;
    for page_index in 0..64 {
        assert!(page.items.len() <= 3);
        if page_index < 2 {
            assert_eq!(
                page.items.len(),
                3,
                "the first two native pages must be full"
            );
            assert!(page.has_more());
        }
        assert_eq!(page.total, None);
        for id in asset_definition_ids(&page.items)? {
            assert!(all_ids.last().is_none_or(|previous| previous < &id));
            all_ids.push(id);
            assert!(
                all_ids.len() <= 64,
                "fixture exceeds bounded collection size"
            );
        }
        let Some(next) = query.next_page(&page) else {
            completed = true;
            break;
        };
        assert!(cursors.insert(page.next_cursor.expect("continuation cursor")));
        query = next;
        page = client.list_page(&collection, &query)?;
    }
    assert!(
        completed,
        "collection did not finish within 64 bounded pages"
    );
    assert_eq!(&all_ids[..7], seven_ids.as_slice());
    for (expected, _) in pagination_asset_definitions() {
        assert!(
            all_ids.binary_search(&expected).is_ok(),
            "registered fixture missing"
        );
    }
    Ok(())
}
fn asset_definition_ids(rows: &[norito::json::Value]) -> Result<Vec<AssetDefinitionId>> {
    rows.iter()
        .map(|row| {
            let id = row
                .get("id")
                .ok_or_else(|| eyre!("asset-definition row omitted its ID"))?;
            norito::json::from_value(id.clone()).map_err(Into::into)
        })
        .collect()
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
                    Some(
                        DomainId::try_new("wonderland", "universal")
                            .expect("fixture owning domain"),
                    ),
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
