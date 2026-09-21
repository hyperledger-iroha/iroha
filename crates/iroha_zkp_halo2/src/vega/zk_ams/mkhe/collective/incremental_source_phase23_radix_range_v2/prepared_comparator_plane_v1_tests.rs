//! Real tiny storage and independent comparator projection/ownership controls.
use super::*;
use crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1;
#[cfg(unix)]
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn packed_chunk_v1(fill: u8) -> ConfidentialSpoolChunkV1 {
    let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
    chunk.as_mut_slice_v1().fill(fill);
    chunk
}

#[test]
fn all_7224_selectors_are_bijective_in_the_existing_21_bits_per_group() {
    let mut expected: Vec<(u64, u8, u8)> = Vec::new();
    for group in 0..344 {
        expected.push((3 * group, 0, 0));
    }
    for group in 0..344 {
        expected.push((3 * group, 0, 1));
    }
    let beta_bits: [(u8, u8); 18] = [
        (0, 2),
        (0, 3),
        (0, 4),
        (0, 5),
        (0, 6),
        (0, 7),
        (1, 0),
        (1, 1),
        (1, 2),
        (1, 3),
        (1, 4),
        (1, 5),
        (1, 6),
        (1, 7),
        (2, 0),
        (2, 1),
        (2, 2),
        (2, 3),
    ];
    for group in 0..344 {
        for (lane, bit) in beta_bits {
            expected.push((3 * group + u64::from(lane), lane, bit));
        }
    }
    for group in 0..344 {
        expected.push((3 * group + 2, 2, 4));
    }
    let mut seen = std::collections::BTreeSet::new();
    for (ordinal, (slot, lane, bit)) in expected.into_iter().enumerate() {
        let actual = comparator_coordinate_v1(ordinal as u16).unwrap();
        assert_eq!(
            (actual.ordinal, actual.slot, actual.lane, actual.bit),
            (ordinal as u16, slot, lane, bit)
        );
        assert!(seen.insert((slot, bit)));
    }
    assert_eq!(seen.len(), 7_224);
    assert_eq!(comparator_coordinate_v1(7_223).unwrap().slot, 1_031);
    for invalid in [7_224, 7_225, u16::MAX] {
        assert!(comparator_coordinate_v1(invalid).is_err());
    }
}

#[test]
fn every_selector_axis_and_reserved_tail_bit_is_checked_before_scalar_allocation() {
    let coordinate = comparator_coordinate_v1(702).unwrap(); // group0, beta14 -> lane2 bit0
    for axis in 0..4 {
        let mut changed = coordinate;
        match axis {
            0 => changed.ordinal += 1,
            1 => changed.slot += 1,
            2 => changed.lane = 1,
            _ => changed.bit += 1,
        }
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        assert!(expand_comparator_values_v1(packed_chunk_v1(0), changed).is_err());
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before);
    }
    for length in [1, 16_383] {
        assert!(
            expand_comparator_values_v1(
                ConfidentialSpoolChunkV1::new_zeroed_v1(length).unwrap(),
                coordinate
            )
            .is_err()
        );
    }
    assert!(ConfidentialSpoolChunkV1::new_zeroed_v1(16_385).is_err());
    for position in [0, 511, 512, 16_383] {
        for bit in [5, 6, 7] {
            let mut packed = packed_chunk_v1(0);
            packed.as_mut_slice_v1()[position] = 1 << bit;
            let before = zeroizing_t256_scalar_vec_drop_count_v1();
            assert!(expand_comparator_values_v1(packed, coordinate).is_err());
            assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before);
        }
    }
}

const BOUNDARY_PACKED_V1: [[u8; 3]; 7] = [
    [2, 255, 15],
    [2, 255, 15],
    [0, 255, 15],
    [0, 0, 0],
    [0, 0, 0],
    [1, 255, 23],
    [1, 0, 23],
];
fn boundary_coefficients_v1() -> [[u8; 32]; 7] {
    let mut one = [0; 32];
    one[31] = 1;
    let mut top = [0; 32];
    top[0] = 0x80;
    [
        [0; 32],
        one,
        decrement_be_v2(RADIX_CENTERING_THRESHOLD_BE_V2),
        RADIX_CENTERING_THRESHOLD_BE_V2,
        decrement_be_v2(top),
        top,
        RADIX_MODULUS_MINUS_ONE_BE_V2,
    ]
}
// The final 64 zero coefficients are an arithmetic fixture, not decoded-slot padding.
fn boundary_case_v1(index: usize) -> usize {
    if index >= 16_320 { 0 } else { index % 7 }
}
#[cfg(unix)]
static NEXT_DIRECTORY_V1: AtomicU64 = AtomicU64::new(0);
#[cfg(unix)]
struct DirectoryV1(PathBuf);
#[cfg(unix)]
impl DirectoryV1 {
    fn new_v1() -> Self {
        let path = std::env::temp_dir().join(format!(
            "iroha-comparator-values-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY_V1.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
#[cfg(unix)]
impl Drop for DirectoryV1 {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn boundary_snapshot_v1(directory: &DirectoryV1) -> ConfidentialSpoolSnapshotV1 {
    let layout = ConfidentialSpoolLayoutV1::new_v1(3, 16_384, [0x41; 32]).unwrap();
    let mut writer = ConfidentialSpoolWriterV1::create_in_v1(&directory.0, layout).unwrap();
    for lane in 0..3 {
        let mut packed = packed_chunk_v1(0);
        for (index, byte) in packed.as_mut_slice_v1().iter_mut().enumerate() {
            *byte = BOUNDARY_PACKED_V1[boundary_case_v1(index)][lane];
        }
        writer.write_slot_v1(lane as u64, packed).unwrap();
    }
    writer.seal_v1().unwrap()
}

#[cfg(unix)]
#[test]
fn real_storage_projection_matches_independent_boundaries_in_all_21_roles_and_full_coefficient_extent()
 {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    for (encoded, expected) in boundary_coefficients_v1().iter().zip(BOUNDARY_PACKED_V1) {
        let witness = radix_coefficient_witness_v2(encoded).unwrap();
        let packed = pack_comparator_lanes_v2(&witness).unwrap();
        assert_eq!(*packed.as_ref_v2(), expected);
    }
    let directory = DirectoryV1::new_v1();
    let mut snapshot = boundary_snapshot_v1(&directory);
    let ordinals = [
        0, 344, 688, 689, 690, 691, 692, 693, 694, 695, 696, 697, 698, 699, 700, 701, 702, 703,
        704, 705, 6880,
    ];
    for (role, ordinal) in ordinals.into_iter().enumerate() {
        let coordinate = comparator_coordinate_v1(ordinal).unwrap();
        let mut values = read_comparator_values_v1(&mut snapshot, coordinate, [0x41; 32]).unwrap();
        assert_eq!(values.live.as_ref().unwrap().values.len(), 16_384);
        for chunk_index in 0..32 {
            let chunk = values.emit_next_v1(chunk_index).unwrap();
            assert_eq!(chunk.len_v1(), 16_384);
            for (local, bytes) in chunk.as_slice_v1().chunks_exact(32).enumerate() {
                let index = usize::from(chunk_index) * 512 + local;
                let packed = BOUNDARY_PACKED_V1[boundary_case_v1(index)];
                let expected = match role {
                    0 => packed[0] & 1,
                    1 => (packed[0] >> 1) & 1,
                    2..=7 => (packed[0] >> role) & 1,
                    8..=15 => (packed[1] >> (role - 8)) & 1,
                    16..=19 => (packed[2] >> (role - 16)) & 1,
                    _ => (packed[2] >> 4) & 1,
                };
                assert!(bytes[..31].iter().all(|byte| *byte == 0));
                assert_eq!(bytes[31], expected, "role {role}, coordinate {index}");
            }
        }
        values.finish_v1().unwrap();
    }
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn storage_context_wrong_slot_and_authenticated_reserved_bits_reject() {
    let directory = DirectoryV1::new_v1();
    let mut snapshot = boundary_snapshot_v1(&directory);
    assert!(
        read_comparator_values_v1(
            &mut snapshot,
            comparator_coordinate_v1(0).unwrap(),
            [0x42; 32]
        )
        .is_err()
    );
    let mut snapshot = boundary_snapshot_v1(&directory);
    assert!(
        read_comparator_values_v1(
            &mut snapshot,
            comparator_coordinate_v1(1).unwrap(),
            [0x41; 32]
        )
        .is_err()
    );
    let layout = ConfidentialSpoolLayoutV1::new_v1(3, 16_384, [0x41; 32]).unwrap();
    let mut writer = ConfidentialSpoolWriterV1::create_in_v1(&directory.0, layout).unwrap();
    for slot in 0..3 {
        writer
            .write_slot_v1(slot, packed_chunk_v1(if slot == 2 { 0x80 } else { 0 }))
            .unwrap();
    }
    let mut snapshot = writer.seal_v1().unwrap();
    assert!(
        read_comparator_values_v1(
            &mut snapshot,
            comparator_coordinate_v1(702).unwrap(),
            [0x41; 32]
        )
        .is_err()
    );
}

#[test]
fn emitted_chunks_preserve_every_coordinate_without_a_second_transpose() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let mut packed = packed_chunk_v1(0);
    for index in [0, 1, 63, 64, 255, 256, 511, 512, 16_319, 16_320, 16_383] {
        packed.as_mut_slice_v1()[index] = 1;
    }
    let mut values =
        expand_comparator_values_v1(packed, comparator_coordinate_v1(0).unwrap()).unwrap();
    for chunk_index in 0..32 {
        let chunk = values.emit_next_v1(chunk_index).unwrap();
        for (local, bytes) in chunk.as_slice_v1().chunks_exact(32).enumerate() {
            let index = usize::from(chunk_index) * 512 + local;
            assert_eq!(
                bytes[31],
                u8::from(
                    [0, 1, 63, 64, 255, 256, 511, 512, 16_319, 16_320, 16_383].contains(&index)
                )
            );
            assert!(bytes[..31].iter().all(|byte| *byte == 0));
        }
    }
    values.finish_v1().unwrap();
}

#[test]
fn order_retries_overrun_incomplete_finish_and_drop_clear_values() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    for first_wrong in [1, 31, 32, u8::MAX] {
        let mut values =
            expand_comparator_values_v1(packed_chunk_v1(1), comparator_coordinate_v1(0).unwrap())
                .unwrap();
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        assert!(values.emit_next_v1(first_wrong).is_err());
        assert!(values.live.is_none());
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 1);
        assert!(values.emit_next_v1(0).is_err());
        assert!(values.finish_v1().is_err());
    }
    for complete in [0, 1, 31, 32] {
        let mut values =
            expand_comparator_values_v1(packed_chunk_v1(1), comparator_coordinate_v1(0).unwrap())
                .unwrap();
        for chunk in 0..complete {
            drop(values.emit_next_v1(chunk).unwrap());
        }
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        assert_eq!(values.finish_v1().is_ok(), complete == 32);
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 1);
    }
    let mut values =
        expand_comparator_values_v1(packed_chunk_v1(1), comparator_coordinate_v1(0).unwrap())
            .unwrap();
    drop(values.emit_next_v1(0).unwrap());
    assert!(values.emit_next_v1(0).is_err());
    assert!(values.live.is_none());
    let mut values =
        expand_comparator_values_v1(packed_chunk_v1(1), comparator_coordinate_v1(0).unwrap())
            .unwrap();
    for chunk in 0..32 {
        drop(values.emit_next_v1(chunk).unwrap());
    }
    assert!(values.emit_next_v1(32).is_err()); // commitment tail is not a value chunk
    assert!(values.finish_v1().is_err());
    let values =
        expand_comparator_values_v1(packed_chunk_v1(1), comparator_coordinate_v1(0).unwrap())
            .unwrap();
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    drop(values);
    assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 1);
}

#[test]
fn source_consumer_is_normal_registered_and_preserves_all_authority_and_poison_boundaries() {
    let source = include_str!("prepared_comparator_plane_v1.rs");
    let parent = include_str!("../incremental_source_phase23_radix_range_v2.rs");
    let replay = include_str!(
        "../incremental_source_phase23_source_algebra/global_lookup_source_replay_v1.rs"
    );
    assert!(parent.contains("mod prepared_comparator_plane_v1;"));
    assert!(parent.contains("next_comparator_plane: 0"));
    assert!(parent.contains("next_comparator_plane: _,"));
    let prepare = source
        .split("fn prepare_next_comparator_plane_v1(")
        .nth(1)
        .unwrap()
        .split("struct PreparedComparatorPlaneLiveV1")
        .next()
        .unwrap();
    assert!(
        prepare
            .find("comparator_coordinate_v1(self.next_comparator_plane)")
            .unwrap()
            < prepare.find("validate_comparator_preparation_v1").unwrap()
    );
    assert!(
        prepare.find("validate_comparator_preparation_v1").unwrap()
            < prepare
                .find("validate_materialized_context_v1(&self)")
                .unwrap()
    );
    assert!(
        prepare
            .find("validate_materialized_context_v1(&self)")
            .unwrap()
            < prepare.find("read_comparator_values_v1(").unwrap()
    );
    assert!(
        prepare.find("read_comparator_values_v1(").unwrap()
            < prepare
                .find(".commit_prepared_comparator_v1(&statement)")
                .unwrap()
    );
    assert!(
        prepare
            .find(".commit_prepared_comparator_v1(&statement)")
            .unwrap()
            < prepare.find("Ok(PreparedComparatorPlaneV1").unwrap()
    );
    let preflight = replay
        .split("fn validate_comparator_preparation_v1(")
        .nth(1)
        .unwrap()
        .split("fn commit_prepared_comparator_v1(")
        .next()
        .unwrap();
    assert!(preflight.contains("self.validate_radix_materialization_source_v1("));
    assert!(preflight.contains("self.openings.require_comparator_position_v1(ordinal)"));
    let lineage = replay
        .split("fn validate_radix_materialization_source_v1(")
        .nth(1)
        .unwrap()
        .split("fn validate_replay_evidence_v1")
        .next()
        .unwrap();
    for required in [
        "validate_replay_evidence_v1(self)?",
        "self.record.record_digest != replay_record_digest",
        "self.record.source_receipt_digest != source_receipt_digest",
    ] {
        assert!(lineage.contains(required));
    }
    for required in [
        "validate_radix_witness_record_v2(record)",
        "record.mapping_digest != mapping",
        "radix_witness_context_digest_v2(",
        "source.snapshot.slot_count_v1()",
        "source.snapshot.plaintext_len_v1()",
        "source.snapshot.file_len_v1()",
        "source.snapshot.snapshot_digest_v1()",
        "validate_for_materialized_record_v2(&source.record)",
    ] {
        assert!(source.contains(required));
    }
    for forbidden in [
        "fn into_parts",
        "Infallible",
        "mem::forget",
        "to_be_bytes()",
        "impl Clone",
        "derive(Clone, Copy, Debug)]\nstruct Prepared",
        "point: &VegaT256PointV1",
        "proof_ready",
        "release_ready: true",
    ] {
        assert!(!source.contains(forbidden));
    }
    let emission = source
        .split("fn emit_next_value_chunk_v1(")
        .nth(1)
        .unwrap()
        .split("fn finish_v1(")
        .next()
        .unwrap();
    let emission = emission.split_whitespace().collect::<String>();
    assert!(
        emission.find("self.live.take()").unwrap()
            < emission
                .find("live.opening.emit_next_value_chunk_v1")
                .unwrap()
    );
    assert!(
        emission
            .find("live.opening.emit_next_value_chunk_v1")
            .unwrap()
            < emission.find("self.live=Some(live)").unwrap()
    );
    let finish = source
        .split("fn finish_v1(")
        .nth(1)
        .unwrap()
        .split("struct PreparedRadixValuesStateV1")
        .next()
        .unwrap();
    assert!(
        finish.find("live.opening.finish_v1()?").unwrap()
            < finish.find("source.next_comparator_plane =").unwrap()
    );
}

fn invalid_record_v1() -> RadixWitnessMaterializationRecordV2 {
    RadixWitnessMaterializationRecordV2 {
        replay_record_digest: [0x11; 32],
        source_receipt_digest: [0x22; 32],
        mapping_digest: [0x33; 32],
        spool_context_digest: [0x44; 32],
        authenticated_read_schedule_root: [0x55; 32],
        snapshot_root: [0x66; 32],
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
    }
}

#[cfg(unix)]
fn invalid_source_v1(
    directory: &DirectoryV1,
) -> Phase23RadixWitnessMaterializedV2<core::convert::Infallible, (), ()> {
    let snapshot = boundary_snapshot_v1(directory);
    let mut record = invalid_record_v1();
    record.snapshot_root = *snapshot.snapshot_digest_v1();
    record.record_digest = radix_witness_record_digest_v2(&record).unwrap();
    let materialization_seal = RadixWitnessMaterializationSealV2::mint_v2(
        record.replay_record_digest,
        record.spool_context_digest,
        record.snapshot_root,
        record.record_digest,
    )
    .unwrap();
    Phase23RadixWitnessMaterializedV2 {
        evidence: None,
        snapshot,
        record,
        materialization_seal,
        next_comparator_plane: 0,
        ordered_writer: None,
    }
}

#[cfg(unix)]
#[test]
fn consuming_source_entry_rejects_missing_evidence_and_exhausted_cursor() {
    let directory = DirectoryV1::new_v1();
    for ordinal in [0, 7_223, 7_224, u16::MAX] {
        let mut source = invalid_source_v1(&directory);
        source.next_comparator_plane = ordinal;
        assert!(source.prepare_next_comparator_plane_v1().is_err());
    }
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[cfg(unix)]
fn failure_opening_fixture_v1() -> PreparedPlaneOpeningV1 {
    PreparedPlaneOpeningV1::from_committed_v1(
        expand_comparator_values_v1(packed_chunk_v1(1), comparator_coordinate_v1(0).unwrap())
            .unwrap(),
        super::super::super::source_algebra::PreparedPlaneOpeningTailV1::test_wire_fixture_v1(0),
        0,
    )
    .unwrap()
}

#[cfg(unix)]
#[test]
fn outer_chunk_errors_and_early_finish_drop_the_entire_unadmitted_source_owner() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let directory = DirectoryV1::new_v1();
    for wrong in [1, 32, u8::MAX] {
        let mut prepared = PreparedComparatorPlaneV1 {
            live: Some(PreparedComparatorPlaneLiveV1 {
                source: invalid_source_v1(&directory),
                opening: failure_opening_fixture_v1(),
            }),
        };
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        assert!(prepared.emit_next_value_chunk_v1(wrong).is_err());
        assert!(prepared.live.is_none());
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 1);
        assert!(prepared.emit_next_value_chunk_v1(0).is_err());
        assert!(prepared.finish_v1().is_err());
    }
    let prepared = PreparedComparatorPlaneV1 {
        live: Some(PreparedComparatorPlaneLiveV1 {
            source: invalid_source_v1(&directory),
            opening: failure_opening_fixture_v1(),
        }),
    };
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    assert!(prepared.finish_v1().is_err());
    assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 1);
}

#[cfg(unix)]
#[test]
fn prepared_opening_tail_outer_error_and_missing_tail_close_source_before_handoff() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let directory = DirectoryV1::new_v1();
    for early_tail in [false, true] {
        // An unadmitted source is intentional: only outer failure ownership is
        // tested here. This fixture cannot provide production source authority.
        let mut prepared = PreparedComparatorPlaneV1 {
            live: Some(PreparedComparatorPlaneLiveV1 {
                source: invalid_source_v1(&directory),
                opening: failure_opening_fixture_v1(),
            }),
        };
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        if early_tail {
            assert!(prepared.emit_opening_tail_v1().is_err());
            assert!(prepared.live.is_none());
            assert!(prepared.emit_next_value_chunk_v1(0).is_err());
        } else {
            for chunk in 0..32 {
                drop(prepared.emit_next_value_chunk_v1(chunk).unwrap());
            }
        }
        assert!(prepared.finish_v1().is_err());
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 1);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    }
}

#[cfg(unix)]
#[test]
fn unwind_after_owner_take_drops_retained_values_and_does_not_restore_source() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let directory = DirectoryV1::new_v1();
    let mut prepared = PreparedComparatorPlaneV1 {
        live: Some(PreparedComparatorPlaneLiveV1 {
            source: invalid_source_v1(&directory),
            opening: failure_opening_fixture_v1(),
        }),
    };
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _live = prepared.live.take().unwrap();
            panic!("intentional prepared-owner unwind");
        }))
        .is_err()
    );
    assert!(prepared.live.is_none());
    assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 1);
    assert!(prepared.finish_v1().is_err());
}

#[test]
fn self_hashed_record_still_requires_exact_mapping_and_lineage_derived_context() {
    for axis in 0..4 {
        let mut record = invalid_record_v1();
        record.mapping_digest = canonical_radix_mapping_v1().unwrap();
        assert_eq!(canonical_radix_mapping_v1().unwrap(), record.mapping_digest);
        record.spool_context_digest = radix_witness_context_digest_v2(
            record.replay_record_digest,
            record.source_receipt_digest,
            record.mapping_digest,
        )
        .unwrap();
        record.record_digest = radix_witness_record_digest_v2(&record).unwrap();
        validate_materialization_record_context_v1(&record).unwrap();
        let mut changed = record;
        match axis {
            0 => changed.mapping_digest[0] ^= 1,
            1 => changed.spool_context_digest[0] ^= 1,
            2 => changed.replay_record_digest[0] ^= 1,
            _ => changed.source_receipt_digest[0] ^= 1,
        }
        changed.record_digest = radix_witness_record_digest_v2(&changed).unwrap();
        validate_radix_witness_record_v2(&changed).unwrap();
        assert!(validate_materialization_record_context_v1(&changed).is_err());
    }
}

#[cfg(not(unix))]
#[test]
fn unsupported_storage_platform_cannot_supply_a_prepared_source() {
    let layout = ConfidentialSpoolLayoutV1::new_v1(3, 16_384, [0x41; 32]).unwrap();
    assert!(matches!(
        ConfidentialSpoolWriterV1::create_in_v1(std::env::temp_dir(), layout),
        Err(iroha_crypto::confidential_spool::ConfidentialSpoolErrorV1::UnsupportedPlatform)
    ));
}

#[cfg(unix)]
#[test]
fn low_digit_entry_rejects_missing_evidence_and_started_comparator() {
    let directory = DirectoryV1::new_v1();
    for ordinal in [0, 1, 7_224, u16::MAX] {
        let mut source = invalid_source_v1(&directory);
        source.next_comparator_plane = ordinal;
        assert!(source.into_low_digit_preparation_v1().is_err());
    }
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn shared_radix_emitter_rejects_inexact_values_and_zeros_owned_buffers() {
    for length in [0, 1, 16_383, 16_385] {
        let mut values = ZeroizingT256ScalarVecV1::try_with_exact_capacity(length).unwrap();
        for _ in 0..length {
            values.push(VegaT256ScalarV1::from_u64(3));
        }
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        assert!(PreparedRadixValuesV1::from_exact_values_v1(values).is_err());
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 1);
    }
}

#[test]
#[cfg(unix)]
fn materialized_storage_outer_store_advances_original_source_only_after_all_33_writes() {
    use crate::vega::zk_ams::mkhe::global_lookup_statement_v1::{
        OrderedPlaneSpoolWriterV1, OrderedStorageSessionBudgetV1,
    };
    let directory = DirectoryV1::new_v1();
    let mut source = invalid_source_v1(&directory);
    let identity = *source.snapshot.snapshot_digest_v1();
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    source.ordered_writer = Some(
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], &mut budget)
            .unwrap(),
    );
    let prepared = PreparedComparatorPlaneV1 {
        live: Some(PreparedComparatorPlaneLiveV1 {
            source,
            opening: failure_opening_fixture_v1(),
        }),
    };
    let source = match prepared.store_v1() {
        Ok(source) => source,
        Err(_) => panic!("exact prepared stream failed"),
    };
    assert_eq!(source.next_comparator_plane, 1);
    assert_eq!(*source.snapshot.snapshot_digest_v1(), identity);
    source
        .ordered_writer
        .as_ref()
        .unwrap()
        .require_next_slot_v1(33)
        .unwrap();
    assert_eq!(budget.test_usage_words_v1()[3], 33 * 16_400);
    // The enclosing fixture deliberately lacks source authority; local writes
    // cannot promote it into the genuine completed source/sealed replay owner.
    assert!(source.seal_ordered_storage_v1().is_err());
    assert_eq!(budget.test_usage_words_v1()[0], 0);
}

#[test]
#[cfg(unix)]
fn materialized_storage_outer_finish_rejects_emitted_but_dropped_chunks() {
    use crate::vega::zk_ams::mkhe::global_lookup_statement_v1::{
        OrderedPlaneSpoolWriterV1, OrderedStorageSessionBudgetV1,
    };
    let directory = DirectoryV1::new_v1();
    for emitted in [0, 32, 33] {
        let mut source = invalid_source_v1(&directory);
        let mut budget = OrderedStorageSessionBudgetV1::new_v1();
        source.ordered_writer = Some(
            OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(&directory.0, [7; 32], &mut budget)
                .unwrap(),
        );
        let mut prepared = PreparedComparatorPlaneV1 {
            live: Some(PreparedComparatorPlaneLiveV1 {
                source,
                opening: failure_opening_fixture_v1(),
            }),
        };
        for index in 0..emitted.min(32) {
            drop(prepared.emit_next_value_chunk_v1(index).unwrap());
        }
        if emitted == 33 {
            drop(prepared.emit_opening_tail_v1().unwrap());
        }
        assert!(prepared.finish_v1().is_err());
        assert_eq!(budget.test_usage_words_v1()[0], 0);
        assert_eq!(budget.test_usage_words_v1()[3], 0);
    }
}
