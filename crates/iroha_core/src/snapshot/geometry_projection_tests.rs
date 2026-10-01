//! Exact current/predecessor lane geometry from the serialized MV runtime envelope.

use super::*;

fn runtime_at(activation_height: u64, seed: u8) -> SnapshotNexusRuntime {
    let nexus = iroha_config::parameters::actual::Nexus::default();
    let incarnation = Hash::new([seed]);
    let incarnations = BTreeMap::from([(LaneId::SINGLE, incarnation)]);
    let activations = BTreeMap::from([(LaneId::SINGLE, activation_height)]);
    let lineage = BTreeMap::from([(
        LaneId::SINGLE,
        LaneIncarnationLineage {
            generation: activation_height,
            incarnation,
            activation_height,
        },
    )]);
    SnapshotNexusRuntime::from_nexus(&nexus, &incarnations, &activations, &lineage)
}

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"snapshot-geometry-images",
    )))
}

fn decoded(cell: &Cell<SnapshotNexusRuntime>) -> Cell<SnapshotNexusRuntime> {
    let mut encoded = String::from("{\"revert\":");
    json::JsonSerialize::json_serialize(cell.predecessor_view().get(), &mut encoded);
    encoded.push_str(",\"blocks\":");
    json::JsonSerialize::json_serialize(cell.view().get(), &mut encoded);
    encoded.push('}');
    json::from_str(&encoded).expect("decode the same immutable snapshot MV envelope")
}

#[test]
fn physical_retirement_projects_exact_storage_from_paired_snapshot_parameter_images() {
    use iroha_data_model::nexus::{
        DataSpaceCatalog, DataSpaceMetadata, LaneConfig, LaneLifecyclePlan, NexusRuntimeCatalogV1,
        RuntimeDataSpaceRetirementRecordV1, RuntimeDataSpaceRetirementV1, RuntimeLaneRetirementV1,
    };
    use iroha_data_model::parameter::{Parameter, Parameters};
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    let retired_lane = LaneConfig {
        id: LaneId::new(7),
        dataspace_id: iroha_model_base::topology::DataSpaceId::new(7),
        alias: "is".into(),
        ..Default::default()
    };
    nexus.lane_catalog = LaneCatalog::new(
        NonZeroU32::new(8).unwrap(),
        vec![LaneConfig::default(), retired_lane.clone()],
    )
    .unwrap();
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: retired_lane.dataspace_id,
            alias: "is".into(),
            fault_tolerance: 1,
            description: None,
        },
    ])
    .unwrap();
    let incarnations = BTreeMap::from([
        (LaneId::SINGLE, Hash::new(b"primary")),
        (retired_lane.id, Hash::new(b"retained IS storage")),
    ]);
    let activation_heights = BTreeMap::from([(LaneId::SINGLE, 0), (retired_lane.id, 3)]);
    let lineage = incarnations
        .iter()
        .map(|(&id, &incarnation)| {
            (
                id,
                LaneIncarnationLineage {
                    generation: 0,
                    incarnation,
                    activation_height: activation_heights[&id],
                },
            )
        })
        .collect();
    let runtime = Cell::new(SnapshotNexusRuntime::from_nexus(
        &nexus,
        &incarnations,
        &activation_heights,
        &lineage,
    ));
    let parameters = Cell::new(Parameters::default());
    let owner = iroha_data_model::account::AccountId::new(
        iroha_crypto::KeyPair::random().public_key().clone(),
    );
    let catalog = NexusRuntimeCatalogV1 {
        version: 1,
        baseline_dataspaces_hash: Hash::new(b"original configured dataspaces"),
        baseline_manifests_hash: Hash::new(b"original configured manifests"),
        dataspaces: Vec::new(),
        manifests: Vec::new(),
        retired_dataspaces: vec![RuntimeDataSpaceRetirementRecordV1 {
            retirement: RuntimeDataSpaceRetirementV1 {
                dataspace_id: retired_lane.dataspace_id,
                alias: "is".into(),
                owner,
                expected_ownership_generation: 1,
            },
            retirement_height: 8,
        }],
        retired_lanes: vec![RuntimeLaneRetirementV1 {
            lane: retired_lane.clone(),
            incarnation: incarnations[&retired_lane.id],
            activation_height: 3,
            retirement_height: 8,
        }],
    };
    nexus.lane_catalog = nexus
        .lane_catalog
        .apply_lifecycle(&LaneLifecyclePlan {
            additions: Vec::new(),
            retire: vec![retired_lane.id],
        })
        .unwrap();
    nexus.dataspace_catalog = DataSpaceCatalog::default();
    let mut updated_incarnations = incarnations.clone();
    updated_incarnations.remove(&retired_lane.id);
    let mut updated_activations = activation_heights.clone();
    updated_activations.remove(&retired_lane.id);
    let mut runtime_block = runtime.block();
    *runtime_block.get_mut() = SnapshotNexusRuntime::from_nexus(
        &nexus,
        &updated_incarnations,
        &updated_activations,
        &lineage,
    );
    runtime_block.commit();
    let mut parameters_block = parameters.block();
    parameters_block
        .get_mut()
        .set_parameter(Parameter::Custom(catalog.into_custom_parameter().unwrap()));
    parameters_block.commit();
    let (current, predecessor) =
        snapshot_lane_geometry_images(&decoded(&runtime), &parameters, 8, &network()).unwrap();
    assert_eq!(Some(current.clone()), predecessor);
    assert_eq!(current.1, incarnations);
    assert_eq!(current.2, activation_heights);
    // Omitting the parameter undo must not reinterpret current retirement as the predecessor.
    let missing_undo = Cell::new(parameters.view().get().clone());
    assert!(
        snapshot_lane_geometry_images(&decoded(&runtime), &missing_undo, 8, &network()).is_err()
    );
}

#[test]
fn changed_runtime_uses_actual_serialized_predecessor() {
    let cell = Cell::new(runtime_at(0, 1));
    let mut block = cell.block();
    *block.get_mut() = runtime_at(1, 2);
    block.commit();
    let cell = decoded(&cell);
    let (current, predecessor) = snapshot_lane_geometry_images(
        &cell,
        &Cell::new(iroha_data_model::parameter::Parameters::default()),
        1,
        &network(),
    )
    .unwrap();
    let predecessor = predecessor.unwrap();
    assert_eq!(current.2[&LaneId::SINGLE], 1);
    assert_eq!(predecessor.2[&LaneId::SINGLE], 0);
    assert_ne!(current.1, predecessor.1);
    assert_ne!(current.3, predecessor.3);
}

#[test]
fn unchanged_runtime_uses_same_serialized_value_at_previous_height() {
    let cell = Cell::new(runtime_at(0, 1));
    let mut block = cell.block();
    *block.get_mut() = runtime_at(1, 2);
    block.commit();
    cell.block().commit();
    let cell = decoded(&cell);
    assert!(cell.predecessor_view().get().is_none());
    let (current, predecessor) = snapshot_lane_geometry_images(
        &cell,
        &Cell::new(iroha_data_model::parameter::Parameters::default()),
        2,
        &network(),
    )
    .unwrap();
    assert_eq!(Some(current), predecessor);
}

#[test]
fn absent_undo_does_not_hide_a_change_at_snapshot_height() {
    let cell = decoded(&Cell::new(runtime_at(1, 2)));
    assert!(
        snapshot_lane_geometry_images(
            &cell,
            &Cell::new(iroha_data_model::parameter::Parameters::default()),
            1,
            &network()
        )
        .is_err()
    );
    let cell = decoded(&Cell::new(runtime_at(0, 1)));
    assert!(
        snapshot_lane_geometry_images(
            &cell,
            &Cell::new(iroha_data_model::parameter::Parameters::default()),
            0,
            &network()
        )
        .unwrap()
        .1
        .is_none()
    );
}
