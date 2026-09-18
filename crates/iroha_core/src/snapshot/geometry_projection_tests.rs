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
fn changed_runtime_uses_actual_serialized_predecessor() {
    let cell = Cell::new(runtime_at(0, 1));
    let mut block = cell.block();
    *block.get_mut() = runtime_at(1, 2);
    block.commit();
    let cell = decoded(&cell);
    let (current, predecessor) = snapshot_lane_geometry_images(&cell, 1, &network()).unwrap();
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
    let (current, predecessor) = snapshot_lane_geometry_images(&cell, 2, &network()).unwrap();
    assert_eq!(Some(current), predecessor);
}

#[test]
fn absent_undo_does_not_hide_a_change_at_snapshot_height() {
    let cell = decoded(&Cell::new(runtime_at(1, 2)));
    assert!(snapshot_lane_geometry_images(&cell, 1, &network()).is_err());
    let cell = decoded(&Cell::new(runtime_at(0, 1)));
    assert!(
        snapshot_lane_geometry_images(&cell, 0, &network())
            .unwrap()
            .1
            .is_none()
    );
}
