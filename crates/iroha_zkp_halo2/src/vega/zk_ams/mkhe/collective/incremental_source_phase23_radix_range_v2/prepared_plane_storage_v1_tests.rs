//! Actual tiny two-file storage of prepared streams; no authenticated source seal.
use super::*;
use crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1;
use crate::vega::zk_ams::mkhe::global_lookup_statement_v1::{
    OrderedPlaneSpoolWriterV1, OrderedSnapshotErrorV1, OrderedStorageSessionBudgetV1,
};
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
            "iroha-prepared-store-{}-{}",
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
fn opening_v1(ordinal: u16) -> PreparedPlaneOpeningV1 {
    let mut values =
        crate::vega::bulletproof_t256::ZeroizingT256ScalarVecV1::try_with_exact_capacity(16_384)
            .unwrap();
    for index in 0..16_384 {
        values.push(crate::vega::VegaT256ScalarV1::from_u64(
            index + u64::from(ordinal),
        ));
    }
    PreparedPlaneOpeningV1::from_committed_v1(
        PreparedRadixValuesV1::from_exact_values_v1(values).unwrap(),
        PreparedPlaneOpeningTailV1::test_wire_fixture_v1(ordinal),
        ordinal,
    )
    .unwrap()
}

#[test]
#[cfg(unix)]
fn prepared_storage_stream_writes_exact_values_and_original_tail_across_both_real_files() {
    let directory = DirectoryV1::new_v1();
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let mut writer =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], &mut budget)
            .unwrap();
    writer.require_context_v1([7; 32]).unwrap();
    assert_eq!(
        writer.require_context_v1([8; 32]),
        Err(OrderedSnapshotErrorV1::Context)
    );
    writer.require_next_slot_v1(0).unwrap();
    let vectors = zeroizing_t256_scalar_vec_drop_count_v1();
    let tails = PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1();
    for ordinal in 0..2 {
        let mut opening = opening_v1(ordinal);
        opening.store_v1(&mut writer, ordinal).unwrap();
        opening.finish_v1().unwrap();
        writer
            .require_next_slot_v1((u64::from(ordinal) + 1) * 33)
            .unwrap();
    }
    assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), vectors + 2);
    assert_eq!(
        PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1(),
        tails + 2
    );
    let mut snapshot = writer.seal_v1().unwrap();
    let identity = snapshot.snapshot_digest_v1().unwrap();
    for ordinal in 0..2 {
        for index in 0..32 {
            let chunk = snapshot.read_slot_v1(ordinal * 33 + index).unwrap();
            for (local, encoded) in chunk.as_slice_v1().chunks_exact(32).enumerate() {
                let value = crate::vega::VegaT256ScalarV1::from_be_bytes_exact_ref(
                    encoded.try_into().unwrap(),
                )
                .unwrap();
                assert_eq!(
                    value,
                    crate::vega::VegaT256ScalarV1::from_u64(index * 512 + local as u64 + ordinal)
                );
            }
        }
        let tail = snapshot.read_slot_v1(ordinal * 33 + 32).unwrap();
        let expected = PreparedPlaneOpeningTailV1::test_wire_fixture_v1(ordinal as u16)
            .into_chunk_v1(ordinal as u16)
            .unwrap();
        assert_eq!(tail.as_slice_v1(), expected.as_slice_v1());
    }
    assert_eq!(snapshot.snapshot_digest_v1().unwrap(), identity);
    assert_eq!(
        budget.test_usage_words_v1(),
        [66 * 16_400, 66 * 16_400, 0, 3 * 66 * 16_400]
    );
    drop(snapshot);
    assert_eq!(budget.test_usage_words_v1()[0], 0);
    assert!(directory.0.read_dir().unwrap().next().is_none());
}

#[test]
#[cfg(unix)]
fn prepared_storage_stream_reordering_prior_emission_and_repeated_store_erase_opening() {
    let directory = DirectoryV1::new_v1();
    for fault in 0..4 {
        let mut budget = OrderedStorageSessionBudgetV1::new_v1();
        let mut writer =
            OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], &mut budget)
                .unwrap();
        let mut opening = opening_v1(0);
        if fault == 0 {
            drop(opening.emit_next_value_chunk_v1(0).unwrap());
        }
        if fault == 3 {
            opening.store_v1(&mut writer, 0).unwrap();
        }
        let vectors = zeroizing_t256_scalar_vec_drop_count_v1();
        let tails = PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1();
        let ordinal = if fault == 1 { 1 } else { 0 };
        if fault == 2 {
            writer
                .write_slot_v1(0, ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap())
                .unwrap();
        }
        assert!(opening.store_v1(&mut writer, ordinal).is_err());
        assert!(opening.live.is_none());
        assert!(opening.finish_v1().is_err());
        if fault != 3 {
            assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), vectors + 1);
            assert_eq!(
                PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1(),
                tails + 1
            );
        }
        drop(writer);
        assert_eq!(budget.test_usage_words_v1()[0], 0);
    }
}

#[test]
#[cfg(unix)]
fn prepared_storage_stream_malformed_value_and_missing_tail_cannot_seal() {
    let directory = DirectoryV1::new_v1();
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let mut writer =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], &mut budget)
            .unwrap();
    let mut bad = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
    bad.as_mut_slice_v1()[..32].copy_from_slice(&crate::vega::VEGA_T256_SCALAR_MODULUS_BE_V1);
    assert_eq!(
        writer.write_slot_v1(0, bad),
        Err(OrderedSnapshotErrorV1::Semantics)
    );
    assert!(writer.seal_v1().is_err());
    assert_eq!(budget.test_usage_words_v1()[0], 0);
    let mut writer =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], &mut budget)
            .unwrap();
    let mut opening = opening_v1(0);
    for index in 0..32 {
        writer
            .write_slot_v1(
                u64::from(index),
                opening.emit_next_value_chunk_v1(index).unwrap(),
            )
            .unwrap();
    }
    assert!(opening.finish_v1().is_err());
    assert!(writer.seal_v1().is_err());
    assert_eq!(budget.test_usage_words_v1()[0], 0);
}

#[test]
#[cfg(unix)]
fn prepared_storage_stream_tail_failure_after_written_values_refuses_completion() {
    let directory = DirectoryV1::new_v1();
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let mut writer =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], &mut budget)
            .unwrap();
    let mut opening = opening_v1(0);
    opening.live.as_mut().unwrap().tail = Some(PreparedPlaneOpeningTailV1::test_wire_fixture_v1(1));
    let vectors = zeroizing_t256_scalar_vec_drop_count_v1();
    let tails = PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1();
    assert!(opening.store_v1(&mut writer, 0).is_err());
    assert!(opening.finish_v1().is_err());
    assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), vectors + 1);
    assert_eq!(
        PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1(),
        tails + 1
    );
    writer.require_next_slot_v1(32).unwrap();
    assert_eq!(budget.test_usage_words_v1()[3], 32 * 16_400);
    assert!(writer.seal_v1().is_err());
    assert_eq!(budget.test_usage_words_v1()[0], 0);
}
