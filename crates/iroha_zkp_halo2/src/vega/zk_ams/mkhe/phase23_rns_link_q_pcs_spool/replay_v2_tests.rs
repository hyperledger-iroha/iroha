use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use super::*;

static REPLAY_DIRECTORY_SEQUENCE_V2: AtomicU64 = AtomicU64::new(0);
static REPLAY_TEST_MODULI_V2: [u64; 2] = [97, 113];

const CQ_MAPPING_KAT_V2: [u8; 32] = [
    0x01, 0xe6, 0xd1, 0xaf, 0x4f, 0x15, 0x35, 0xa9, 0x2f, 0xf3, 0x3b, 0x68, 0x10, 0x75, 0x87, 0xef,
    0xa7, 0x3a, 0x9c, 0xce, 0xc2, 0x48, 0x54, 0x15, 0xef, 0x99, 0x78, 0xdf, 0xe8, 0x17, 0x2f, 0xbe,
];
const ROW_SCRATCH_MAPPING_KAT_V2: [u8; 32] = [
    0xc7, 0xe3, 0xba, 0x7a, 0xf6, 0x5b, 0x5c, 0x5a, 0xc6, 0xce, 0x86, 0xed, 0x2b, 0x2e, 0x28, 0xa3,
    0x9d, 0x4d, 0x18, 0x84, 0x8c, 0x55, 0x22, 0x5d, 0xe8, 0x52, 0x3c, 0x04, 0xe4, 0xc3, 0x9c, 0x8a,
];
const FRI_17_MAPPING_KAT_V2: [u8; 32] = [
    0xcc, 0xb8, 0x3e, 0x88, 0xe5, 0x67, 0x2e, 0xdd, 0x15, 0x64, 0x70, 0x1b, 0x07, 0x51, 0x7c, 0xf9,
    0x62, 0x4a, 0xf4, 0x36, 0xc9, 0x4f, 0x65, 0x01, 0x2d, 0xf5, 0xec, 0x8f, 0x5b, 0x52, 0x34, 0x2d,
];

struct ReplayDirectoryV2(PathBuf);

impl ReplayDirectoryV2 {
    fn new_v2() -> Self {
        let sequence = REPLAY_DIRECTORY_SEQUENCE_V2.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "iroha-q-pcs-replay-v2-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated replay directory");
        Self(path)
    }
}

impl Drop for ReplayDirectoryV2 {
    fn drop(&mut self) {
        fs::remove_dir(&self.0).expect("remove empty replay directory");
    }
}

fn replay_context_v2() -> PublicSpoolContextV2 {
    PublicSpoolContextV2 {
        sealed_source_transcript_digest: [0x31; 32],
        source_algebra_binding_digest: [0x32; 32],
    }
}

fn replay_geometry_v2() -> SpoolGeometryV2 {
    SpoolGeometryV2 {
        ring_degree: 4,
        domain_log: 4,
        query_count: 4,
        coefficient_values_per_block: 2,
        lde_values_per_block: 2,
        moduli: &REPLAY_TEST_MODULI_V2,
    }
}

#[cfg(unix)]
fn coefficient_stage_v2(directory: &ReplayDirectoryV2) -> QPcsCoefficientReplayStageV2 {
    let geometry = replay_geometry_v2();
    let mut writer = QPcsSpoolWriterV2::create_with_geometry_v2(
        &directory.0,
        geometry,
        replay_context_v2(),
        AuthenticatedReplayPermitV2::TestOnly,
    )
    .expect("create replay writer");
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
    writer.seal_coefficients_for_replay_v2().unwrap()
}

#[test]
fn exact_storage_equations_mappings_and_kats_are_frozen() {
    let parameter = [0x42; 32];
    let cq = cq_column_layout_v2(parameter).unwrap();
    assert_eq!(
        (cq.slot_count, cq.plaintext_bytes, cq.file_bytes),
        (194_560, 16_384, 3_190_784_000)
    );
    assert_eq!(cq.mapping_digest, CQ_MAPPING_KAT_V2);
    assert_eq!(fixed_row_column_v2(0, 0, LdeRowRoleV2::Product), Ok(0));
    assert_eq!(fixed_row_column_v2(37, 4, LdeRowRoleV2::Quotient), Ok(379));
    assert_eq!(
        fixed_row_column_v2(38, 0, LdeRowRoleV2::Product),
        Err(QPcsSpoolErrorV2::InvalidReplayPurpose)
    );

    let axes = RowScratchAxesV2 {
        limb: 37,
        repetition: 4,
        role: LdeRowRoleV2::Quotient,
        pass: 1,
        orientation: ScratchOrientationV2::Columns,
        tile: 511,
    };
    let scratch = row_scratch_layout_v2(parameter, axes).unwrap();
    assert_eq!(
        (
            scratch.slot_count,
            scratch.plaintext_bytes,
            scratch.file_bytes
        ),
        (512, 16_384, 8_396_800)
    );
    assert_eq!(scratch.mapping_digest, ROW_SCRATCH_MAPPING_KAT_V2);
    assert_ne!(
        row_scratch_layout_v2(
            parameter,
            RowScratchAxesV2 {
                orientation: ScratchOrientationV2::Rows,
                ..axes
            }
        )
        .unwrap()
        .mapping_digest,
        scratch.mapping_digest
    );
    assert_eq!(
        row_scratch_layout_v2(parameter, RowScratchAxesV2 { pass: 2, ..axes }),
        Err(QPcsSpoolErrorV2::InvalidReplayPurpose)
    );

    let mut total = 0_u64;
    for (layer, expected_file) in FRI_RELEASE_FILES_V2.iter().enumerate() {
        let layout = fri_layer_layout_v2(parameter, u8::try_from(layer).unwrap()).unwrap();
        assert_eq!(layout.logical_length, REPLAY_DOMAIN_VALUES_V2 >> layer);
        assert_eq!(layout.file_bytes, *expected_file);
        total += layout.file_bytes;
        if layer == 17 {
            assert_eq!(layout.mapping_digest, FRI_17_MAPPING_KAT_V2);
            assert_eq!(
                (
                    layout.logical_length,
                    layout.values_per_block,
                    layout.slot_count
                ),
                (4, 4, 380)
            );
        }
    }
    assert_eq!(total, REPLAY_FRI_TOTAL_FILE_BYTES_V2);
    assert_eq!(
        fri_layer_layout_v2(parameter, 18),
        Err(QPcsSpoolErrorV2::InvalidFriLayer)
    );
    assert_ne!(
        cq_column_layout_v2([0x43; 32]).unwrap().mapping_digest,
        cq.mapping_digest
    );
}

#[cfg(unix)]
#[test]
fn coefficient_replay_phase_order_error_and_unwind_are_fail_closed() {
    let directory = ReplayDirectoryV2::new_v2();
    let geometry = replay_geometry_v2();

    let mut while_lde_open = coefficient_stage_v2(&directory);
    while_lde_open
        .push_lde_block_v2(
            ConfidentialSpoolChunkV1::new_zeroed_v1(geometry.lde_block_bytes_v2().unwrap())
                .unwrap(),
        )
        .unwrap();
    let mut reader = while_lde_open.begin_next_coefficient_row_v2().unwrap();
    assert!(
        reader
            .read_next_block_v2()
            .unwrap()
            .bytes_v2()
            .iter()
            .all(|byte| *byte == 0)
    );
    assert!(matches!(
        reader.complete_v2(),
        Err(QPcsSpoolErrorV2::ReplayIncomplete)
    ));

    assert!(matches!(
        coefficient_stage_v2(&directory).seal_lde_v2(),
        Err(QPcsSpoolErrorV2::ReplayIncomplete)
    ));

    let mut bad_context = coefficient_stage_v2(&directory);
    bad_context.coefficient_context_digest = [0x99; 32];
    let mut reader = bad_context.begin_next_coefficient_row_v2().unwrap();
    assert!(matches!(
        reader.read_next_block_v2(),
        Err(QPcsSpoolErrorV2::Leaf(
            ConfidentialSpoolErrorV1::ContextDigestMismatch
        ))
    ));
    assert!(matches!(
        reader.read_next_block_v2(),
        Err(QPcsSpoolErrorV2::Poisoned)
    ));

    let mut reader = coefficient_stage_v2(&directory)
        .begin_next_coefficient_row_v2()
        .unwrap();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        reader.panic_after_take_for_test_v2();
    }));
    assert!(unwind.is_err());
    assert!(matches!(
        reader.read_next_block_v2(),
        Err(QPcsSpoolErrorV2::Poisoned)
    ));
}

#[cfg(unix)]
fn derived_snapshot_v2(
    role: StorageRoleV2,
    logical_length: u64,
    columns: u16,
    values_per_block: u16,
) -> QPcsDerivedReplayV2 {
    let descriptor = checked_layout_v2(
        role,
        0,
        logical_length,
        columns,
        values_per_block,
        [0x51; 32],
    )
    .unwrap();
    let context_digest = derived_context_digest_v2(descriptor, replay_context_v2()).unwrap();
    let layout = ConfidentialSpoolLayoutV1::new_v1(
        descriptor.slot_count,
        descriptor.plaintext_bytes,
        context_digest,
    )
    .unwrap();
    let directory = ReplayDirectoryV2::new_v2();
    let mut writer = ConfidentialSpoolWriterV1::create_in_v1(&directory.0, layout).unwrap();
    for slot in 0..descriptor.slot_count {
        let mut chunk =
            ConfidentialSpoolChunkV1::new_zeroed_v1(descriptor.plaintext_bytes).unwrap();
        chunk.as_mut_slice_v1()[..8].copy_from_slice(&slot.to_be_bytes());
        writer.write_slot_v1(slot, chunk).unwrap();
    }
    let snapshot = writer.seal_v1().unwrap();
    assert_eq!(
        context_digest,
        derived_context_digest_v2(descriptor, replay_context_v2()).unwrap()
    );
    bind_derived_replay_v2(
        snapshot,
        descriptor,
        replay_context_v2(),
        AuthenticatedReplayPermitV2::TestOnly,
    )
    .unwrap()
}

fn encoded_slot_v2(chunk: &AuthenticatedReplayChunkV2) -> u64 {
    u64::from_be_bytes(chunk.bytes_v2()[..8].try_into().unwrap())
}

#[cfg(unix)]
#[test]
fn transpose_and_fold_readers_derive_only_the_exact_next_coordinates() {
    assert!(matches!(
        derived_snapshot_v2(StorageRoleV2::CqColumnStage, 4, 2, 2)
            .begin_next_cq_transpose_window_v2()
            .unwrap()
            .complete_v2(),
        Err(QPcsSpoolErrorV2::ReplayIncomplete)
    ));
    let mut owner = derived_snapshot_v2(StorageRoleV2::CqColumnStage, 4, 2, 2);
    for expected in [[0, 2], [1, 3]] {
        let mut window = owner.begin_next_cq_transpose_window_v2().unwrap();
        for slot in expected {
            assert_eq!(
                encoded_slot_v2(&window.read_next_column_v2().unwrap()),
                slot
            );
        }
        owner = window.complete_v2().unwrap();
    }
    assert!(matches!(
        owner.begin_next_cq_transpose_window_v2(),
        Err(QPcsSpoolErrorV2::InvalidStoragePhase)
    ));

    let mut owner = derived_snapshot_v2(StorageRoleV2::FriLayer, 8, 2, 2);
    for expected in [[(0, 4), (2, 6)], [(1, 5), (3, 7)]] {
        let mut fold = owner.begin_next_fri_fold_column_v2().unwrap();
        for (lower, upper) in expected {
            let pair = fold.read_next_pair_v2().unwrap();
            assert_eq!(encoded_slot_v2(&pair.lower), lower);
            assert_eq!(encoded_slot_v2(pair.upper.as_ref().unwrap()), upper);
            assert_eq!(pair.values_per_half, 2);
        }
        owner = fold.complete_v2().unwrap();
    }
    assert!(matches!(
        owner.begin_next_fri_fold_column_v2(),
        Err(QPcsSpoolErrorV2::InvalidStoragePhase)
    ));

    let mut fold = derived_snapshot_v2(StorageRoleV2::FriLayer, 2, 1, 2)
        .begin_next_fri_fold_column_v2()
        .unwrap();
    let pair = fold.read_next_pair_v2().unwrap();
    assert_eq!(encoded_slot_v2(&pair.lower), 0);
    assert!(pair.upper.is_none());
    assert_eq!(pair.values_per_half, 1);
    assert!(fold.complete_v2().is_ok());
}

#[test]
fn authenticated_replay_chunks_zeroize_on_drop() {
    let before = REPLAY_CHUNK_ZEROIZED_DROPS_V2.load(Ordering::SeqCst);
    let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16).unwrap();
    chunk.as_mut_slice_v1().fill(0xa5);
    drop(AuthenticatedReplayChunkV2 { chunk });
    assert!(REPLAY_CHUNK_ZEROIZED_DROPS_V2.load(Ordering::SeqCst) > before);
}
