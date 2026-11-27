//! Actual physical original journals projected to selected virtual keyed leaves.
//!
//! These table-level fixtures use World directly. Complete State cutover from a supported
//! native-keyed predecessor is exercised separately by State carrier transition tests.

use super::*;
use crate::state::{World, WorldBlock, generic_carrier_storage::CarrierFixtureStorage};
use iroha_data_model::runtime::{
    carrier::{CarrierStorageKeyV1, CarrierValueV1, encode_carrier_key_v1, encode_carrier_v1},
    definition::{
        CarrierNodeV1, CarrierSchemaV1, DefinitionId, ProjectionV1, SchemaV1, StorageDescriptorV1,
        StorageShapeV1,
    },
};
fn schema() -> CarrierSchemaV1 {
    CarrierSchemaV1 {
        nodes: vec![CarrierNodeV1::U16],
    }
}
fn value(number: u16) -> Vec<u8> {
    encode_carrier_v1(&schema(), &CarrierValueV1::U16(number)).unwrap()
}
fn key(storage: DefinitionId, number: u16) -> CarrierStorageKeyV1 {
    CarrierStorageKeyV1 {
        storage,
        key: encode_carrier_key_v1(&schema(), &CarrierValueV1::U16(number)).unwrap(),
    }
}

fn fixture() -> (World, SelectedStateSchema, [DefinitionId; 2]) {
    let world = World::new();
    let catalog = crate::runtime::release_catalog().unwrap();
    let profile = catalog.generic_carriers.unwrap();
    let mut ids = [DefinitionId([0; 32]); 2];
    {
        let mut block = world.block();
        for (index, namespace) in ["app.z_rows", "app.a_rows"].into_iter().enumerate() {
            let descriptor = DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
                namespace: namespace.into(),
                shape: StorageShapeV1::Table {
                    key: SchemaV1::Carrier(schema()),
                },
                value: SchemaV1::Carrier(schema()),
                initial: None,
                invariants: [index as u8; 32],
                projection: ProjectionV1::BareNorito,
            });
            ids[index] = descriptor.id().unwrap();
            block.runtime_definitions.insert(ids[index], descriptor);
        }
        block.runtime_definitions.insert(
            profile.backing,
            catalog.definitions[&profile.backing].clone(),
        );
        let selection = block.runtime_selection.get_mut();
        selection.kernel = profile.kernel;
        selection.commitment_profile = profile.commitment;
        selection.installed_storage = vec![profile.backing, ids[0], ids[1]];
        selection.installed_storage.sort();
        // The second virtual table stays empty; its selected schema still needs an owner.
        block
            .runtime_generic_values
            .insert(key(ids[0], 1), value(11));
        block
            .runtime_generic_values
            .insert(key(ids[0], 256), value(12));
        block.commit();
    }
    let selected = {
        let view = world.view();
        let published =
            crate::runtime::change_set::PublishedDefinitions(view.runtime_definitions());
        SelectedStateSchema::resolve_carrier(catalog, view.runtime_selection(), &published).unwrap()
    };
    (world, selected, ids)
}

fn capture(block: &WorldBlock<'_>, world: &World, intent: CaptureIntent) -> Collector {
    let mut output = Collector::new(
        world.runtime_generic_values.allocation_budget().clone(),
        CaptureLimits {
            max_fields: usize::MAX,
            max_rows: 100,
            max_payload_bytes: 1 << 20,
            max_total_payload_bytes: 4 << 20,
            max_source_visits: 100,
        },
        intent,
    )
    .unwrap();
    output.executing = true;
    output
        .storage(
            STORAGE_OWNER,
            &block.runtime_generic_values,
            &world.runtime_generic_values,
            block.parameters.mode(),
            bare,
        )
        .unwrap();
    output
}

#[test]
fn actual_physical_rows_commit_only_virtual_names_bare_keys_and_bare_values() {
    let (world, selected, _) = fixture();
    let schema_pool = AllocationBudget::new(1 << 20);
    {
        let view = world.view();
        let published =
            crate::runtime::change_set::PublishedDefinitions(view.runtime_definitions());
        let catalog = crate::runtime::release_catalog().unwrap();
        let refused = SelectedStateSchema::resolve_carrier_funded(
            catalog,
            view.runtime_selection(),
            &published,
            &AllocationBudget::new(0),
        );
        assert!(
            matches!(refused, Err(crate::execution_attempt::ExecutionAttemptError::Deferred(original)) if original.allocation_refusal().is_some())
        );
        let owned = SelectedStateSchema::resolve_carrier_funded(
            catalog,
            view.runtime_selection(),
            &published,
            &schema_pool,
        )
        .unwrap();
        assert_eq!(owned.digest, selected.digest);
        assert!(schema_pool.reserved_bytes() > 0);
        let same_original = owned.clone();
        drop(owned);
        assert!(schema_pool.reserved_bytes() > 0);
        drop(same_original);
        assert_eq!(schema_pool.reserved_bytes(), 0);
    }
    let block = world.block();
    let output = capture(&block, &world, CaptureIntent::Complete);
    let tree = output
        .build_carrier(
            &selected,
            world.runtime_generic_values.allocation_budget(),
            &block,
        )
        .unwrap();
    assert!(selected.schema.table(STORAGE_OWNER).is_none());
    assert_eq!(selected.carrier_tables, Some(2));
    for (number, expected) in [(1_u16, 11_u16), (256, 12_u16)] {
        let position = StateKeyV1 {
            table: "app.z_rows".into(),
            key: number.to_le_bytes().to_vec(),
        };
        assert_eq!(tree.get(&position), Some(expected.to_le_bytes().as_slice()));
    }
    assert_eq!(
        tree.get(&StateKeyV1 {
            table: STORAGE_OWNER.into(),
            key: vec![]
        }),
        None
    );
    let refusal = output
        .build_carrier(&selected, &AllocationBudget::new(0), &block)
        .err()
        .expect("original pool refusal");
    assert!(matches!(
        refusal,
        KeyedCaptureError::Tree(KeyedStateError::Admission(_))
    ));
}

#[test]
fn original_insert_update_delete_and_absent_undo_match_a_complete_rebuild() {
    let (world, selected, ids) = fixture();
    let baseline = {
        let block = world.block();
        let output = capture(&block, &world, CaptureIntent::Complete);
        output
            .build_carrier(
                &selected,
                world.runtime_generic_values.allocation_budget(),
                &block,
            )
            .unwrap()
    };
    let mut block = world.block();
    block
        .runtime_generic_values
        .insert(key(ids[0], 1), value(21));
    block.runtime_generic_values.remove(key(ids[0], 256));
    block.runtime_generic_values.remove(key(ids[0], 999));
    block
        .runtime_generic_values
        .insert(key(ids[1], 256), value(30));
    let delta = capture(&block, &world, CaptureIntent::Changes);
    let next = delta.advance_carrier(&selected, &baseline, &block).unwrap();
    let cold = capture(&block, &world, CaptureIntent::Complete)
        .build_carrier(
            &selected,
            world.runtime_generic_values.allocation_budget(),
            &block,
        )
        .unwrap();
    assert_eq!(next.root(), cold.root());
    assert_eq!(
        next.get(&StateKeyV1 {
            table: "app.a_rows".into(),
            key: vec![0, 1]
        }),
        Some([30, 0].as_slice())
    );
    assert!(
        delta
            .advance_carrier(
                &selected,
                &KeyedStateTreeV1::new(
                    selected.digest,
                    world.runtime_generic_values.allocation_budget()
                ),
                &block
            )
            .is_err()
    );
}

#[test]
fn physical_orphans_and_malformed_records_cannot_enter_a_keyed_cut() {
    let (world, selected, ids) = fixture();
    let mut block = world.block();
    block
        .runtime_generic_values
        .insert(key(DefinitionId([0xff; 32]), 1), value(1));
    let output = capture(&block, &world, CaptureIntent::Complete);
    assert!(
        output
            .build_carrier(
                &selected,
                world.runtime_generic_values.allocation_budget(),
                &block
            )
            .is_err()
    );
    drop(output);
    block
        .runtime_generic_values
        .remove(key(DefinitionId([0xff; 32]), 1));
    block.runtime_generic_values.insert(key(ids[0], 1), vec![0]);
    let output = capture(&block, &world, CaptureIntent::Complete);
    assert!(matches!(
        output.build_carrier(
            &selected,
            world.runtime_generic_values.allocation_budget(),
            &block
        ),
        Err(KeyedCaptureError::Codec(_))
    ));
}
