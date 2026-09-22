//! Bounded-stack tiered captures retain complete baselines and exact overlay changes.

use super::*;

#[test]
fn autoscale_catalog_owners_commit_on_default_stack() {
    std::thread::Builder::new()
        .name("autoscale-tiered-stack".into())
        // Pin the ordinary libtest budget even when RUST_MIN_STACK is set.
        .stack_size(2 * 1024 * 1024)
        .spawn(super::autoscale_catalog_caches_cannot_replace_current_or_undo_runtime_owners)
        .expect("spawn with the ordinary stack budget")
        .join()
        .expect("autoscale commits preserve runtime owners within the stack budget");
}

fn assert_cold_values(backend: &TieredStateBackend, mut expected: Vec<Vec<u8>>) {
    let manifest = backend.last_manifest().expect("snapshot manifest");
    assert_eq!(manifest.total_entries, expected.len());
    assert!(manifest.hot_entries.is_empty());
    let mut actual: Vec<_> = manifest
        .cold_entries
        .iter()
        .map(|entry| {
            backend
                .read_cold_payload(manifest.snapshot_index, entry)
                .expect("read retained cold value")
                .expect("every tracked value is cold")
        })
        .collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn tiered_complete_and_incremental_capture_retain_values_on_default_stack() {
    std::thread::Builder::new()
        .name("tiered-payload-stack".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let mut world = World::with(
                [],
                [
                    Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                    Account::new(BOB_ID.clone()).build(&BOB_ID),
                ],
                [],
            );
            world.tx_sequences.insert(ALICE_ID.clone(), 7);
            world.tx_sequences.insert(BOB_ID.clone(), 11);
            let removed = Hash::new(b"removed contract");
            let inserted = Hash::new(b"inserted contract");
            world.contract_code.insert(removed, vec![0xA1]);
            let account_values: Vec<_> = world
                .accounts
                .view()
                .iter()
                .map(|(_, account)| json::to_vec(account).expect("encode account value"))
                .collect();
            let temp = tempfile::tempdir().expect("temporary cold store");
            // A one-byte hot budget sends every populated row to cold storage.
            let mut backend =
                TieredStateBackend::new(true, 0, 1, 0, Some(temp.path().to_path_buf()), None, 0, 0);
            let baseline = world.block().tiered_snapshot_payload_with_scope(true);
            assert_eq!(TieredSnapshotDiff::from(&baseline).entries().len(), 5);
            backend
                .record_world_snapshot_with_payload(&baseline)
                .expect("persist the complete retained baseline");
            let mut expected = account_values.clone();
            expected.extend([
                json::to_vec(&7_u64).expect("encode initial sequence"),
                json::to_vec(&11_u64).expect("encode untouched sequence"),
                json::to_vec(&vec![0xA1_u8]).expect("encode removed contract"),
            ]);
            assert_cold_values(&backend, expected);

            let mut block = world.block();
            block.tx_sequences.insert(ALICE_ID.clone(), 9);
            block.contract_code.remove(removed);
            block.contract_code.insert(inserted, vec![0xB2, 0xB3]);
            let delta = block.tiered_snapshot_payload();
            let actual_keys: BTreeSet<_> = TieredSnapshotDiff::from(&delta)
                .entries()
                .iter()
                .map(ToString::to_string)
                .collect();
            let expected_keys: BTreeSet<_> = [
                TieredKeyHandle::TxSequence(ALICE_ID.clone()),
                TieredKeyHandle::ContractCode(removed),
                TieredKeyHandle::ContractCode(inserted),
            ]
            .iter()
            .map(ToString::to_string)
            .collect();
            assert_eq!(actual_keys, expected_keys);
            // Captured values must survive later mutation and overlay discard;
            // the background snapshot worker has no live World authority.
            block.tx_sequences.insert(ALICE_ID.clone(), 999);
            block.contract_code.insert(inserted, vec![0xCC]);
            drop(block);
            backend
                .record_world_snapshot_with_payload(&delta)
                .expect("persist exact incremental insert, update, and deletion");
            let mut expected = account_values;
            expected.extend([
                json::to_vec(&9_u64).expect("encode captured sequence"),
                json::to_vec(&11_u64).expect("encode untouched sequence"),
                json::to_vec(&vec![0xB2_u8, 0xB3]).expect("encode captured contract"),
            ]);
            assert_cold_values(&backend, expected);
            assert_eq!(world.tx_sequences.view().get(&ALICE_ID), Some(&7));
            assert!(world.contract_code.view().get(&removed).is_some());
            assert!(world.contract_code.view().get(&inserted).is_none());
        })
        .expect("spawn with the ordinary stack budget")
        .join()
        .expect("complete and incremental captures fit the ordinary stack budget");
}
