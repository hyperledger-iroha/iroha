//! Geometry, real tiny-storage, and failure-ownership controls; no proof authority.
use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn tiny_plan_v1() -> OrderedPlaneSpoolPlanV1 {
    OrderedPlaneSpoolPlanV1::build_v1(GeometryV1::Tiny, [0x11; 32], [0x22; 32]).unwrap()
}

#[test]
fn complete_geometry_has_the_unique_maximal_whole_plane_prefix_and_bijection() {
    let plan = OrderedPlaneSpoolPlanV1::canonical_v1([0x11; 32]).unwrap();
    plan.validate_v1().unwrap();
    assert_eq!(plan.slot_count_v1(), 306_504);
    assert_eq!(plan.total_planes, 9_288);
    assert_eq!(
        plan.segments[0].words_v1(),
        [0, 0, 7_075, 0, 233_475, 3_828_990_000]
    );
    assert_eq!(
        plan.segments[1].words_v1(),
        [1, 7_075, 2_213, 233_475, 73_029, 1_197_675_600]
    );
    assert_eq!(
        plan.segments
            .iter()
            .map(|segment| segment.file_bytes)
            .sum::<u64>(),
        5_026_665_600
    );
    assert_eq!(
        CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1 - plan.segments[0].file_bytes,
        534_480
    );
    assert!(plan.segments[0].file_bytes + 33 * 16_400 > CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1);
    for global in 0..306_504 {
        let (segment, local) = plan.route_v1(global).unwrap();
        assert!(local < plan.layouts[segment].slot_count_v1());
        assert_eq!(plan.segments[segment].first_slot + local, global);
        assert_eq!(plan.segments[segment].first_plane + local / 33, global / 33);
        assert_eq!(local % 33, global % 33);
    }
    assert_eq!(plan.route_v1(233_474), Ok((0, 233_474)));
    assert_eq!(plan.route_v1(233_475), Ok((1, 0)));
    assert_eq!(plan.route_v1(306_503), Ok((1, 73_028)));
    for invalid in [306_504, 306_505, u64::MAX] {
        assert_eq!(plan.route_v1(invalid), Err(OrderedSnapshotErrorV1::Shape));
    }
}

#[test]
fn malformed_ranges_counts_contexts_and_overflow_fail_without_io() {
    let original = OrderedPlaneSpoolPlanV1::canonical_v1([0x11; 32]).unwrap();
    for index in 0..2 {
        let mut altered = original;
        altered.segments[index].first_slot += 1;
        assert_eq!(altered.validate_v1(), Err(OrderedSnapshotErrorV1::Shape));
        let mut altered = original;
        altered.segments[index].planes += 1;
        assert_eq!(altered.validate_v1(), Err(OrderedSnapshotErrorV1::Shape));
        let mut altered = original;
        altered.contexts[index][0] ^= 1;
        assert_eq!(altered.validate_v1(), Err(OrderedSnapshotErrorV1::Shape));
    }
    let mut swapped = original;
    swapped.segments.swap(0, 1);
    assert_eq!(swapped.validate_v1(), Err(OrderedSnapshotErrorV1::Shape));
    let mut count = original;
    count.total_slots -= 1;
    assert_eq!(count.validate_v1(), Err(OrderedSnapshotErrorV1::Shape));
    assert_eq!(
        OrderedPlaneSpoolPlanV1::canonical_v1([0; 32]),
        Err(OrderedSnapshotErrorV1::Context)
    );
    assert_eq!(
        OrderedPlaneSpoolPlanV1::build_v1(GeometryV1::Canonical, [1; 32], [0; 32]),
        Err(OrderedSnapshotErrorV1::Context)
    );
    assert_eq!(
        SegmentV1::new_v1(0, u64::MAX, 1),
        Err(OrderedSnapshotErrorV1::Resource)
    );
    assert_eq!(
        SegmentV1::new_v1(0, 0, u64::MAX),
        Err(OrderedSnapshotErrorV1::Resource)
    );
    assert_eq!(
        SegmentV1::new_v1(2, 0, 1),
        Err(OrderedSnapshotErrorV1::Shape)
    );
    assert_eq!(
        SegmentV1::new_v1(0, 0, 0),
        Err(OrderedSnapshotErrorV1::Shape)
    );
}

#[test]
fn independent_segment_frame_binds_context_mapping_and_every_ordered_range_word() {
    let plan =
        OrderedPlaneSpoolPlanV1::build_v1(GeometryV1::Canonical, [0x11; 32], [0x22; 32]).unwrap();
    let prefix = [1_u64, 2, 9_288, 306_504, 33, 16_384, 16];
    let segments = [
        [0_u64, 0, 7_075, 0, 233_475, 3_828_990_000],
        [1, 7_075, 2_213, 233_475, 73_029, 1_197_675_600],
    ];
    for ordinal in 0..2 {
        let reference = |context: [u8; 32], mapping: [u8; 32], words: [u64; 6]| {
            let mut hash = Keccak256::new();
            hash.update(b"iroha.zk-ams.v1.global-plane.ordered-two-spool.segment\0");
            hash.update(&context);
            hash.update(&mapping);
            for word in prefix.into_iter().chain(words) {
                hash.update(&word.to_be_bytes());
            }
            hash.finalize()
        };
        assert_eq!(
            plan.contexts[ordinal],
            reference([0x11; 32], [0x22; 32], segments[ordinal])
        );
        for axis in 0..6 {
            let mut changed = segments[ordinal];
            changed[axis] += 1;
            assert_ne!(
                plan.contexts[ordinal],
                reference([0x11; 32], [0x22; 32], changed)
            );
        }
        assert_ne!(
            plan.contexts[ordinal],
            reference([0x12; 32], [0x22; 32], segments[ordinal])
        );
        assert_ne!(
            plan.contexts[ordinal],
            reference([0x11; 32], [0x23; 32], segments[ordinal])
        );
    }
    assert_ne!(plan.contexts[0], plan.contexts[1]);
    // Independent PyCryptodome Keccak256 fixed-frame vectors.
    assert_eq!(
        plan.contexts[0],
        [
            0xf1, 0x13, 0xef, 0xfa, 0x1d, 0x75, 0xcb, 0xd3, 0xb5, 0x69, 0x78, 0x5a, 0xcd, 0x01,
            0xb0, 0x02, 0x3c, 0x43, 0x7f, 0xcb, 0x1a, 0xc3, 0x5e, 0xa1, 0x9e, 0xee, 0xa7, 0x30,
            0xdf, 0x47, 0x4f, 0xba
        ]
    );
    assert_eq!(
        plan.contexts[1],
        [
            0x65, 0x08, 0x60, 0xcb, 0x29, 0xfd, 0x20, 0xe0, 0xf3, 0x65, 0x74, 0x5e, 0x7a, 0x70,
            0xf8, 0x4c, 0x0e, 0xb6, 0x6f, 0xd7, 0x6b, 0x4d, 0x56, 0xc8, 0x31, 0x33, 0x22, 0x33,
            0xaf, 0x64, 0xa3, 0xe6
        ]
    );
    assert_eq!(
        plan.descriptor_digest_v1(),
        [
            0x62, 0x6e, 0x99, 0x09, 0xc3, 0xa7, 0x21, 0xa3, 0x8b, 0x0a, 0xfb, 0xd2, 0x49, 0x81,
            0x63, 0x62, 0x12, 0xdd, 0x53, 0x20, 0xa4, 0x54, 0x8f, 0xc2, 0x57, 0x62, 0x0b, 0xdb,
            0x3e, 0x19, 0x3e, 0x32
        ]
    );

    let digest = aggregate_digest_v1(&plan, [[0x31; 32], [0x32; 32]]).unwrap();
    assert_eq!(
        digest,
        [
            0x41, 0x58, 0x9a, 0x3a, 0x5d, 0x8f, 0xb1, 0x9e, 0xc8, 0xa8, 0x79, 0x12, 0x2c, 0xf4,
            0x4a, 0x47, 0xc4, 0x0c, 0xb6, 0xa7, 0xa8, 0xad, 0xa3, 0x4c, 0xa2, 0xc7, 0x1d, 0x81,
            0xf5, 0x98, 0x4b, 0xf3
        ]
    );
    assert_ne!(
        digest,
        aggregate_digest_v1(&plan, [[0x32; 32], [0x31; 32]]).unwrap()
    );
    assert!(aggregate_digest_v1(&plan, [[0; 32], [0x32; 32]]).is_err());
    assert!(aggregate_digest_v1(&plan, [[0x31; 32]; 2]).is_err());
}

fn chunk_v1(global: u64) -> ConfidentialSpoolChunkV1 {
    let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
    if global % 33 == 32 {
        chunk.as_mut_slice_v1()[31] = u8::try_from(global / 33 + 1).unwrap();
        chunk.as_mut_slice_v1()[32..65].copy_from_slice(
            &Point::canonical_generator()
                .unwrap()
                .to_non_identity_wire_bytes()
                .unwrap(),
        );
    } else {
        chunk.as_mut_slice_v1()[31] = u8::try_from(global + 2).unwrap();
    }
    chunk
}

#[test]
fn exact_scalar_blinding_point_and_tail_syntax_are_mandatory() {
    let value = chunk_v1(0);
    let tail = chunk_v1(32);
    validate_slot_v1(0, value.as_slice_v1()).unwrap();
    validate_slot_v1(32, tail.as_slice_v1()).unwrap();
    for scalar in [0, 255, 511] {
        let mut invalid = chunk_v1(0);
        invalid.as_mut_slice_v1()[scalar * 32..(scalar + 1) * 32]
            .copy_from_slice(&crate::vega::VEGA_T256_SCALAR_MODULUS_BE_V1);
        assert_eq!(
            validate_slot_v1(0, invalid.as_slice_v1()),
            Err(OrderedSnapshotErrorV1::Semantics)
        );
    }
    for index in [65, 8192, 16383] {
        let mut invalid = chunk_v1(32);
        invalid.as_mut_slice_v1()[index] = 1;
        assert_eq!(
            validate_slot_v1(32, invalid.as_slice_v1()),
            Err(OrderedSnapshotErrorV1::Semantics)
        );
    }
    let mut zero = chunk_v1(32);
    zero.as_mut_slice_v1()[..32].fill(0);
    assert_eq!(
        validate_slot_v1(32, zero.as_slice_v1()),
        Err(OrderedSnapshotErrorV1::Semantics)
    );
    let mut identity = chunk_v1(32);
    identity.as_mut_slice_v1()[32..65].fill(0);
    identity.as_mut_slice_v1()[32] = 0x40;
    assert_eq!(
        validate_slot_v1(32, identity.as_slice_v1()),
        Err(OrderedSnapshotErrorV1::Semantics)
    );
    assert_eq!(
        validate_slot_v1(0, &[0; 32]),
        Err(OrderedSnapshotErrorV1::Shape)
    );
}

static NEXT_DIRECTORY_V1: AtomicU64 = AtomicU64::new(0);
struct DirectoryV1(PathBuf);
impl DirectoryV1 {
    fn new_v1() -> Self {
        let path = std::env::temp_dir().join(format!(
            "iroha-two-plane-spool-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY_V1.fetch_add(1, Ordering::Relaxed)
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
fn tiny_writer_v1(directory: &Path) -> OrderedPlaneSpoolWriterV1 {
    OrderedPlaneSpoolWriterV1::create_with_plan_v1(directory, tiny_plan_v1()).unwrap()
}
fn tiny_snapshot_v1(directory: &Path) -> OrderedPlaneSpoolSnapshotV1 {
    let mut writer = tiny_writer_v1(directory);
    for slot in 0..66 {
        writer.write_slot_v1(slot, chunk_v1(slot)).unwrap();
    }
    writer.seal_v1().unwrap()
}

#[test]
#[cfg(unix)]
fn tiny_pair_authenticates_global_reads_and_never_exposes_a_half() {
    let directory = DirectoryV1::new_v1();
    let mut snapshot = tiny_snapshot_v1(&directory.0);
    assert_ne!(snapshot.snapshot_digest_v1().unwrap(), [0; 32]);
    for slot in [0, 31, 32, 33, 64, 65, 33, 0] {
        assert_eq!(
            snapshot.read_slot_v1(slot).unwrap().as_slice_v1(),
            chunk_v1(slot).as_slice_v1()
        );
    }
    assert!(directory.0.read_dir().unwrap().next().is_none());
    assert!(matches!(
        snapshot.read_slot_v1(66),
        Err(OrderedSnapshotErrorV1::Shape)
    ));
    assert!(matches!(
        snapshot.read_slot_v1(0),
        Err(OrderedSnapshotErrorV1::Poisoned)
    ));
}

#[test]
#[cfg(unix)]
fn duplicate_missing_malformed_writes_and_failed_second_seal_discard_both() {
    let directory = DirectoryV1::new_v1();
    let mut writer = tiny_writer_v1(&directory.0);
    writer.write_slot_v1(0, chunk_v1(0)).unwrap();
    assert_eq!(
        writer.write_slot_v1(0, chunk_v1(0)),
        Err(OrderedSnapshotErrorV1::Order)
    );
    assert_eq!(
        writer.write_slot_v1(1, chunk_v1(1)),
        Err(OrderedSnapshotErrorV1::Poisoned)
    );
    assert!(matches!(
        writer.seal_v1(),
        Err(OrderedSnapshotErrorV1::Poisoned)
    ));
    let mut malformed = tiny_writer_v1(&directory.0);
    assert_eq!(
        malformed.write_slot_v1(0, ConfidentialSpoolChunkV1::new_zeroed_v1(32).unwrap()),
        Err(OrderedSnapshotErrorV1::Shape)
    );
    assert_eq!(
        malformed.write_slot_v1(0, chunk_v1(0)),
        Err(OrderedSnapshotErrorV1::Poisoned)
    );
    assert!(matches!(
        tiny_writer_v1(&directory.0).seal_v1(),
        Err(OrderedSnapshotErrorV1::Order)
    ));
    // Force only the outer cursor in this private test. The genuine first leaf
    // seals, while the second's missing record causes actual leaf rejection.
    let mut incomplete = tiny_writer_v1(&directory.0);
    for slot in 0..65 {
        incomplete.write_slot_v1(slot, chunk_v1(slot)).unwrap();
    }
    incomplete.next_slot = 66;
    assert!(matches!(
        incomplete.seal_v1(),
        Err(OrderedSnapshotErrorV1::Storage)
    ));
    assert!(directory.0.read_dir().unwrap().next().is_none());
}

#[test]
#[cfg(unix)]
fn swapped_leaf_replayed_execution_and_changed_expected_context_poison_whole_pair() {
    let directory = DirectoryV1::new_v1();
    let mut swapped = tiny_snapshot_v1(&directory.0);
    swapped.live.as_mut().unwrap().swap(0, 1);
    assert!(matches!(
        swapped.read_slot_v1(0),
        Err(OrderedSnapshotErrorV1::Context)
    ));
    assert!(swapped.live.is_none());
    let mut first = tiny_snapshot_v1(&directory.0);
    let mut second = tiny_snapshot_v1(&directory.0);
    std::mem::swap(
        &mut first.live.as_mut().unwrap()[0],
        &mut second.live.as_mut().unwrap()[0],
    );
    assert!(matches!(
        first.read_slot_v1(0),
        Err(OrderedSnapshotErrorV1::Context)
    ));
    assert!(first.live.is_none());
    let mut changed = tiny_snapshot_v1(&directory.0);
    changed.plan =
        OrderedPlaneSpoolPlanV1::build_v1(GeometryV1::Tiny, [0x12; 32], [0x22; 32]).unwrap();
    changed.digest = aggregate_digest_v1(&changed.plan, changed.leaf_digests).unwrap();
    // Structural metadata matches this remade plan; the real leaf still
    // requires its original context and rejects the attempted substitution.
    assert!(matches!(
        changed.read_slot_v1(0),
        Err(OrderedSnapshotErrorV1::Storage)
    ));
    assert!(matches!(
        changed.read_slot_v1(33),
        Err(OrderedSnapshotErrorV1::Poisoned)
    ));
}

#[test]
#[cfg(unix)]
fn authenticated_but_semantically_invalid_tail_is_rejected_on_read() {
    let directory = DirectoryV1::new_v1();
    let mut writer = tiny_writer_v1(&directory.0);
    // Bypass only this module's semantic writer in its private test to retain
    // genuinely authenticated invalid bytes. Leaf authentication must not be
    // confused with the scalar/point/tail contract checked by the reader.
    for global in 0..66 {
        let (segment, local) = writer.plan.route_v1(global).unwrap();
        let mut chunk = chunk_v1(global);
        if global == 65 {
            chunk.as_mut_slice_v1()[16383] = 1;
        }
        writer.live.as_mut().unwrap()[segment]
            .write_slot_v1(local, chunk)
            .unwrap();
    }
    writer.next_slot = 66;
    let mut snapshot = writer.seal_v1().unwrap();
    assert!(matches!(
        snapshot.read_slot_v1(65),
        Err(OrderedSnapshotErrorV1::Semantics)
    ));
    assert!(matches!(
        snapshot.read_slot_v1(0),
        Err(OrderedSnapshotErrorV1::Poisoned)
    ));
}

#[test]
#[cfg(unix)]
fn writer_unwind_and_malformed_plan_return_no_owner() {
    let directory = DirectoryV1::new_v1();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _writer = tiny_writer_v1(&directory.0);
        panic!("private ownership unwind control");
    }));
    assert!(unwind.is_err());
    assert!(directory.0.read_dir().unwrap().next().is_none());
    // A malformed plan fails before either file is created. No test hook or
    // alternate crypto constructor is introduced for second-file I/O failure.
    let mut malformed = tiny_plan_v1();
    malformed.layouts.swap(0, 1);
    assert!(matches!(
        OrderedPlaneSpoolWriterV1::create_with_plan_v1(&directory.0, malformed),
        Err(OrderedSnapshotErrorV1::Shape)
    ));
    assert!(directory.0.read_dir().unwrap().next().is_none());
}
