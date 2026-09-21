//! Actual full-format S file controls; no source or proof qualification.
use super::*;
use crate::testing::TestDirectory;
const PAIR_BYTES: u64 = 66 * 16_400;
fn pair(path: &Path, budget: &mut OrderedStorageSessionBudgetV1) -> OrderedPlaneSpoolSnapshotV1 {
    let mut writer =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(path, [101; 32], budget).unwrap();
    for slot in 0..66 {
        let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
        if slot % 33 == 32 {
            chunk.as_mut_slice_v1()[31] = 1;
            chunk.as_mut_slice_v1()[32..65].copy_from_slice(
                &Point::canonical_generator()
                    .unwrap()
                    .to_non_identity_wire_bytes()
                    .unwrap(),
            );
        }
        writer.write_slot_v1(slot, chunk).unwrap();
    }
    writer.seal_v1().unwrap()
}
fn start(
    path: &Path,
    budget: &mut OrderedStorageSessionBudgetV1,
    proof: &mut RnsNativeProofResourceBudgetV1,
) -> (OrderedPlaneSpoolSnapshotV1, QMaskSFileV1) {
    let original = pair(path, budget);
    let plan = original.q_mask_s_file_plan_v1().unwrap();
    let memory = plan.reserve_memory_v1(proof).unwrap();
    let file = original
        .create_q_mask_s_file_v1(path, plan, memory)
        .unwrap();
    (original, file)
}
fn write_block(mut writer: QMaskSFileV1, block: u64) -> WrittenQMaskSBlockFileV1 {
    for local in 0..8 {
        let slot = block * 8 + local;
        let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
        chunk.as_mut_slice_v1()[..8].copy_from_slice(&slot.to_le_bytes());
        writer.write_slot_v1(slot, chunk).unwrap();
    }
    writer.finish_block_v1().unwrap()
}
fn complete(
    path: &Path,
    budget: &mut OrderedStorageSessionBudgetV1,
    proof: &mut RnsNativeProofResourceBudgetV1,
) -> (OrderedPlaneSpoolSnapshotV1, WrittenQMaskSBlockFileV1) {
    let (pair, mut writer) = start(path, budget, proof);
    for block in 0..1600 {
        let written = write_block(writer, block);
        if block == 1599 {
            return (pair, written);
        }
        writer = written.resume_next_block_v1().unwrap();
    }
    unreachable!()
}
#[test]
#[cfg(unix)]
fn qmask_s_stream_storage_full_canonical_file_seals_and_replays_all_original_slots_once() {
    let dir = TestDirectory::new("qmask-s-full-format");
    let mut storage = OrderedStorageSessionBudgetV1::new_v1();
    let mut proof = RnsNativeProofResourceBudgetV1::default();
    let (original, file) = complete(dir.path(), &mut storage, &mut proof);
    let binding = file.require_block_binding_v1().unwrap();
    assert_eq!(file.block_ordinal_v1().unwrap(), 1599);
    assert_eq!(
        storage.test_usage_words_v1(),
        [
            PAIR_BYTES + S_FILE_BYTES_V1,
            PAIR_BYTES + S_FILE_BYTES_V1,
            S_FILE_BYTES_V1,
            2 * PAIR_BYTES + S_FILE_BYTES_V1
        ]
    );
    let mut file = file.seal_v1().unwrap();
    file.require_original_v1(binding, &proof).unwrap();
    assert_eq!(storage.test_usage_words_v1()[2], 0);
    let other = RnsNativeProofResourceBudgetV1::default();
    assert_eq!(
        file.require_original_v1(binding, &other),
        Err(OrderedSnapshotErrorV1::Context)
    );
    let digest = file.digest;
    file.digest[0] ^= 1;
    assert_eq!(file.validate_v1(), Err(OrderedSnapshotErrorV1::Context));
    file.digest = digest;
    for block in 0..1600 {
        let mut read = file.begin_block_read_v1(block).unwrap();
        for local in 0..8 {
            let chunk = read.read_next_slot_v1().unwrap();
            assert_eq!(chunk.len_v1(), S_SLOT_BYTES_V1);
            assert_eq!(
                &chunk.as_slice_v1()[..8],
                &((block * 8 + local) as u64).to_le_bytes()
            );
            assert!(chunk.as_slice_v1()[8..].iter().all(|b| *b == 0));
        }
        read.finish_v1().unwrap();
    }
    file.require_replayed_v1().unwrap();
    assert_eq!(file.digest, digest);
    assert_eq!(
        storage.test_usage_words_v1()[3],
        2 * PAIR_BYTES + 3 * S_FILE_BYTES_V1
    );
    assert!(matches!(
        file.begin_block_read_v1(0),
        Err(OrderedSnapshotErrorV1::Order)
    ));
    assert!(file.live.is_none());
    assert_eq!(storage.test_usage_words_v1()[0], PAIR_BYTES);
    assert_eq!(proof.live_bytes().unwrap(), 0);
    drop(original);
    assert_eq!(storage.test_usage_words_v1()[0], 0);
}
#[test]
#[cfg(unix)]
fn qmask_s_stream_storage_capacity_retains_real_snapshot_until_competing_pair_drops() {
    let dir = TestDirectory::new("qmask-s-read-capacity");
    let mut storage = OrderedStorageSessionBudgetV1::with_test_limits_v1(
        S_FILE_BYTES_V1 + 2 * PAIR_BYTES,
        2 * S_FILE_BYTES_V1 + 4 * PAIR_BYTES,
    );
    let mut proof = RnsNativeProofResourceBudgetV1::default();
    let (original, file) = complete(dir.path(), &mut storage, &mut proof);
    let mut file = file.seal_v1().unwrap();
    let competitor =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(dir.path(), [102; 32], &mut storage)
            .unwrap();
    let digest = file.digest;
    let before = storage.test_usage_words_v1();
    let memory = proof.live_bytes().unwrap();
    assert!(matches!(
        file.begin_block_read_v1(0),
        Err(OrderedSnapshotErrorV1::Capacity)
    ));
    assert_eq!(file.digest, digest);
    assert_eq!(file.next_block, 0);
    assert!(file.live.is_some());
    assert_eq!(storage.test_usage_words_v1(), before);
    assert_eq!(proof.live_bytes().unwrap(), memory);
    drop(competitor);
    let mut read = file.begin_block_read_v1(0).unwrap();
    for _ in 0..8 {
        drop(read.read_next_slot_v1().unwrap());
    }
    read.finish_v1().unwrap();
    assert_eq!(file.next_block, 1);
    assert_eq!(file.digest, digest);
    assert_eq!(
        storage.test_usage_words_v1()[3],
        2 * PAIR_BYTES + 2 * S_FILE_BYTES_V1 + 8 * 16_400
    );
    drop(file);
    assert_eq!(proof.live_bytes().unwrap(), 0);
    drop(original);
    assert_eq!(storage.test_usage_words_v1()[0], 0);
}
#[test]
#[cfg(unix)]
fn qmask_s_stream_storage_missing_actual_records_cannot_be_sealed_by_wrapper_metadata() {
    let dir = TestDirectory::new("qmask-s-no-metadata-seal");
    for fake_complete in [false, true] {
        let mut storage = OrderedStorageSessionBudgetV1::new_v1();
        let mut proof = RnsNativeProofResourceBudgetV1::default();
        let (original, writer) = start(dir.path(), &mut storage, &mut proof);
        let mut file = write_block(writer, 0);
        if fake_complete {
            file.file.next_slot = S_SLOTS_V1;
            file.file.block_end_slot = S_SLOTS_V1;
        }
        assert!(matches!(
            file.seal_v1(),
            Err(OrderedSnapshotErrorV1::Order | OrderedSnapshotErrorV1::Storage)
        ));
        assert_eq!(storage.test_usage_words_v1()[0], PAIR_BYTES);
        assert_eq!(storage.test_usage_words_v1()[2], 0);
        assert_eq!(
            storage.test_usage_words_v1()[3],
            2 * PAIR_BYTES + 8 * 16_400 + if fake_complete { S_FILE_BYTES_V1 } else { 0 }
        );
        assert_eq!(proof.live_bytes().unwrap(), 0);
        drop(original);
    }
}
#[test]
#[cfg(unix)]
fn qmask_s_stream_storage_incomplete_or_context_substituted_read_consumes_actual_file() {
    let dir = TestDirectory::new("qmask-s-read-terminal");
    for substitute in [false, true] {
        let mut storage = OrderedStorageSessionBudgetV1::new_v1();
        let mut proof = RnsNativeProofResourceBudgetV1::default();
        let (original, file) = complete(dir.path(), &mut storage, &mut proof);
        let mut file = file.seal_v1().unwrap();
        if substitute {
            file.context[0] ^= 1;
            file.live.as_mut().unwrap().memory.binding = file.context;
        }
        let mut read = file.begin_block_read_v1(0).unwrap();
        if substitute {
            assert!(matches!(
                read.read_next_slot_v1(),
                Err(OrderedSnapshotErrorV1::Storage)
            ));
        } else {
            drop(read.read_next_slot_v1().unwrap());
        }
        drop(read);
        assert!(file.live.is_none());
        assert_eq!(storage.test_usage_words_v1()[0], PAIR_BYTES);
        assert_eq!(
            storage.test_usage_words_v1()[3],
            2 * PAIR_BYTES + 2 * S_FILE_BYTES_V1 + 8 * 16_400
        );
        assert_eq!(proof.live_bytes().unwrap(), 0);
        assert!(matches!(
            file.begin_block_read_v1(0),
            Err(OrderedSnapshotErrorV1::Poisoned)
        ));
        drop(original);
    }
}
