//! Current/predecessor restore and commitment coverage for native game partitions.
//!
//! These tests exercise persisted state custody, not foreign-root finality.

use super::*;
use iroha_data_model::game_partition::{
    GAME_PARTITION_MAX_WORLDS, GamePartitionStateV1, GameUnitV1, GameUnitWatermarkV1, game_unit_id,
};

fn record(marker: u8) -> GamePartitionStateV1 {
    let pair =
        iroha_crypto::KeyPair::try_from_seed(vec![0x65; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap();
    let owner = AccountId::new(pair.public_key().clone());
    let world_id = Hash::new([marker]);
    let unit_id = game_unit_id(world_id, [1; 32], 7);
    GamePartitionStateV1 {
        world_id,
        local_instance: [1; 32],
        kernel_hash: ivm::game_unit_step::kernel_hash(),
        controller: owner.clone(),
        peers: Vec::new(),
        units: vec![GameUnitV1 {
            unit_id,
            origin: [1; 32],
            owner,
            hp: 90,
            x: -7,
            sequence: 0,
            work_remaining: 17,
        }],
        exports: Vec::new(),
        imported: Vec::new(),
        known_units: vec![GameUnitWatermarkV1 {
            unit_id,
            sequence: 0,
        }],
    }
}

fn write(world: &World, key: Hash, partition: GamePartitionStateV1) {
    let mut block = world.block();
    block.game_partitions.insert(key, partition);
    block.commit();
}

fn restore_map(map: SnapshotJsonMap<'_>) -> Result<World, json::Error> {
    let ivm = IVM::new(0);
    let seed = IvmSeed {
        ivm: &ivm,
        _marker: PhantomData,
    };
    parse_world(
        &iroha_allocation::AllocationBudget::new(
            iroha_config::parameters::defaults::pipeline::IVM_EXECUTION_MAX_BYTES,
        ),
        map,
        &seed,
    )
    .map_err(crate::state::deserialize::snapshot_format_error_for_test)
}

fn restore(world: &World) -> Result<World, json::Error> {
    let encoded = json::to_json(world).unwrap();
    restore_map(SnapshotJsonMap::parse(&encoded, "world")?)
}

#[test]
fn game_partition_snapshot_preserves_current_and_exact_predecessor() {
    let world = World::default();
    let first = record(1);
    write(&world, first.world_id, first.clone());
    let mut next = first.clone();
    next.units[0].x += 1;
    next.units[0].work_remaining -= 1;
    write(&world, next.world_id, next.clone());
    let before = json::to_json(&world.game_partitions).unwrap();
    validate_game_partition_persistence(&world).unwrap();
    assert_eq!(json::to_json(&world.game_partitions).unwrap(), before);
    let restored = restore(&world).unwrap();
    assert_eq!(json::to_json(&restored.game_partitions).unwrap(), before);
    assert_eq!(
        restored.game_partitions.view().get(&first.world_id),
        Some(&next)
    );
    let predecessor = restored.game_partitions.block_and_revert();
    assert_eq!(predecessor.get(&first.world_id), Some(&first));
    drop(predecessor);
    assert_eq!(json::to_json(&restored.game_partitions).unwrap(), before);
}

#[test]
fn game_partition_snapshot_field_cannot_be_omitted() {
    let encoded = json::to_json(&World::default()).unwrap();
    let mut map = SnapshotJsonMap::parse(&encoded, "world").unwrap();
    assert!(map.remove("game_partitions").is_some());
    let error = restore_map(map)
        .err()
        .expect("missing authoritative field rejects");
    assert!(error.to_string().contains("game_partitions"));
}

#[test]
fn game_partition_restore_rejects_key_substitution() {
    let world = World::default();
    write(&world, Hash::new(b"wrong-key"), record(2));
    let error = restore(&world).err().expect("key substitution rejects");
    assert!(error.to_string().contains("authenticated world identifier"));
}

#[test]
fn game_partition_restore_rejects_inconsistent_current_watermark() {
    let world = World::default();
    let mut partition = record(3);
    partition.units[0].sequence += 1;
    write(&world, partition.world_id, partition);
    let error = restore(&world)
        .err()
        .expect("current custody mismatch rejects");
    assert!(error.to_string().contains("current game partition"));
}

#[test]
fn game_partition_restore_rejects_inconsistent_retained_predecessor() {
    let world = World::default();
    let valid = record(4);
    let mut invalid = valid.clone();
    invalid.known_units.clear();
    write(&world, invalid.world_id, invalid);
    write(&world, valid.world_id, valid);
    let before = json::to_json(&world.game_partitions).unwrap();
    let error = restore(&world).err().expect("invalid predecessor rejects");
    assert!(error.to_string().contains("predecessor game partition"));
    assert_eq!(json::to_json(&world.game_partitions).unwrap(), before);
}

#[test]
fn game_partition_restore_enforces_world_bound_at_both_cuts() {
    for predecessor_only in [false, true] {
        let world = World::default();
        {
            let mut block = world.block();
            for marker in 0..=GAME_PARTITION_MAX_WORLDS {
                let partition = record(marker as u8);
                block.game_partitions.insert(partition.world_id, partition);
            }
            block.commit();
        }
        if predecessor_only {
            let mut block = world.block();
            block.game_partitions.remove(Hash::new([0_u8]));
            block.commit();
        }
        let error = restore(&world).err().expect("world bound rejects");
        let cut = if predecessor_only {
            "predecessor"
        } else {
            "current"
        };
        assert!(error.to_string().contains(cut));
        assert!(error.to_string().contains("world retention bound"));
    }
}

#[test]
fn game_partition_block_rollback_does_not_persist_unit_mutation() {
    let world = World::default();
    let original = record(8);
    write(&world, original.world_id, original.clone());
    let before = json::to_json(&world.game_partitions).unwrap();
    {
        let mut block = world.block();
        let mut changed = original.clone();
        changed.units[0].hp = 1;
        block.game_partitions.insert(changed.world_id, changed);
        // Deliberate drop: no committed unit update or undo-map consumption.
    }
    assert_eq!(json::to_json(&world.game_partitions).unwrap(), before);
    assert_eq!(
        world.game_partitions.view().get(&original.world_id),
        Some(&original)
    );
}

#[test]
fn game_partition_canonical_projection_binds_every_unit_field() {
    use crate::state::world_projection::WorldStateAccumulator;

    let world = World::default();
    let original = record(9);
    let empty = WorldStateAccumulator::capture(&world.block()).unwrap();
    write(&world, original.world_id, original.clone());
    let present = WorldStateAccumulator::capture(&world.block()).unwrap();
    assert_ne!(empty.baseline_root(), present.baseline_root());
    assert_eq!(present.entries(), empty.entries() + 1);

    for field in 0..7 {
        let mut changed = original.clone();
        match field {
            0 => changed.units[0].hp -= 1,
            1 => changed.units[0].x += 1,
            2 => changed.units[0].work_remaining -= 1,
            3 => {
                changed.units[0].sequence += 1;
                changed.known_units[0].sequence += 1;
            }
            4 => changed.units[0].origin = [7; 32],
            5 => {
                changed.units[0].unit_id = Hash::new(b"another-unit");
                changed.known_units[0].unit_id = changed.units[0].unit_id;
            }
            _ => {
                let other = iroha_crypto::KeyPair::try_from_seed(
                    vec![0x66; 32],
                    iroha_crypto::Algorithm::Ed25519,
                )
                .unwrap();
                changed.units[0].owner = AccountId::new(other.public_key().clone());
            }
        }
        let mut block = world.block();
        block.game_partitions.insert(changed.world_id, changed);
        let projected = WorldStateAccumulator::capture(&block).unwrap();
        assert_ne!(
            projected.baseline_root(),
            present.baseline_root(),
            "field {field}"
        );
        assert_eq!(projected.entries(), present.entries());
        // Drop each candidate: only its exact field difference enters the test.
    }
    assert_eq!(
        WorldStateAccumulator::capture(&world.block())
            .unwrap()
            .baseline_root(),
        present.baseline_root()
    );
}
