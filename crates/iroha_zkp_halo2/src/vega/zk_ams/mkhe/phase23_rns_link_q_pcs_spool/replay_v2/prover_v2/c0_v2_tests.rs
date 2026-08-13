use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use iroha_confidential_spool::ConfidentialSpoolErrorV1;
use super::*;
static DIRECTORY_SEQUENCE_V2: AtomicU64 = AtomicU64::new(0);
static TEST_MODULI_V2: [u64; 2] = [97, 113];
const RELEASE_PARAMETER_KAT_V2: [u8; 32] = [
    0xcc, 0x56, 0x91, 0x18, 0x77, 0xef, 0x83, 0xb0, 0x4c, 0x3c, 0xe8, 0x79, 0x64, 0x0f, 0x29, 0x43,
    0xce, 0xab, 0xe1, 0x3c, 0x38, 0xa7, 0x37, 0x2d, 0x5c, 0x4f, 0x69, 0x63, 0x7f, 0xe7, 0x75, 0x66,
];
const ROOT_KAT_V2: [u8; 32] = [
    0x98, 0x23, 0x45, 0xe0, 0x91, 0xf2, 0x4b, 0x71, 0x20, 0x26, 0x53, 0xf8, 0x52, 0xec, 0xc2, 0x64,
    0x80, 0x1b, 0x53, 0xac, 0x77, 0xa1, 0xdd, 0x98, 0x1f, 0x26, 0xf1, 0xbf, 0x48, 0x44, 0x87, 0x1e,
];
const COLUMN_MAPPING_KAT_V2: [u8; 32] = [
    0xa3, 0xcc, 0xfb, 0xf0, 0x10, 0x2b, 0x8f, 0x51, 0x2b, 0x57, 0xb9, 0x50, 0xbe, 0x20, 0x53, 0xb2,
    0xb9, 0xfd, 0x0b, 0xd1, 0x52, 0xfe, 0xe1, 0x29, 0x51, 0xe2, 0xe5, 0xe2, 0x59, 0x04, 0x31, 0x03,
];
const COLUMN_CONTEXT_KAT_V2: [u8; 32] = [
    0x5b, 0xbd, 0xbf, 0x04, 0xdf, 0x74, 0x07, 0xd5, 0xa9, 0x0d, 0xb2, 0x00, 0x5f, 0x7b, 0xd6, 0x20,
    0x51, 0xfc, 0xa0, 0x94, 0x0b, 0x16, 0xc8, 0xcd, 0xf8, 0xb6, 0xa1, 0x78, 0x31, 0xf4, 0x16, 0x8d,
];
struct TestDirectoryV2(PathBuf);
impl TestDirectoryV2 {
    fn new_v2() -> Self {
        let sequence = DIRECTORY_SEQUENCE_V2.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "iroha-q-pcs-c0-v2-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated C0 test directory");
        Self(path)
    }
}
impl Drop for TestDirectoryV2 {
    fn drop(&mut self) {
        fs::remove_dir(&self.0).expect("remove empty C0 test directory");
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
fn source_value_v2(limb: u8, repetition: u8, component: u8, index: usize) -> u64 {
    let base = 1 + u64::from(limb) * 20 + u64::from(repetition) * 3;
    match component {
        0 => base + index as u64,
        1 if index == 3 => 0,
        1 => base + 4 + index as u64,
        2 if index == 3 => 0,
        2 => 2 * base + index as u64,
        _ => unreachable!("fixed coefficient component"),
    }
}
fn coefficient_chunk_v2(
    geometry: SpoolGeometryV2,
    limb: u8,
    repetition: u8,
    block: u64,
    component: u8,
) -> ConfidentialSpoolChunkV1 {
    let mut chunk =
        ConfidentialSpoolChunkV1::new_zeroed_v1(geometry.coefficient_block_bytes_v2().unwrap())
            .unwrap();
    for (offset, encoded) in chunk.as_mut_slice_v1().chunks_exact_mut(8).enumerate() {
        let index = block as usize * usize::from(geometry.coefficient_values_per_block) + offset;
        encoded.copy_from_slice(&source_value_v2(limb, repetition, component, index).to_be_bytes());
    }
    chunk
}
fn sealed_v2(directory: &TestDirectoryV2) -> CoefficientsSealedV2 {
    let geometry = geometry_v2();
    let mut writer = QPcsSpoolWriterV2::create_with_geometry_v2(
        &directory.0,
        geometry,
        context_v2(),
        AuthenticatedReplayPermitV2::TestOnly,
    )
    .unwrap();
    for limb in 0..geometry.limb_count_v2().unwrap() {
        for repetition in 0..OPENING_REPETITIONS_V2 {
            for block in 0..geometry.coefficient_blocks_per_component_v2().unwrap() {
                for component in 0..COEFFICIENT_COMPONENTS_V2 {
                    writer
                        .push_coefficient_block_v2(coefficient_chunk_v2(
                            geometry, limb, repetition, block, component,
                        ))
                        .unwrap();
                }
            }
        }
    }
    CoefficientsSealedV2 {
        stage: Some(writer.seal_coefficients_for_replay_v2().unwrap()),
        masks: Some(zero_mask_spool_for_test_v2(&directory.0, geometry, context_v2()).unwrap()),
        context: context_v2(),
    }
}
fn exhaust_coefficients_v2(
    mut stage: QPcsCoefficientReplayStageV2,
) -> QPcsCoefficientReplayStageV2 {
    let purposes = u16::from(stage.geometry.limb_count_v2().unwrap())
        * u16::from(OPENING_REPETITIONS_V2)
        * u16::from(COEFFICIENT_COMPONENTS_V2);
    for _ in 0..purposes {
        let blocks = stage
            .geometry
            .coefficient_blocks_per_component_v2()
            .unwrap();
        let mut reader = stage.begin_next_coefficient_row_v2().unwrap();
        for _ in 0..blocks {
            let _chunk = reader.read_next_block_v2().unwrap();
        }
        stage = reader.complete_v2().unwrap();
    }
    stage
}
fn row_coefficients_v2(limb: u8, repetition: u8, product: bool) -> Vec<u64> {
    let mut coefficients = vec![0_u64; 16];
    if product {
        for index in 0..8 {
            coefficients[index] =
                source_value_v2(limb, repetition, if index < 4 { 0 } else { 1 }, index % 4);
        }
    } else {
        for index in 0..4 {
            coefficients[index] = source_value_v2(limb, repetition, 2, index);
        }
    }
    coefficients
}
fn schoolbook_v2(coefficients: &[u64], field: Fq2ParametersV1) -> Vec<Fq2V1> {
    (0..coefficients.len())
        .map(|index| {
            let x = field.pow(field.domain_root, index as u128);
            let mut power = Fq2V1::ONE;
            let mut value = Fq2V1::ZERO;
            for coefficient in coefficients {
                value = field.add(value, field.mul(Fq2V1::base(*coefficient), power));
                power = field.mul(power, x);
            }
            value
        })
        .collect()
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ManualMutationV2 {
    Clean,
    Framing,
    Order,
    Count,
}
fn manual_leaf_hash_v2(parameter: [u8; 32], values: &[u8], mutation: ManualMutationV2) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(if mutation == ManualMutationV2::Framing {
        b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf-X\0"
    } else {
        b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf\0"
    });
    hash.update(&[2]);
    hash.update(&parameter);
    hash.update(&[1, 0]);
    hash.update(
        &(if mutation == ManualMutationV2::Count {
            15_u32
        } else {
            16_u32
        })
        .to_be_bytes(),
    );
    hash.update(&20_u16.to_be_bytes());
    hash.update(values);
    hash.finalize()
}
fn manual_node_hash_v2(
    parameter: [u8; 32],
    height: u8,
    left: [u8; 32],
    right: [u8; 32],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-node\0");
    hash.update(&[2]);
    hash.update(&parameter);
    hash.update(&[1, 0, height]);
    hash.update(&left);
    hash.update(&right);
    hash.finalize()
}
fn manual_root_v2(mutation: ManualMutationV2) -> [u8; 32] {
    let geometry = geometry_v2();
    let parameter = parameter_digest_v2(geometry).unwrap();
    let mut rows = Vec::new();
    for limb in 0..geometry.limb_count_v2().unwrap() {
        let field = Fq2ParametersV1::derive(
            geometry.moduli[usize::from(limb)],
            usize::from(geometry.domain_log),
        )
        .unwrap();
        for repetition in 0..OPENING_REPETITIONS_V2 {
            rows.push(schoolbook_v2(
                &row_coefficients_v2(limb, repetition, true),
                field,
            ));
            rows.push(schoolbook_v2(
                &row_coefficients_v2(limb, repetition, false),
                field,
            ));
        }
    }
    let mut leaves = Vec::new();
    for index in 0..16 {
        let mut values = Vec::with_capacity(320);
        for row in &rows {
            values.extend_from_slice(&row[index].c0.to_be_bytes());
            values.extend_from_slice(&row[index].c1.to_be_bytes());
        }
        leaves.push(manual_leaf_hash_v2(parameter, &values, mutation));
    }
    if mutation == ManualMutationV2::Order {
        leaves.swap(0, 1);
    }
    let mut height = 1_u8;
    while leaves.len() > 1 {
        leaves = leaves
            .chunks_exact(2)
            .map(|pair| manual_node_hash_v2(parameter, height, pair[0], pair[1]))
            .collect();
        height += 1;
    }
    leaves[0]
}
fn marker_chunk_v2(geometry: SpoolGeometryV2, column: u16, block: u64) -> ConfidentialSpoolChunkV1 {
    let limb = usize::from(column / u16::from(FIXED_ROW_COUNT_V2));
    let modulus = geometry.moduli[limb];
    let mut chunk =
        ConfidentialSpoolChunkV1::new_zeroed_v1(geometry.lde_block_bytes_v2().unwrap()).unwrap();
    for (offset, encoded) in chunk.as_mut_slice_v1().chunks_exact_mut(16).enumerate() {
        let value = (u64::from(column) * 7 + block * 2 + offset as u64) % modulus;
        encoded[..8].copy_from_slice(&value.to_be_bytes());
        encoded[8..].copy_from_slice(&((value + 1) % modulus).to_be_bytes());
    }
    chunk
}
fn filled_column_snapshot_v2(
    directory: &TestDirectoryV2,
) -> (InitialColumnSnapshotV2, QPcsCoefficientReplayStageV2) {
    let sealed = sealed_v2(directory);
    let stage = exhaust_coefficients_v2(sealed.stage.unwrap());
    let geometry = stage.geometry;
    let mut writer = InitialColumnWriterV2::create_v2(
        &directory.0,
        geometry,
        stage.parameter_digest,
        context_v2(),
    )
    .unwrap();
    assert_eq!(writer.descriptor.mapping_digest, COLUMN_MAPPING_KAT_V2);
    assert_eq!(writer.context_digest, COLUMN_CONTEXT_KAT_V2);
    for column in 0..writer.descriptor.columns {
        for block in 0..writer.descriptor.blocks_per_column {
            writer
                .push_next_block_v2(marker_chunk_v2(geometry, column, block))
                .unwrap();
        }
    }
    (writer.seal_v2().unwrap(), stage)
}
#[test]
fn tiny_exact_ntt_matches_independent_schoolbook() {
    let field = Fq2ParametersV1::derive(97, 4).unwrap();
    let coefficients: Vec<u64> = (0..16).map(|value| value % 97).collect();
    let expected = schoolbook_v2(&coefficients, field);
    let mut buffer = ZeroizingNttBufferV2::new_v2(16).unwrap();
    assert_eq!(buffer.values.capacity(), 16);
    for (destination, value) in buffer.values.iter_mut().zip(&coefficients) {
        *destination = Fq2V1::base(*value);
    }
    ntt_in_place_v2(&mut buffer.values, field).unwrap();
    assert_eq!(buffer.values, expected);
}
#[test]
fn tiny_authenticated_transition_matches_materialized_root_kat() {
    let before_ntt = ZEROIZING_NTT_BUFFER_DROPS_V2.load(Ordering::SeqCst);
    let before_leaf = ZEROIZING_LEAF_WINDOW_DROPS_V2.load(Ordering::SeqCst);
    let directory = TestDirectoryV2::new_v2();
    let prepared = sealed_v2(&directory)
        .prepare_test_geometry_v2(&directory.0)
        .unwrap();
    assert_eq!(
        prepared.initial_root_v2(),
        manual_root_v2(ManualMutationV2::Clean)
    );
    assert_eq!(prepared.initial_root_v2(), ROOT_KAT_V2);
    assert_eq!(
        prepared.parameter_digest_v2(),
        parameter_digest_v2(geometry_v2()).unwrap()
    );
    assert_eq!(
        prepared.context.sealed_source_transcript_digest,
        context_v2().sealed_source_transcript_digest
    );
    assert_eq!(
        prepared.context.source_algebra_binding_digest,
        context_v2().source_algebra_binding_digest
    );
    assert!(prepared.accepted_c0.is_some());
    assert!(prepared.masks.is_some());
    assert!(ZEROIZING_NTT_BUFFER_DROPS_V2.load(Ordering::SeqCst) > before_ntt);
    assert!(ZEROIZING_LEAF_WINDOW_DROPS_V2.load(Ordering::SeqCst) > before_leaf);
}
#[test]
fn literal_root_oracle_rejects_framing_order_and_count_mutations() {
    let expected = manual_root_v2(ManualMutationV2::Clean);
    assert_eq!(expected, ROOT_KAT_V2);
    for mutation in [
        ManualMutationV2::Framing,
        ManualMutationV2::Order,
        ManualMutationV2::Count,
    ] {
        assert_ne!(manual_root_v2(mutation), expected);
    }
}
#[test]
fn column_stage_transposes_into_exact_block_major_order() {
    let directory = TestDirectoryV2::new_v2();
    let (snapshot, stage) = filled_column_snapshot_v2(&directory);
    let mut transpose = snapshot.begin_transpose_v2(stage, context_v2()).unwrap();
    for _ in 0..transpose.descriptor.slot_count {
        transpose.copy_next_block_v2().unwrap();
    }
    let snapshot = transpose.complete_v2().unwrap().seal_lde_v2().unwrap();
    let geometry = geometry_v2();
    let mut reader = snapshot.begin_c0_replay_v2().unwrap();
    for block in 0..geometry.lde_blocks_per_column_v2().unwrap() {
        for column in 0..u16::try_from(geometry.lde_column_count_v2().unwrap()).unwrap() {
            let chunk = reader.read_next_block_column_v2().unwrap();
            assert_eq!(
                chunk.bytes_v2(),
                marker_chunk_v2(geometry, column, block).as_slice_v1()
            );
        }
    }
    let _complete = reader.complete_v2().unwrap();
}
#[test]
fn staging_failures_poison_and_zeroizing_owners_drop() {
    let geometry = geometry_v2();
    let parameter = parameter_digest_v2(geometry).unwrap();
    let directory = TestDirectoryV2::new_v2();
    assert!(matches!(
        sealed_v2(&directory)
            .prepare_initial_c0_root_v2(&directory.0, InitialC0AuthorityV2::TestOnly),
        Err(ProverPrerequisiteErrorV2::InvalidC0Geometry)
    ));
    let missing =
        InitialColumnWriterV2::create_v2(&directory.0, geometry, parameter, context_v2()).unwrap();
    assert!(matches!(
        missing.seal_v2(),
        Err(ProverPrerequisiteErrorV2::Spool(
            QPcsSpoolErrorV2::MissingLdeBlocks
        ))
    ));
    let mut unwound =
        InitialColumnWriterV2::create_v2(&directory.0, geometry, parameter, context_v2()).unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unwound.panic_after_take_for_test_v2();
        }))
        .is_err()
    );
    assert!(matches!(
        unwound.push_next_block_v2(marker_chunk_v2(geometry, 0, 0)),
        Err(ProverPrerequisiteErrorV2::Poisoned)
    ));
    let mut extra =
        InitialColumnWriterV2::create_v2(&directory.0, geometry, parameter, context_v2()).unwrap();
    for column in 0..extra.descriptor.columns {
        for block in 0..extra.descriptor.blocks_per_column {
            extra
                .push_next_block_v2(marker_chunk_v2(geometry, column, block))
                .unwrap();
        }
    }
    assert!(matches!(
        extra.push_next_block_v2(marker_chunk_v2(geometry, 0, 0)),
        Err(ProverPrerequisiteErrorV2::Spool(
            QPcsSpoolErrorV2::ExtraLdeBlock
        ))
    ));
    assert!(matches!(
        extra.seal_v2(),
        Err(ProverPrerequisiteErrorV2::Poisoned)
    ));
    let (snapshot, stage) = filled_column_snapshot_v2(&directory);
    let wrong = PublicSpoolContextV2 {
        sealed_source_transcript_digest: [0x99; 32],
        ..context_v2()
    };
    assert!(matches!(
        snapshot.begin_transpose_v2(stage, wrong),
        Err(ProverPrerequisiteErrorV2::InvalidC0Context)
    ));
    let (snapshot, mut stage) = filled_column_snapshot_v2(&directory);
    stage.geometry.lde_values_per_block = 4;
    assert!(matches!(
        snapshot.begin_transpose_v2(stage, context_v2()),
        Err(ProverPrerequisiteErrorV2::InvalidC0Context)
    ));
    let (snapshot, stage) = filled_column_snapshot_v2(&directory);
    let mut transpose = snapshot.begin_transpose_v2(stage, context_v2()).unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            transpose.panic_after_take_for_test_v2();
        }))
        .is_err()
    );
    assert!(matches!(
        transpose.copy_next_block_v2(),
        Err(ProverPrerequisiteErrorV2::Poisoned)
    ));
    let absent = directory.0.join("absent");
    assert!(matches!(
        InitialColumnWriterV2::create_v2(&absent, geometry, parameter, context_v2()),
        Err(ProverPrerequisiteErrorV2::Spool(QPcsSpoolErrorV2::Leaf(
            ConfidentialSpoolErrorV1::FileOperation { .. }
        )))
    ));
}
#[test]
fn verifier_hash_frames_and_release_parameter_match_directly() {
    let geometry = SpoolGeometryV2::release_v2();
    let parameter = parameter_digest_v2(geometry).unwrap();
    assert_eq!(parameter, RELEASE_PARAMETER_KAT_V2);
    let soundness =
        super::super::super::super::super::v2_soundness::parameter_digest_for_spool_parity_v2();
    assert_eq!(parameter, soundness);
    let mut leaf = [0_u8; 6_080];
    for (index, byte) in leaf.iter_mut().enumerate() {
        *byte = index as u8;
    }
    let local_leaf = initial_leaf_hash_v2(parameter, 1 << 19, 380, &leaf).unwrap();
    let verifier_leaf =
        super::super::super::super::super::v2_soundness::initial_leaf_hash_for_prover_parity_v2(
            parameter,
            1 << 19,
            &leaf,
        );
    assert_eq!(local_leaf, verifier_leaf);
    let local_node = initial_node_hash_v2(parameter, 7, local_leaf, [0x55; 32]).unwrap();
    let verifier_node =
        super::super::super::super::super::v2_soundness::initial_node_hash_for_prover_parity_v2(
            parameter, 7, local_leaf, [0x55; 32],
        );
    assert_eq!(local_node, verifier_node);
}
#[test]
fn source_guards_keep_c0_private_bounded_uninhabited_and_non_authorizing() {
    let source = include_str!("c0_v2.rs");
    let storage = include_str!("c0_v2/storage_v2.rs");
    let tests = include_str!("c0_v2_tests.rs");
    assert!(source.lines().count() <= 650);
    assert!(storage.lines().count() <= 450);
    assert!(tests.lines().count() <= 550);
    for required in [
        "ntt: Infallible",
        "transpose: Infallible",
        "initial_root: Infallible",
        ".try_reserve_exact(len)",
        "values.capacity() != len",
        "INITIAL_MERKLE_FRONTIER_NODES_V2: usize = 20",
        "RELEASE_INITIAL_LEAVES_V2: usize = 1 << 19",
        "accepted_c0: Option<QPcsC0CompleteV2>",
        "INITIAL_C0_ROOT_PREPARED_V2",
    ] {
        assert!(source.contains(required), "missing C0 pin: {required}");
    }
    for forbidden in [
        "pub struct",
        "pub enum",
        "pub trait",
        "pub fn",
        "pub(crate)",
        "pub use",
        "PathBuf",
        "fn into_inner",
        "RELEASE_READY_V2: bool = true",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden C0 surface: {forbidden}"
        );
        assert!(
            !storage.contains(forbidden),
            "forbidden storage surface: {forbidden}"
        );
    }
}
