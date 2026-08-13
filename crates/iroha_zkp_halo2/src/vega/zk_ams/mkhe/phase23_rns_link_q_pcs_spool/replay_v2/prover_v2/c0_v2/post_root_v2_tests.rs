use core::sync::atomic::Ordering;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering as StdOrdering},
};

use super::*;

static DIRECTORY_SEQUENCE_V2: AtomicU64 = AtomicU64::new(0);

struct TestDirectoryV2(PathBuf);

impl TestDirectoryV2 {
    fn new_v2() -> Self {
        let sequence = DIRECTORY_SEQUENCE_V2.fetch_add(1, StdOrdering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "iroha-q-pcs-post-root-v2-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated post-root directory");
        Self(path)
    }
}

impl Drop for TestDirectoryV2 {
    fn drop(&mut self) {
        fs::remove_dir(&self.0).expect("remove empty post-root directory");
    }
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

fn manual_leaf_v2(parameter: [u8; 32], length: usize, values: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf\0");
    hash.update(&[2]);
    hash.update(&parameter);
    hash.update(&[2, 0]);
    hash.update(&u32::try_from(length).unwrap().to_be_bytes());
    hash.update(&380_u16.to_be_bytes());
    hash.update(values);
    hash.finalize()
}

fn manual_node_v2(parameter: [u8; 32], height: usize, left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-node\0");
    hash.update(&[2]);
    hash.update(&parameter);
    hash.update(&[2, 0, u8::try_from(height).unwrap()]);
    hash.update(&left);
    hash.update(&right);
    hash.finalize()
}

#[test]
fn synthetic_division_and_ntt_match_independent_schoolbook_oracles() {
    let modulus = 97;
    let point = 7;
    let evaluation = 11;
    let mut coefficients = ZeroizingCoefficientBufferV2::new_v2(16).unwrap();
    coefficients.values.extend_from_slice(&[87, 72, 66, 5, 0]);
    synthesize_quotient_v2(&mut coefficients, point, evaluation, modulus, 16).unwrap();
    assert_eq!(&coefficients.values[..5], &[3, 4, 5, 0, 0]);
    for x in 0..modulus {
        let source = evaluate_coefficients_v2(&[87, 72, 66, 5, 0], x, modulus);
        let quotient = evaluate_coefficients_v2(&coefficients.values, x, modulus);
        assert_eq!(
            source,
            add_mod_v2(
                evaluation,
                ((u128::from(add_mod_v2(x, modulus - point, modulus)) * u128::from(quotient))
                    % u128::from(modulus)) as u64,
                modulus,
            )
        );
    }
    let field = Fq2ParametersV1::derive(modulus, 4).unwrap();
    let expected = schoolbook_v2(&coefficients.values, field);
    let mut ntt = ZeroizingNttBufferV2::new_v2(16).unwrap();
    load_fq2_buffer_v2(&mut ntt, &coefficients).unwrap();
    ntt_in_place_v2(&mut ntt.values, field).unwrap();
    assert_eq!(ntt.values, expected);
}

#[test]
fn synthetic_division_rejects_false_evaluation_and_missing_top_zero() {
    let mut false_evaluation = ZeroizingCoefficientBufferV2::new_v2(16).unwrap();
    false_evaluation
        .values
        .extend_from_slice(&[87, 72, 66, 5, 0]);
    assert!(matches!(
        synthesize_quotient_v2(&mut false_evaluation, 7, 12, 97, 16),
        Err(ProverPrerequisiteErrorV2::InvalidOpeningQuotient)
    ));
    let mut nonzero_top = ZeroizingCoefficientBufferV2::new_v2(16).unwrap();
    nonzero_top.values.extend_from_slice(&[1, 2, 3]);
    assert!(matches!(
        synthesize_quotient_v2(&mut nonzero_top, 7, 1, 97, 16),
        Err(ProverPrerequisiteErrorV2::InvalidSourceShape)
    ));
}

#[test]
fn quotient_frames_frontier_and_verifier_parity_are_exact() {
    let parameter = parameter_digest_v2(SpoolGeometryV2::release_v2()).unwrap();
    let mut leaves = Vec::new();
    let mut frontier = QuotientFrontierV2::new_v2(parameter);
    for leaf_index in 0..16_u8 {
        let mut values = [0_u8; 6_080];
        for (index, byte) in values.iter_mut().enumerate() {
            *byte = leaf_index.wrapping_add(index as u8);
        }
        let local = quotient_leaf_hash_v2(parameter, 16, 380, &values).unwrap();
        assert_eq!(local, manual_leaf_v2(parameter, 16, &values));
        assert_eq!(
            local,
            crate::vega::zk_ams::mkhe::phase23_rns_link::q_pcs::v2_soundness::quotient_leaf_hash_for_prover_parity_v2(
                parameter, 16, &values
            )
        );
        frontier.push_v2(local).unwrap();
        leaves.push(local);
    }
    let mut height = 1;
    while leaves.len() > 1 {
        leaves = leaves
            .chunks_exact(2)
            .map(|pair| manual_node_v2(parameter, height, pair[0], pair[1]))
            .collect();
        height += 1;
    }
    let root = frontier.finish_v2(16).unwrap();
    assert_eq!(root, leaves[0]);
    assert_eq!(
        quotient_node_hash_v2(parameter, 1, [0x31; 32], [0x42; 32]).unwrap(),
        crate::vega::zk_ams::mkhe::phase23_rns_link::q_pcs::v2_soundness::quotient_node_hash_for_prover_parity_v2(
            parameter, 1, [0x31; 32], [0x42; 32]
        )
    );
}

#[test]
fn post_root_storage_context_binds_every_public_purpose() {
    let descriptor = cq_bound_layout_v2([0x11; 32], 16, 2, 2).unwrap();
    let context = PublicSpoolContextV2 {
        sealed_source_transcript_digest: [0x31; 32],
        source_algebra_binding_digest: [0x42; 32],
    };
    let digest =
        cq_post_root_context_digest_v2(descriptor, context, [0x11; 32], [0x22; 32], [0x33; 32])
            .unwrap();
    assert!(cq_post_root_context_digest_v2(
        descriptor, context, [0x12; 32], [0x22; 32], [0x33; 32]
    )
    .is_err());
    for (root, transcript) in [([0x23; 32], [0x33; 32]), ([0x22; 32], [0x34; 32])] {
        assert_ne!(
            digest,
            cq_post_root_context_digest_v2(descriptor, context, [0x11; 32], root, transcript)
                .unwrap()
        );
    }
    let mut changed_context = context;
    changed_context.source_algebra_binding_digest[0] ^= 1;
    assert_ne!(
        digest,
        cq_post_root_context_digest_v2(
            descriptor,
            changed_context,
            [0x11; 32],
            [0x22; 32],
            [0x33; 32]
        )
        .unwrap()
    );
}

#[test]
fn cq_row_order_validation_takes_and_poison_the_writer() {
    let directory = TestDirectoryV2::new_v2();
    let descriptor = StorageLayoutDescriptorV2 {
        role: StorageRoleV2::CqColumnStage,
        layer: 0,
        logical_length: 16,
        columns: 2,
        values_per_block: 2,
        blocks_per_column: 8,
        slot_count: 16,
        plaintext_bytes: 32,
        file_bytes: 768,
        mapping_digest: [0x51; 32],
    };
    let layout = ConfidentialSpoolLayoutV1::new_v1(16, 32, [0x61; 32]).unwrap();
    let mut writer = CqColumnWriterV2 {
        writer: Some(ConfidentialSpoolWriterV1::create_in_v1(&directory.0, layout).unwrap()),
        descriptor,
        context_digest: [0x61; 32],
        next_slot: 0,
    };
    let buffer = ZeroizingNttBufferV2::new_v2(16).unwrap();
    assert!(matches!(
        writer.write_row_v2(1, &buffer),
        Err(ProverPrerequisiteErrorV2::InvalidRelationOrder)
    ));
    assert!(matches!(
        writer.write_row_v2(0, &buffer),
        Err(ProverPrerequisiteErrorV2::Poisoned)
    ));
}

#[test]
fn exact_capacity_unwind_zeroization_and_source_guards_are_pinned() {
    let before_evaluations = POST_ROOT_EVALUATION_DROPS_V2.load(Ordering::SeqCst);
    drop(ZeroizingEvaluationFrameV2::new_v2());
    assert_eq!(
        POST_ROOT_EVALUATION_DROPS_V2.load(Ordering::SeqCst),
        before_evaluations + 1
    );
    let before_coefficients = POST_ROOT_COEFFICIENT_DROPS_V2.load(Ordering::SeqCst);
    let unwind = std::panic::catch_unwind(|| {
        let mut coefficients = ZeroizingCoefficientBufferV2::new_v2(16).unwrap();
        assert_eq!(coefficients.values.capacity(), 16);
        coefficients.values.resize(16, 0xa5);
        panic!("exercise post-root coefficient unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(
        POST_ROOT_COEFFICIENT_DROPS_V2.load(Ordering::SeqCst),
        before_coefficients + 1
    );
    let before_window = POST_ROOT_CQ_WINDOW_DROPS_V2.load(Ordering::SeqCst);
    drop(ZeroizingCqWindowV2::new_v2(2, 3).unwrap());
    assert_eq!(
        POST_ROOT_CQ_WINDOW_DROPS_V2.load(Ordering::SeqCst),
        before_window + 1
    );

    let source = include_str!("post_root_v2.rs");
    let tests = include_str!("post_root_v2_tests.rs");
    assert!(source.lines().count() <= 900);
    assert!(tests.lines().count() <= 350);
    for required in [
        "QUOTIENT_FRONTIER_NODES_V2: usize = 20",
        "RELEASE_QUOTIENT_LEAVES_V2: usize = 1 << 19",
        "CQ_PRODUCT_MAX_DEGREE_V2: u64 = 262_141",
        "CQ_QUOTIENT_MAX_DEGREE_V2: u64 = 131_069",
        "CQ_FIXED_COEFFICIENT_WIDTH_V2: u64 = 524_288",
        "POST_ROOT_COEFFICIENT_REPLAY_PASSES_V2: u64 = 2",
        "POST_ROOT_CQ_SEAL_READ_BYTES_V2: u64 = 3_190_784_000",
        "POST_ROOT_TOTAL_IO_BYTES_V2: u64 = 10_770_063_360",
        "COMBINED_CQ_AND_S_TOTAL_IO_BYTES_V2: u64 = 11_169_300_480",
        "POST_ROOT_CQ_SEAL_BLOCK_READS_V2: u64 = 194_560",
        "POST_ROOT_CQ_ROOT_BLOCK_READS_V2: u64 = 194_560",
        "POST_ROOT_CQ_BLOCK_READS_V2: u64 = 389_120",
        "COMBINED_AUTHENTICATED_FILE_BYTES_V2: u64 = 7_180_042_240",
        "POST_ROOT_PEAK_EXPLICIT_HEAP_BYTES_V2: usize = 12_599_296",
        "point_schedule: Infallible",
        "accepted_c0: Option<QPcsC0StoredV2>",
        "masks: Option<MaskSpoolSealedV2>",
        "accepted_cq: Option<QPcsDerivedReplayV2>",
        "CQ_ROOT_PREPARED_V2: bool = false",
        "FRI_PROVER_COMPLETE_V2: bool = false",
        "CROSS_FIELD_MASK_PROOF_COMPLETE_V2: bool = false",
        "POST_ROOT_ZERO_KNOWLEDGE_BOUND_V2: bool = false",
        "POST_ROOT_CANONICAL_PROOF_EMITTED_V2: bool = false",
        "POST_ROOT_OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false",
        "POST_ROOT_MEASURED_RSS_WITHIN_CAP_V2: bool = false",
        "POST_ROOT_RELEASE_READY_V2: bool = false",
        "POST_ROOT_RELEASE_COMPLETE_V2: bool = false",
    ] {
        assert!(
            source.contains(required),
            "missing post-root pin: {required}"
        );
    }
    for forbidden in ["pub struct", "pub enum", "pub fn", "pub(crate)", "pub use"] {
        assert!(
            !source.contains(forbidden),
            "forbidden post-root surface: {forbidden}"
        );
    }
}
