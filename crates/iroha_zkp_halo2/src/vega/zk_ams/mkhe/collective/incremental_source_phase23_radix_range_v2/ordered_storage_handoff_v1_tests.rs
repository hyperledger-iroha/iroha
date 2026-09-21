//! Source/pair custody on real tiny-file refusal; fixture sources are unqualified.
use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT_V1: AtomicU64 = AtomicU64::new(0);
struct DirectoryV1(PathBuf);
impl DirectoryV1 {
    fn new_v1() -> Self {
        let path = std::env::temp_dir().join(format!(
            "iroha-source-store-{}-{}",
            std::process::id(),
            NEXT_V1.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for DirectoryV1 {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
type Source = Phase23RadixWitnessMaterializedV2<core::convert::Infallible, (), ()>;
const FILE_BYTES: u64 = 66 * 16_400;

// This deliberately lacks replay Evidence and uses a short compact snapshot.
// It tests original-owner storage custody without fabricating native40 authority.
fn unqualified_source_v1(directory: &DirectoryV1) -> Source {
    let layout = ConfidentialSpoolLayoutV1::new_v1(3, 16_384, [1; 32]).unwrap();
    let mut writer = ConfidentialSpoolWriterV1::create_in_v1(&directory.0, layout).unwrap();
    for slot in 0..3 {
        writer
            .write_slot_v1(
                slot,
                ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap(),
            )
            .unwrap();
    }
    let snapshot = writer.seal_v1().unwrap();
    let mut record = RadixWitnessMaterializationRecordV2 {
        replay_record_digest: [0x11; 32],
        source_receipt_digest: [0x22; 32],
        mapping_digest: [0x33; 32],
        spool_context_digest: [0x44; 32],
        authenticated_read_schedule_root: [0x55; 32],
        snapshot_root: *snapshot.snapshot_digest_v1(),
        source_reread_blocks: RADIX_SOURCE_REREAD_BLOCKS_V2 as u32,
        source_reread_plaintext_bytes: RADIX_SOURCE_REREAD_PLAINTEXT_BYTES_V2,
        source_reread_authenticated_bytes: RADIX_SOURCE_REREAD_AUTHENTICATED_BYTES_V2,
        output_slot_count: RADIX_WITNESS_SLOT_COUNT_V2 as u16,
        output_plaintext_bytes: RADIX_WITNESS_PLAINTEXT_BYTES_V2,
        output_authentication_tag_bytes: RADIX_WITNESS_AUTHENTICATION_TAG_BYTES_V2,
        output_file_bytes: RADIX_WITNESS_FILE_BYTES_V2,
        output_spool_io_bytes: RADIX_WITNESS_SPOOL_IO_BYTES_V2,
        total_io_bytes: RADIX_WITNESS_TOTAL_IO_BYTES_V2,
        named_live_payload_bytes: RADIX_WITNESS_NAMED_LIVE_PAYLOAD_BYTES_V2 as u32,
        authenticated_canonical_reread_complete: true,
        compact_radix_witness_materialized: true,
        commitments_constructed: false,
        transcript_bound: false,
        final_arithmetic_plane_constructed: false,
        radix_proof_verified: false,
        zero_knowledge_accepted: false,
        authority_minted: false,
        rss_qualified: false,
        operational_receipt_accepted: false,
        release_ready: false,
        release_complete: false,
        record_digest: [0; 32],
    };
    record.record_digest = radix_witness_record_digest_v2(&record).unwrap();
    let materialization_seal = RadixWitnessMaterializationSealV2::mint_v2(
        record.replay_record_digest,
        record.spool_context_digest,
        record.snapshot_root,
        record.record_digest,
    )
    .unwrap();
    Source {
        evidence: None,
        snapshot,
        record,
        materialization_seal,
        next_comparator_plane: 0,
        ordered_writer: None,
    }
}
fn chunk_v1(slot: u64) -> ConfidentialSpoolChunkV1 {
    let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
    if slot % 33 == 32 {
        chunk.as_mut_slice_v1()[31] = 1;
        chunk.as_mut_slice_v1()[32..65].copy_from_slice(
            &crate::vega::VegaT256PointV1::canonical_generator()
                .unwrap()
                .to_non_identity_wire_bytes()
                .unwrap(),
        );
    }
    chunk
}
fn snapshot_v1(
    directory: &DirectoryV1,
    budget: &mut OrderedStorageSessionBudgetV1,
) -> OrderedPlaneSpoolSnapshotV1 {
    let mut writer =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], budget).unwrap();
    for slot in 0..66 {
        writer.write_slot_v1(slot, chunk_v1(slot)).unwrap();
    }
    writer.seal_v1().unwrap()
}
fn replay_v1(
    directory: &DirectoryV1,
    snapshot: OrderedPlaneSpoolSnapshotV1,
) -> OrderedMaterializedReplayV1<core::convert::Infallible, (), ()> {
    let mut source = unqualified_source_v1(directory);
    source.next_comparator_plane = PLANES_V1;
    let identity = snapshot.snapshot_digest_v1().unwrap();
    OrderedMaterializedReplayV1 {
        live: Some(StoredMaterializedLiveV1 {
            source,
            snapshot,
            identity,
            next_slot: 0,
        }),
    }
}

#[test]
#[cfg(unix)]
fn materialized_storage_rejects_unqualified_source_before_reservation_or_file_creation() {
    let directory = DirectoryV1::new_v1();
    for ordinal in [0, 1, 9_288, u16::MAX] {
        let mut source = unqualified_source_v1(&directory);
        source.next_comparator_plane = ordinal;
        let mut budget = OrderedStorageSessionBudgetV1::new_v1();
        let refusal = match source.begin_ordered_storage_v1(&directory.0, &mut budget) {
            Ok(_) => panic!("unqualified source accepted"),
            Err(refusal) => refusal,
        };
        assert_eq!(refusal.reason_v1(), MaterializedStorageErrorV1::Source);
        assert!(refusal.source.is_none());
        let refusal = match refusal.retry_v1(&directory.0, &mut budget) {
            Ok(_) => panic!("fatal refusal was retryable"),
            Err(error) => error,
        };
        assert_eq!(refusal.reason_v1(), MaterializedStorageErrorV1::Source);
        assert!(refusal.source.is_none());
        assert_eq!(budget.test_usage_words_v1(), [0; 4]);
        assert!(directory.0.read_dir().unwrap().next().is_none());
    }
}

#[test]
#[cfg(unix)]
fn materialized_storage_pre_io_capacity_retains_exact_whole_source_and_retries_after_release() {
    let directory = DirectoryV1::new_v1();
    let mut budget = OrderedStorageSessionBudgetV1::with_test_limits_v1(FILE_BYTES, 4 * FILE_BYTES);
    let blocking =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], &mut budget)
            .unwrap();
    let source = unqualified_source_v1(&directory);
    let original_snapshot = *source.snapshot.snapshot_digest_v1();
    let original_record = source.record.record_digest;
    let before = budget.test_usage_words_v1();
    let result =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], &mut budget);
    let mut refusal = match source.complete_ordered_storage_admission_v1(result) {
        Ok(_) => panic!("capacity accepted"),
        Err(error) => error,
    };
    assert_eq!(
        refusal.reason_v1(),
        MaterializedStorageErrorV1::Storage(OrderedSnapshotErrorV1::Capacity)
    );
    assert_eq!(budget.test_usage_words_v1(), before);
    let retained = refusal.source.as_ref().unwrap();
    assert_eq!(*retained.snapshot.snapshot_digest_v1(), original_snapshot);
    assert_eq!(retained.record.record_digest, original_record);
    assert_eq!(retained.next_comparator_plane, 0);
    assert!(retained.ordered_writer.is_none());
    drop(blocking);
    // Exercise the same concrete admission join with the same retained fixture.
    // Its public start remains rejected because it is not authenticated source.
    let source = refusal.source.take().unwrap();
    let writer =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], &mut budget);
    let source = match source.complete_ordered_storage_admission_v1(writer) {
        Ok(source) => source,
        Err(_) => panic!("released capacity refused"),
    };
    assert_eq!(*source.snapshot.snapshot_digest_v1(), original_snapshot);
    source
        .ordered_writer
        .as_ref()
        .unwrap()
        .require_next_slot_v1(0)
        .unwrap();
    drop(source);
    assert_eq!(budget.test_usage_words_v1()[0], 0);
    assert_eq!(budget.test_usage_words_v1()[3], 0);
}

#[test]
#[cfg(unix)]
fn materialized_storage_read_capacity_preserves_same_pair_identity_source_and_cursor() {
    let directory = DirectoryV1::new_v1();
    let mut budget =
        OrderedStorageSessionBudgetV1::with_test_limits_v1(2 * FILE_BYTES, 4 * FILE_BYTES);
    let snapshot = snapshot_v1(&directory, &mut budget);
    let mut replay = replay_v1(&directory, snapshot);
    let identity = replay.live.as_ref().unwrap().identity;
    let source_identity = *replay
        .live
        .as_ref()
        .unwrap()
        .source
        .snapshot
        .snapshot_digest_v1();
    let blocking =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], &mut budget)
            .unwrap();
    let before = budget.test_usage_words_v1();
    assert_eq!(
        replay.verify_next_slot_v1(),
        Err(MaterializedStorageErrorV1::Storage(
            OrderedSnapshotErrorV1::Capacity
        ))
    );
    let live = replay.live.as_ref().unwrap();
    assert_eq!(live.identity, identity);
    assert_eq!(live.snapshot.snapshot_digest_v1().unwrap(), identity);
    assert_eq!(*live.source.snapshot.snapshot_digest_v1(), source_identity);
    assert_eq!(live.next_slot, 0);
    assert_eq!(budget.test_usage_words_v1(), before);
    drop(blocking);
    assert_eq!(replay.verify_next_slot_v1(), Ok(false));
    assert_eq!(replay.live.as_ref().unwrap().next_slot, 1);
    assert_eq!(replay.live.as_ref().unwrap().identity, identity);
    assert_eq!(budget.test_usage_words_v1()[3], 2 * FILE_BYTES + 16_400);
    drop(replay);
    assert_eq!(budget.test_usage_words_v1()[0], 0);
}

#[test]
#[cfg(unix)]
fn materialized_storage_replayed_pair_missing_original_tail_and_short_replay_drop_both() {
    let directory = DirectoryV1::new_v1();
    for fault in 0..4 {
        let mut budget = OrderedStorageSessionBudgetV1::new_v1();
        let snapshot = snapshot_v1(&directory, &mut budget);
        let mut replay = replay_v1(&directory, snapshot);
        match fault {
            0 => replay.live.as_mut().unwrap().snapshot = snapshot_v1(&directory, &mut budget),
            1 => replay.live.as_mut().unwrap().next_slot = 32, // Genuine AEAD tail, missing original Evidence.
            2 => replay.live.as_mut().unwrap().next_slot = SLOTS_V1,
            _ => {
                assert!(replay.finish_v1().is_err());
                assert_eq!(budget.test_usage_words_v1()[0], 0);
                continue;
            }
        }
        assert!(replay.verify_next_slot_v1().is_err());
        assert!(replay.live.is_none());
        assert!(replay.verify_next_slot_v1().is_err());
        assert!(replay.finish_v1().is_err());
        assert_eq!(budget.test_usage_words_v1()[0], 0);
    }
}

#[test]
#[cfg(unix)]
fn materialized_storage_creation_error_and_short_seal_never_return_original_source() {
    let directory = DirectoryV1::new_v1();
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let source = unqualified_source_v1(&directory);
    let missing = directory.0.join("absent");
    let writer = OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&missing, [7; 32], &mut budget);
    let refusal = match source.complete_ordered_storage_admission_v1(writer) {
        Ok(_) => panic!("missing directory accepted"),
        Err(error) => error,
    };
    assert_eq!(
        refusal.reason_v1(),
        MaterializedStorageErrorV1::Storage(OrderedSnapshotErrorV1::Storage)
    );
    assert!(refusal.source.is_none());
    assert_eq!(budget.test_usage_words_v1()[0], 0);
    let mut source = unqualified_source_v1(&directory);
    source.ordered_writer = Some(
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], &mut budget)
            .unwrap(),
    );
    assert!(source.seal_ordered_storage_v1().is_err());
    assert_eq!(budget.test_usage_words_v1()[0], 0);
}

#[test]
fn materialized_storage_context_view_keeps_existing_encoder_order_and_kat() {
    // Private test-only view exercises the unchanged encoder. It is not used
    // to manufacture a production source or enter its consuming start method.
    let view = MaterializedPlaneContextV1 {
        axes: [[0x11; 32], [0x22; 32], [0x44; 32]],
        original_owner: PhantomData,
    };
    let digest = materialized_plane_context_digest_v1(&view).unwrap();
    assert_eq!(
        hex::encode(digest),
        "ee8e26c5e94f1234947a027894ab7e23ed4e4ec9fbe22a14b651c498c0805a86"
    );
    for index in 0..3 {
        let mut changed = MaterializedPlaneContextV1 {
            axes: view.axes,
            original_owner: PhantomData,
        };
        changed.axes[index] = [0; 32];
        assert!(materialized_plane_context_digest_v1(&changed).is_err());
        changed.axes[index] = [0x55; 32];
        assert_ne!(
            materialized_plane_context_digest_v1(&changed).unwrap(),
            digest
        );
    }
    let swapped = MaterializedPlaneContextV1 {
        axes: [[0x22; 32], [0x11; 32], [0x44; 32]],
        original_owner: PhantomData,
    };
    assert_ne!(
        materialized_plane_context_digest_v1(&swapped).unwrap(),
        digest
    );
}

#[test]
fn u15_stored_source_gate_rejects_unqualified_real_pair_and_closes_files() {
    use crate::generalized_bulletproof::secret_u15_msm_v1::test_controls_v1 as controls;
    let directory = DirectoryV1::new_v1();
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let snapshot = snapshot_v1(&directory, &mut budget);
    let mut replay = replay_v1(&directory, snapshot);
    // This fixture has no authenticated source Evidence. Setting a cursor is
    // deliberately insufficient to create kernel or native40 source authority.
    replay.live.as_mut().unwrap().next_slot = SLOTS_V1;
    let original = replay.finish_v1().unwrap();
    assert_eq!(budget.test_usage_words_v1()[0], FILE_BYTES);
    controls::reset_v1();
    let refusal = match original.prepare_q_mask_kernel_v1() {
        Ok(_) => panic!("unqualified fixture cannot prepare kernel"),
        Err(error) => error,
    };
    assert_eq!(refusal.reason_v1(), RnsNativeU15MsmErrorV1::Source);
    assert!(refusal.original.is_none());
    assert_eq!(controls::allocations_v1(), 0);
    assert_eq!(budget.test_usage_words_v1()[0], 0);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
#[cfg(unix)]
fn qmask_first_outer_rejects_missing_evidence_wrong_cursor_and_identity_without_sampling() {
    use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::source_algebra::QMaskSErrorV1;
    let directory = DirectoryV1::new_v1();
    for failure in 0..3 {
        let mut storage = OrderedStorageSessionBudgetV1::new_v1();
        let pair = snapshot_v1(&directory, &mut storage);
        let mut live = replay_v1(&directory, pair).live.take().unwrap();
        live.next_slot = SLOTS_V1;
        if failure == 1 {
            live.next_slot -= 1;
        }
        if failure == 2 {
            live.identity[0] ^= 1;
        }
        let kernel = PreparedStoredQMaskKernelV1 {
            live: Some(VerifiedStoredMaterializedSourceV1 { live }),
        };
        let before = storage.test_usage_words_v1()[3];
        let refusal = match kernel.sample_first_q_mask_block_v1(&directory.0) {
            Ok(_) => panic!("unqualified source entered original-S sampling"),
            Err(error) => error,
        };
        assert_eq!(refusal.reason_v1(), QMaskSErrorV1::Source);
        assert_eq!(storage.test_usage_words_v1()[0], 0);
        assert_eq!(storage.test_usage_words_v1()[2], 0);
        assert_eq!(storage.test_usage_words_v1()[3], before);
        let retry = match refusal.retry_v1(&directory.0) {
            Ok(_) => panic!("fatal source refusal was retryable"),
            Err(error) => error,
        };
        assert_eq!(retry.reason_v1(), QMaskSErrorV1::Source);
        assert_eq!(storage.test_usage_words_v1()[3], before);
        assert!(directory.0.read_dir().unwrap().next().is_none());
    }
}
