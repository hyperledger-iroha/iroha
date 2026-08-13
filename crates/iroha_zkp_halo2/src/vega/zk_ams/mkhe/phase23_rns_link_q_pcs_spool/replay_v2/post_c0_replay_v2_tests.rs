use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use super::*;
static DIRECTORY_SEQUENCE_V2: AtomicU64 = AtomicU64::new(0);
static TEST_MODULI_V2: [u64; 2] = [97, 113];
struct TestDirectoryV2(PathBuf);
impl TestDirectoryV2 {
    fn new_v2() -> Self {
        let sequence = DIRECTORY_SEQUENCE_V2.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "iroha-q-pcs-post-c0-replay-v2-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated post-C0 replay directory");
        Self(path)
    }
}
impl Drop for TestDirectoryV2 {
    fn drop(&mut self) {
        fs::remove_dir(&self.0).expect("remove empty post-C0 replay directory");
    }
}
fn geometry_v2() -> SpoolGeometryV2 {
    SpoolGeometryV2 {
        ring_degree: 4,
        domain_log: 4,
        query_count: 4,
        coefficient_values_per_block: 2,
        lde_values_per_block: 2,
        moduli: &TEST_MODULI_V2,
    }
}
fn context_v2() -> PublicSpoolContextV2 {
    PublicSpoolContextV2 {
        sealed_source_transcript_digest: [0x31; 32],
        source_algebra_binding_digest: [0x42; 32],
    }
}
fn c0_complete_v2(directory: &TestDirectoryV2) -> QPcsC0CompleteV2 {
    let geometry = geometry_v2();
    let mut writer = QPcsSpoolWriterV2::create_with_geometry_v2(
        &directory.0,
        geometry,
        context_v2(),
        AuthenticatedReplayPermitV2::TestOnly,
    )
    .unwrap();
    for _ in 0..geometry.coefficient_slot_count_v2().unwrap() {
        writer
            .push_coefficient_block_v2(
                ConfidentialSpoolChunkV1::new_zeroed_v1(
                    geometry.coefficient_block_bytes_v2().unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let mut stage = writer.seal_coefficients_for_replay_v2().unwrap();
    let purposes = u16::from(geometry.limb_count_v2().unwrap())
        * u16::from(OPENING_REPETITIONS_V2)
        * u16::from(COEFFICIENT_COMPONENTS_V2);
    for _ in 0..purposes {
        let mut row = stage.begin_next_coefficient_row_v2().unwrap();
        for _ in 0..geometry.coefficient_blocks_per_component_v2().unwrap() {
            drop(row.read_next_block_v2().unwrap());
        }
        stage = row.complete_v2().unwrap();
    }
    for _ in 0..geometry.lde_slot_count_v2().unwrap() {
        stage
            .push_lde_block_v2(
                ConfidentialSpoolChunkV1::new_zeroed_v1(geometry.lde_block_bytes_v2().unwrap())
                    .unwrap(),
            )
            .unwrap();
    }
    let snapshot = stage.seal_lde_v2().unwrap();
    let mut c0 = snapshot.begin_c0_replay_v2().unwrap();
    for _ in 0..geometry.lde_slot_count_v2().unwrap() {
        drop(c0.read_next_block_column_v2().unwrap());
    }
    c0.complete_v2().unwrap()
}
fn exhaust_pass_v2(mut replay: PostC0CoefficientReplayV2) -> PostC0ReplayBoundaryV2 {
    let geometry = replay.geometry_v2().unwrap();
    let purposes = u16::from(geometry.limb_count_v2().unwrap())
        * u16::from(OPENING_REPETITIONS_V2)
        * u16::from(COEFFICIENT_COMPONENTS_V2);
    for purpose in 0..purposes {
        let mut row = replay.begin_next_row_v2().unwrap();
        assert_eq!(row.pair, purpose / u16::from(COEFFICIENT_COMPONENTS_V2));
        for _ in 0..geometry.coefficient_blocks_per_component_v2().unwrap() {
            assert!(
                row.read_next_block_v2()
                    .unwrap()
                    .bytes_v2()
                    .iter()
                    .all(|byte| *byte == 0)
            );
        }
        replay = row.complete_v2().unwrap();
    }
    replay.complete_v2().unwrap()
}
#[test]
fn exactly_two_complete_passes_transfer_the_only_permit() {
    let directory = TestDirectoryV2::new_v2();
    let first = exhaust_pass_v2(
        c0_complete_v2(&directory)
            .begin_post_c0_coefficient_replay_v2()
            .unwrap(),
    );
    assert_eq!(first.completed_passes, 1);
    let second = exhaust_pass_v2(first.begin_second_replay_v2().unwrap());
    assert_eq!(second.completed_passes, 2);
    let completed = second.finish_v2().unwrap();
    let (stored, permit) = completed.separate_replay_permit_v2().unwrap();
    assert_eq!(stored.geometry.ring_degree, 4);
    assert_eq!(
        stored.parameter_digest,
        parameter_digest_v2(geometry_v2()).unwrap()
    );
    assert_ne!(stored.snapshot_binding_digest, [0; 32]);
    assert!(matches!(permit, AuthenticatedReplayPermitV2::TestOnly));
}
#[test]
fn stored_c0_can_only_reopen_in_exact_block_major_order_after_full_revalidation() {
    let directory = TestDirectoryV2::new_v2();
    let first = exhaust_pass_v2(
        c0_complete_v2(&directory)
            .begin_post_c0_coefficient_replay_v2()
            .unwrap(),
    );
    let completed = exhaust_pass_v2(first.begin_second_replay_v2().unwrap())
        .finish_v2()
        .unwrap();
    let (stored, _permit) = completed.separate_replay_permit_v2().unwrap();
    let geometry = geometry_v2();
    let mut replay = stored.begin_c0_batch_replay_v2(context_v2()).unwrap();
    for block in 0..geometry.lde_blocks_per_column_v2().unwrap() {
        for column in 0..geometry.lde_column_count_v2().unwrap() {
            assert!(
                replay
                    .read_next_v2(block, u16::try_from(column).unwrap())
                    .unwrap()
                    .bytes_v2()
                    .iter()
                    .all(|byte| *byte == 0)
            );
        }
    }
    let stored = replay.complete_v2().unwrap();
    assert_eq!(stored.geometry.domain_size_v2().unwrap(), 16);
    assert_ne!(stored.snapshot_binding_digest, [0; 32]);
    let first = exhaust_pass_v2(
        c0_complete_v2(&directory)
            .begin_post_c0_coefficient_replay_v2()
            .unwrap(),
    );
    let completed = exhaust_pass_v2(first.begin_second_replay_v2().unwrap())
        .finish_v2()
        .unwrap();
    let (stored, _permit) = completed.separate_replay_permit_v2().unwrap();
    let mut replay = stored.begin_c0_batch_replay_v2(context_v2()).unwrap();
    assert!(matches!(
        replay.read_next_v2(0, 1),
        Err(QPcsSpoolErrorV2::InvalidStoragePhase)
    ));
    assert!(matches!(
        replay.read_next_v2(0, 0),
        Err(QPcsSpoolErrorV2::Poisoned)
    ));
}
#[test]
fn stored_c0_snapshot_binding_and_public_context_are_rechecked_before_reset() {
    let directory = TestDirectoryV2::new_v2();
    let first = exhaust_pass_v2(
        c0_complete_v2(&directory)
            .begin_post_c0_coefficient_replay_v2()
            .unwrap(),
    );
    let completed = exhaust_pass_v2(first.begin_second_replay_v2().unwrap())
        .finish_v2()
        .unwrap();
    let (mut stored, _permit) = completed.separate_replay_permit_v2().unwrap();
    stored.snapshot_binding_digest[0] ^= 1;
    assert!(stored.begin_c0_batch_replay_v2(context_v2()).is_err());
    let first = exhaust_pass_v2(
        c0_complete_v2(&directory)
            .begin_post_c0_coefficient_replay_v2()
            .unwrap(),
    );
    let completed = exhaust_pass_v2(first.begin_second_replay_v2().unwrap())
        .finish_v2()
        .unwrap();
    let (stored, _permit) = completed.separate_replay_permit_v2().unwrap();
    let mut wrong = context_v2();
    wrong.source_algebra_binding_digest[0] ^= 1;
    assert!(stored.begin_c0_batch_replay_v2(wrong).is_err());
}
#[test]
fn incomplete_extra_context_and_unwind_fail_closed() {
    let directory = TestDirectoryV2::new_v2();
    let replay = c0_complete_v2(&directory)
        .begin_post_c0_coefficient_replay_v2()
        .unwrap();
    assert!(matches!(
        replay.complete_v2(),
        Err(QPcsSpoolErrorV2::ReplayIncomplete)
    ));
    let replay = c0_complete_v2(&directory)
        .begin_post_c0_coefficient_replay_v2()
        .unwrap();
    let mut row = replay.begin_next_row_v2().unwrap();
    assert!(matches!(
        row.complete_v2(),
        Err(QPcsSpoolErrorV2::ReplayIncomplete)
    ));
    let replay = c0_complete_v2(&directory)
        .begin_post_c0_coefficient_replay_v2()
        .unwrap();
    let mut row = replay.begin_next_row_v2().unwrap();
    for _ in 0..geometry_v2().coefficient_blocks_per_component_v2().unwrap() {
        drop(row.read_next_block_v2().unwrap());
    }
    assert!(matches!(
        row.read_next_block_v2(),
        Err(QPcsSpoolErrorV2::ExtraCoefficientBlock)
    ));
    assert!(matches!(row.complete_v2(), Err(QPcsSpoolErrorV2::Poisoned)));
    let mut complete = c0_complete_v2(&directory);
    complete.snapshot.coefficient_context_digest[0] ^= 1;
    let mut row = complete
        .begin_post_c0_coefficient_replay_v2()
        .unwrap()
        .begin_next_row_v2()
        .unwrap();
    assert!(row.read_next_block_v2().is_err());
    assert!(matches!(
        row.read_next_block_v2(),
        Err(QPcsSpoolErrorV2::Poisoned)
    ));
    let replay = c0_complete_v2(&directory)
        .begin_post_c0_coefficient_replay_v2()
        .unwrap();
    let mut row = replay.begin_next_row_v2().unwrap();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        row.panic_after_take_for_test_v2();
    }));
    assert!(unwind.is_err());
    assert!(matches!(
        row.read_next_block_v2(),
        Err(QPcsSpoolErrorV2::Poisoned)
    ));
}
#[test]
fn one_pass_cannot_be_relabelled_complete_and_source_guards_are_pinned() {
    let directory = TestDirectoryV2::new_v2();
    let first = exhaust_pass_v2(
        c0_complete_v2(&directory)
            .begin_post_c0_coefficient_replay_v2()
            .unwrap(),
    );
    assert!(matches!(
        first.finish_v2(),
        Err(QPcsSpoolErrorV2::InvalidStoragePhase)
    ));
    let source = include_str!("post_c0_replay_v2.rs");
    let tests = include_str!("post_c0_replay_v2_tests.rs");
    assert!(source.lines().count() <= 550);
    assert!(tests.lines().count() <= 400);
    for required in [
        "completed_passes != 1",
        "completed_passes != 2",
        "if completed_passes > 2",
        "live.replay_permit",
        "coefficient_context_digest",
        "cq-post-root.context",
        "begin_c0_batch_replay_v2",
        "begin_cq_batch_replay_v2",
        "descriptor != exact",
        "next_unit != descriptor.blocks_per_column",
        "parameter_digest_v2(SpoolGeometryV2::release_v2())? != parameter_digest",
        "replay_permit: Option<AuthenticatedReplayPermitV2>",
        "pre_quotient_transcript",
    ] {
        assert!(
            source.contains(required),
            "missing post-C0 replay pin: {required}"
        );
    }
    for forbidden in ["pub struct", "pub enum", "pub fn", "pub(crate)", "pub use"] {
        assert!(
            !source.contains(forbidden),
            "forbidden post-C0 surface: {forbidden}"
        );
    }
    let stored_start = source.find("struct QPcsC0StoredV2").unwrap();
    let stored_end = source[stored_start..]
        .find("struct C0BatchReplayV2")
        .map(|offset| stored_start + offset)
        .unwrap();
    assert!(!source[stored_start..stored_end].contains("replay_permit"));
    let cq_start = source.find("struct QPcsCqStoredV2").unwrap();
    let cq_end = source[cq_start..]
        .find("struct PostC0CoefficientReplayV2")
        .map(|offset| cq_start + offset)
        .unwrap();
    assert!(!source[cq_start..cq_end].contains("replay_permit"));
}
#[test]
fn cq_batch_boundary_is_the_exact_release_descriptor_at_unit_512() {
    let parameter = parameter_digest_v2(SpoolGeometryV2::release_v2()).unwrap();
    let descriptor = cq_bound_layout_v2(
        parameter,
        REPLAY_DOMAIN_VALUES_V2,
        REPLAY_COLUMNS_V2,
        REPLAY_BLOCK_VALUES_V2,
    )
    .unwrap();
    assert!(validate_exhausted_cq_batch_boundary_v2(descriptor, 512, parameter).is_ok());
    assert!(validate_exhausted_cq_batch_boundary_v2(descriptor, 511, parameter).is_err());
    let mut wrong = descriptor;
    wrong.mapping_digest[0] ^= 1;
    assert!(validate_exhausted_cq_batch_boundary_v2(wrong, 512, parameter).is_err());
}
#[test]
fn tiny_post_root_binding_uses_the_accepted_column_transpose_order() {
    let directory = TestDirectoryV2::new_v2();
    let descriptor = cq_bound_layout_v2([0x11; 32], 4, 2, 2).unwrap();
    let context_digest = cq_post_root_context_digest_v2(
        descriptor,
        context_v2(),
        [0x11; 32],
        [0x22; 32],
        [0x33; 32],
    )
    .unwrap();
    let layout = ConfidentialSpoolLayoutV1::new_v1(
        descriptor.slot_count,
        descriptor.plaintext_bytes,
        context_digest,
    )
    .unwrap();
    let mut writer = ConfidentialSpoolWriterV1::create_in_v1(&directory.0, layout).unwrap();
    for slot in 0..descriptor.slot_count {
        let mut chunk =
            ConfidentialSpoolChunkV1::new_zeroed_v1(descriptor.plaintext_bytes).unwrap();
        chunk.as_mut_slice_v1()[..8].copy_from_slice(&slot.to_be_bytes());
        writer.write_slot_v1(slot, chunk).unwrap();
    }
    let mut replay = bind_cq_post_root_replay_v2(
        writer.seal_v1().unwrap(),
        descriptor,
        context_v2(),
        [0x11; 32],
        [0x22; 32],
        [0x33; 32],
        AuthenticatedReplayPermitV2::TestOnly,
    )
    .unwrap();
    for expected in [[0_u64, 2], [1, 3]] {
        let mut window = replay.begin_next_cq_transpose_window_v2().unwrap();
        for slot in expected {
            let chunk = window.read_next_column_v2().unwrap();
            assert_eq!(
                u64::from_be_bytes(chunk.bytes_v2()[..8].try_into().unwrap()),
                slot
            );
        }
        replay = window.complete_v2().unwrap();
    }
    assert!(matches!(
        replay.begin_next_cq_transpose_window_v2(),
        Err(QPcsSpoolErrorV2::InvalidStoragePhase)
    ));
}
