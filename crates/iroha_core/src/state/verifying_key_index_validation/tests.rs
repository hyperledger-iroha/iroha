//! Exact physical row and variable-key work, without allocation in the shared relation.

use super::test_support::*;
use super::*;
use crate::test_allocations::allocations_during;
use mv::storage::StorageReadOnly as _;

fn check(world: &crate::state::World, limit: u64) -> Result<(), Error> {
    let rows = world
        .verifying_keys
        .try_committed_view_nonblocking()
        .unwrap();
    let index = world
        .verifying_keys_by_circuit
        .try_committed_view_nonblocking()
        .unwrap();
    let mut result = None;
    assert_eq!(
        allocations_during(|| {
            result = Some(validate(&rows, &index, &mut Work::bounded(limit)));
        }),
        0
    );
    result.unwrap()
}

#[test]
fn exact_both_image_work_pays_masked_rows_tombstones_and_variable_strings() {
    let mut world = Box::new(crate::state::World::default());
    let key = VerifyingKeyId::new("b", "n");
    let mut value = record();
    value.circuit_id = "c".into();
    world.verifying_keys.insert(key.clone(), value.clone());
    world
        .verifying_keys_by_circuit
        .insert(("c".into(), 1), key.clone());
    // Per image: two counting rows, two membership rows, 10 circuit/version
    // bytes and four id bytes. Neither image initially has undo entries.
    assert_eq!(check(&world, 35), Err(Error::WorkLimit));
    assert_eq!(check(&world, 36), Ok(()));
    {
        let mut rows = world.verifying_keys.block();
        rows.remove(VerifyingKeyId::new("z", "n"));
        rows.commit();
    }
    // Two predecessor source traversals each pay one physical tombstone and
    // four key-comparison bytes, even though the absent preimage has no row.
    assert_eq!(check(&world, 45), Err(Error::WorkLimit));
    assert_eq!(check(&world, 46), Ok(()));
    {
        let mut rows = world.verifying_keys.block();
        rows.insert(key.clone(), value);
        rows.remove(VerifyingKeyId::new("z", "n"));
        rows.commit();
    }
    // Masked current key plus its original undo value are both paid. Each
    // predecessor pass now adds six units over the no-undo baseline.
    assert_eq!(check(&world, 47), Err(Error::WorkLimit));
    assert_eq!(check(&world, 48), Ok(()));
    {
        let mut index = world.verifying_keys_by_circuit.block();
        index.insert(("c".into(), 1), key);
        index.remove(("z".into(), 2));
        index.commit();
    }
    // Two index passes: two extra physical rows + ten comparison bytes each.
    assert_eq!(check(&world, 71), Err(Error::WorkLimit));
    assert_eq!(check(&world, 72), Ok(()));
}

#[test]
fn all_comparison_bytes_are_admitted_before_inspecting_long_ids_or_circuits() {
    let mut world = world();
    let mut value = record();
    value.circuit_id = "証明".repeat(4096);
    let key = VerifyingKeyId::new("very/long/backend".repeat(2048), "long-name".repeat(4096));
    world.verifying_keys = mv::storage::Storage::from_iter([(key.clone(), value.clone())]);
    world.verifying_keys_by_circuit =
        mv::storage::Storage::from_iter([((value.circuit_id.clone(), value.version), key.clone())]);
    let expected =
        2 * (4 + 2 * value.circuit_id.len() + 8 + 2 * (key.backend.len() + key.name.len())) as u64;
    assert_eq!(check(&world, expected - 1), Err(Error::WorkLimit));
    assert_eq!(check(&world, expected), Ok(()));
    let rows = world
        .verifying_keys
        .try_committed_view_nonblocking()
        .unwrap();
    let index = world
        .verifying_keys_by_circuit
        .try_committed_view_nonblocking()
        .unwrap();
    assert_eq!(validate(&rows, &index, &mut Work::startup()), Ok(()));
}

#[test]
fn merged_source_images_match_original_history_for_insert_update_remove_and_touch() {
    let mut world = world();
    {
        let mut block = world.block();
        let mut changed = record();
        changed.version = 2;
        block.verifying_keys.insert(id(), changed);
        block
            .verifying_keys_by_circuit
            .remove(("circuit".into(), 1));
        block
            .verifying_keys_by_circuit
            .insert(("circuit".into(), 2), id());
        let extra = VerifyingKeyId::new("other", "key");
        let mut other = record();
        other.circuit_id = "other".into();
        block.verifying_keys.insert(extra.clone(), other);
        block
            .verifying_keys_by_circuit
            .insert(("other".into(), 1), extra);
        block.commit();
    }
    assert_eq!(check(&world, 100_000), Ok(()));
    let expected = {
        let history = world.verifying_keys.history();
        let now = history
            .current()
            .iter()
            .map(|(id, value)| (id.clone(), value.clone()))
            .collect::<Vec<_>>();
        let mut prior = history
            .iter_before_block()
            .map(|(id, value)| (id.clone(), value.clone()))
            .collect::<Vec<_>>();
        prior.sort();
        [now, prior]
    };
    let retained = world
        .verifying_keys
        .try_committed_view_nonblocking()
        .unwrap();
    for (image, expected) in [Image::Current, Image::Predecessor]
        .into_iter()
        .zip(expected)
    {
        let mut actual = Vec::new();
        visit(&retained, image, &mut Work::startup(), |id, value, _| {
            actual.push((id.clone(), value.clone()));
            Ok(())
        })
        .unwrap();
        assert_eq!(actual, expected);
    }
    drop(retained);
    world.block_and_revert().commit();
    assert_eq!(check(&world, 100_000), Ok(()));
}
