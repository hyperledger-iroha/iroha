//! Carrier cold cutover uses the original native keyed predecessor and complete State image.

use super::*;
use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
use iroha_data_model::runtime::{
    carrier::{CarrierStorageKeyV1, CarrierValueV1, encode_carrier_key_v1, encode_carrier_v1},
    definition::{
        CarrierNodeV1, CarrierSchemaV1, DefinitionDescriptorV1, ProjectionV1, SchemaV1,
        StorageDescriptorV1, StorageShapeV1,
    },
};

#[test]
fn original_native_keyed_parent_drives_complete_carrier_cut_and_refuses_unfunded_schema() {
    let mut world = World::new();
    runtime::keyed_state::install_for_test(&mut world);
    let chain = CertifiedTestChain::start(TestChainConfig::new(world, 1000)).unwrap();
    let state = chain.state();
    let genesis = chain.committed(1);
    let predecessor = state
        .keyed_versions
        .lock()
        .current
        .as_ref()
        .unwrap()
        .clone();
    let original_root = predecessor.published.root();
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(2).unwrap(),
        Some(genesis.block_hash()),
        None,
        2000,
        0,
    );
    let mut block = state.block(header);
    let catalog = runtime::release_catalog().unwrap();
    let profile = catalog.generic_carriers.unwrap();
    let artifacts = catalog.world_storage["world.runtime_generic_artifacts"];
    for id in [
        profile.kernel,
        profile.host,
        profile.backing,
        profile.commitment,
        artifacts,
    ] {
        block
            .world
            .runtime_definitions
            .insert(id, catalog.definitions[&id].clone());
    }
    let schema = CarrierSchemaV1 {
        nodes: vec![CarrierNodeV1::U16],
    };
    let table = DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
        namespace: "app.original_cut".into(),
        shape: StorageShapeV1::Table {
            key: SchemaV1::Carrier(schema.clone()),
        },
        value: SchemaV1::Carrier(schema.clone()),
        initial: None,
        invariants: [19; 32],
        projection: ProjectionV1::BareNorito,
    });
    let table_id = table.id().unwrap();
    block.world.runtime_definitions.insert(table_id, table);
    let previous = block.world.runtime_selection.get().clone();
    let selection = block.world.runtime_selection.get_mut();
    selection.kernel = profile.kernel;
    selection.commitment_profile = profile.commitment;
    selection.active.push(profile.host);
    selection.active.sort_unstable();
    selection
        .installed_storage
        .extend([profile.backing, artifacts, table_id]);
    selection.installed_storage.sort_unstable();
    runtime::keyed_state::validate_transition_with_definitions(
        &previous,
        block.world.runtime_selection.get(),
        &runtime::change_set::PublishedDefinitions(&block.world.runtime_definitions),
    )
    .unwrap();
    block.world.runtime_generic_values.insert(
        CarrierStorageKeyV1 {
            storage: table_id,
            key: encode_carrier_key_v1(&schema, &CarrierValueV1::U16(256)).unwrap(),
        },
        encode_carrier_v1(&schema, &CarrierValueV1::U16(9)).unwrap(),
    );
    let unfunded = schema_for_world(&block.world, &iroha_allocation::AllocationBudget::new(0));
    assert!(matches!(unfunded, Err(CutError::Deferred(_))));
    let fields = block.fields.as_ref().unwrap();
    let execution = fields.keyed_execution().unwrap();
    assert_eq!(execution.parent, KeyedStateRoot::from(original_root));
    assert_ne!(execution.schema.digest, predecessor.schema.digest);
    assert_eq!(execution.schema.carrier_tables, Some(1));
    let cold = capture_keyed_image(fields, &execution.schema, None).unwrap();
    assert_eq!(execution.tree.root(), cold.root());
    assert_eq!(
        execution.tree.get(&StateKeyV1 {
            table: "app.original_cut".into(),
            key: vec![0, 1]
        }),
        Some([9, 0].as_slice())
    );
    assert!(
        execution
            .schema
            .schema
            .table("world.runtime_generic_values")
            .is_none()
    );
    // This is an unpublished component cut: the actual predecessor remains immutable and the
    // dormant profile gains no support or activation authority from the fixture mutation.
    assert_eq!(
        state
            .keyed_versions
            .lock()
            .current
            .as_ref()
            .unwrap()
            .published
            .root(),
        original_root
    );
    drop(block);
    assert_eq!(
        state
            .keyed_versions
            .lock()
            .current
            .as_ref()
            .unwrap()
            .published
            .root(),
        original_root
    );
}
