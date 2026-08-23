use super::*;
use crate::{
    generalized_bulletproof::{ProofGenerators, ProofSuite},
    vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1,
};
use std::sync::OnceLock;
const PRODUCTION_SOURCE_V1: &str = include_str!("source_openings_v1.rs");
const CANONICAL_REOPEN_SOURCE_V1: &str = include_str!("source_openings_v1/canonical_reopen_v1.rs");
const REPLAY_SOURCE_V1: &str = include_str!("../global_lookup_source_replay_v1.rs");
const EXTERNAL_SOURCE_V1: &str = include_str!("../../../phase23_rns_link_external_source.rs");
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TinySourceOpeningSuiteV1;
impl ProofSuite for TinySourceOpeningSuiteV1 {
    type Scalar = Scalar;
    type Point = Point;
    fn generators() -> &'static ProofGenerators<Self> {
        static GENERATORS: OnceLock<ProofGenerators<TinySourceOpeningSuiteV1>> = OnceLock::new();
        GENERATORS.get_or_init(|| {
            let point = Point::canonical_generator().expect("canonical test generator");
            ProofGenerators::new(point, point, vec![point; 4], vec![point; 4])
                .expect("tiny fixed source-opening basis")
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IdentitySourceOpeningSuiteV1;
impl ProofSuite for IdentitySourceOpeningSuiteV1 {
    type Scalar = Scalar;
    type Point = Point;
    fn generators() -> &'static ProofGenerators<Self> {
        static GENERATORS: OnceLock<ProofGenerators<IdentitySourceOpeningSuiteV1>> =
            OnceLock::new();
        GENERATORS.get_or_init(|| {
            let point = Point::canonical_generator().expect("canonical test generator");
            ProofGenerators::new(point, point, vec![point], vec![point])
                .expect("identity hostile basis")
        })
    }
}
fn context_axes_v1() -> SourceOpeningContextAxesV1 {
    SourceOpeningContextAxesV1 {
        source_receipt_digest: [0x11; 32],
        prerequisite_record_digest: [0x22; 32],
        replay_spool_context_digest: [0x33; 32],
    }
}
fn opening_record_v1() -> SourceOpeningRecordV1 {
    let mapping_digest = exact_source_opening_mapping_digest_v1().unwrap();
    let blinding_context_digest = source_opening_blinding_context_digest_v1(
        [0x33; 32],
        mapping_digest,
        ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
    )
    .unwrap();
    SourceOpeningRecordV1 {
        source_receipt_digest: [0x11; 32],
        prerequisite_record_digest: [0x22; 32],
        topology_digest: GLOBAL_LOOKUP_TOPOLOGY_KAT_V1,
        mapping_digest,
        basis_digest: ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
        context_digest: [0x33; 32],
        blinding_context_digest,
        commitments_root: [0x44; 32],
        blinding_snapshot_root: [0x55; 32],
        group_count: SOURCE_OPENING_GROUP_COUNT_V1 as u16,
        scalars_per_group: SOURCE_OPENING_SCALARS_PER_GROUP_V1 as u32,
        total_source_scalars: SOURCE_OPENING_SCALAR_COUNT_V1,
        pedersen_terms_per_group: SOURCE_OPENING_PEDERSEN_TERMS_PER_GROUP_V1 as u32,
        retained_blinding_bytes: SOURCE_OPENING_RETAINED_BLINDING_BYTES_V1,
        public_point_wire_bytes: SOURCE_OPENING_PUBLIC_POINT_WIRE_BYTES_V1,
        first_pass_replay_io_bytes: TOTAL_REPLAY_IO_BYTES_V1,
        blinding_file_bytes: SOURCE_OPENING_BLINDING_FILE_BYTES_V1,
        blinding_write_and_seal_read_bytes: SOURCE_OPENING_BLINDING_WRITE_AND_SEAL_READ_BYTES_V1,
        current_replay_io_bytes: SOURCE_OPENING_CURRENT_REPLAY_IO_BYTES_V1,
        later_canonical_plaintext_bytes: CANONICAL_REOPEN_PLAINTEXT_BYTES_V1,
        later_canonical_authenticated_read_bytes: CANONICAL_REOPEN_AUTHENTICATED_READ_BYTES_V1,
        total_lifecycle_io_bytes: SOURCE_OPENING_LIFECYCLE_IO_BYTES_V1,
        new_scalar_mirror_file_bytes: SOURCE_OPENING_NEW_SCALAR_MIRROR_FILE_BYTES_V1,
        source_opening_materialized: true,
        source_same_opening_proved: false,
        packing_same_opening_proved: false,
        global_lookup_proof_verified: false,
        zero_knowledge_accepted: false,
        authority_accepted: false,
        operational_receipt_accepted: false,
        rss_gate_accepted: false,
        release_ready: false,
        release_complete: false,
        record_digest: [0; 32],
    }
}
#[test]
fn exact_group_commitment_and_corrected_io_accounting_are_frozen() {
    assert_eq!(SOURCE_OPENING_GROUPS_PER_RECORD_V1, 8);
    assert_eq!(SOURCE_OPENING_BLOCKS_PER_GROUP_V1, 64);
    assert_eq!(SOURCE_OPENING_SCALARS_PER_BLOCK_V1, 256);
    assert_eq!(SOURCE_OPENING_SCALARS_PER_GROUP_V1, 16_384);
    assert_eq!(SOURCE_OPENING_GROUP_COUNT_V1, 43 * 8);
    assert_eq!(SOURCE_OPENING_PEDERSEN_TERMS_PER_GROUP_V1, 16_384 + 1);
    assert_eq!(SOURCE_OPENING_SCALAR_COUNT_V1, 5_636_096);
    assert_eq!(SOURCE_OPENING_RETAINED_BLINDING_BYTES_V1, 344 * 32);
    assert_eq!(SOURCE_OPENING_PUBLIC_POINT_WIRE_BYTES_V1, 344 * 33);
    assert_eq!(SOURCE_OPENING_BLINDING_FILE_BYTES_V1, 344 * (32 + 16));
    assert_eq!(SOURCE_OPENING_BLINDING_WRITE_AND_SEAL_READ_BYTES_V1, 33_024);
    assert_eq!(SOURCE_OPENING_CURRENT_REPLAY_IO_BYTES_V1, 350_120_448);
    assert_eq!(CANONICAL_REOPEN_BLOCK_COUNT_V1, 22_016);
    assert_eq!(CANONICAL_REOPEN_PLAINTEXT_BYTES_V1, 180_355_072);
    assert_eq!(CANONICAL_REOPEN_AUTHENTICATED_READ_BYTES_V1, 180_707_328);
    assert_eq!(TOTAL_REPLAY_IO_BYTES_V1, 350_087_424);
    assert_eq!(SOURCE_OPENING_LIFECYCLE_IO_BYTES_V1, 530_827_776);
    assert_eq!(SOURCE_OPENING_NEW_SCALAR_MIRROR_FILE_BYTES_V1, 0);
    assert_eq!(WEIGHTED_COLUMN_NAMED_HEAP_BYTES_V1, 1_067_776);
    assert!(WEIGHTED_COLUMN_NAMED_HEAP_BYTES_V1 < 2_700_000);
}
#[test]
fn source_and_inverse_packing_coordinates_are_exact_bijections() {
    let mut seen = [false; SOURCE_OPENING_SCALARS_PER_GROUP_V1];
    for block in 0..SOURCE_OPENING_BLOCKS_PER_GROUP_V1 {
        for coefficient in 0..SOURCE_OPENING_SCALARS_PER_BLOCK_V1 {
            let source_j = 256 * block + coefficient;
            let packing_k = source_to_packing_coordinate_v1(source_j).unwrap();
            assert_eq!(packing_k, 64 * coefficient + block);
            assert!(!seen[packing_k]);
            seen[packing_k] = true;
        }
    }
    assert!(!seen.contains(&false));
    assert!(source_to_packing_coordinate_v1(16_384).is_err());
    for ordinal in 0..SOURCE_OPENING_GROUP_COUNT_V1 {
        let coordinate = source_opening_group_coordinate_v1(ordinal).unwrap();
        assert_eq!(usize::from(coordinate.record), ordinal / 8);
        assert_eq!(usize::from(coordinate.group), ordinal % 8);
    }
    assert!(source_opening_group_coordinate_v1(344).is_err());
}
#[test]
fn mapping_and_context_kats_reject_order_duplicates_and_wrong_axes() {
    let groups: [u16; SOURCE_OPENING_GROUP_COUNT_V1] = core::array::from_fn(|index| index as u16);
    let source: [u16; SOURCE_OPENING_SCALARS_PER_GROUP_V1] =
        core::array::from_fn(|index| index as u16);
    let mapping = source_opening_mapping_digest_for_orders_v1(&groups, &source).unwrap();
    assert_eq!(mapping, exact_source_opening_mapping_digest_v1().unwrap());
    assert_eq!(
        hex::encode(mapping),
        "8216632703174865bcbf16b05ed3c8a3571dc11672cf5dc1c2f00c288fa912f1"
    );
    let mut reordered_groups = groups;
    reordered_groups.swap(0, 1);
    assert_ne!(
        source_opening_mapping_digest_for_orders_v1(&reordered_groups, &source).unwrap(),
        mapping
    );
    let mut duplicate_groups = groups;
    duplicate_groups[1] = 0;
    assert!(source_opening_mapping_digest_for_orders_v1(&duplicate_groups, &source).is_err());
    let mut reordered_source = source;
    reordered_source.swap(0, 1);
    assert_ne!(
        source_opening_mapping_digest_for_orders_v1(&groups, &reordered_source).unwrap(),
        mapping
    );
    let mut duplicate_source = source;
    duplicate_source[1] = 0;
    assert!(source_opening_mapping_digest_for_orders_v1(&groups, &duplicate_source).is_err());
    assert!(source_opening_mapping_digest_for_orders_v1(&groups[..343], &source).is_err());
    assert!(source_opening_mapping_digest_for_orders_v1(&groups, &source[..16_383]).is_err());
    let context = source_opening_context_digest_v1(
        &context_axes_v1(),
        GLOBAL_LOOKUP_TOPOLOGY_KAT_V1,
        mapping,
        ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
    )
    .unwrap();
    assert_eq!(
        hex::encode(context),
        "3be10e51876d3927bfb58d02f3f3450ff989b89c146bf71cce69d059718d54d7"
    );
    let mut changed = context_axes_v1();
    changed.source_receipt_digest[0] ^= 1;
    assert_ne!(
        source_opening_context_digest_v1(
            &changed,
            GLOBAL_LOOKUP_TOPOLOGY_KAT_V1,
            mapping,
            ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
        )
        .unwrap(),
        context
    );
    let mut wrong_basis = ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1;
    wrong_basis[0] ^= 1;
    assert!(
        source_opening_context_digest_v1(
            &context_axes_v1(),
            GLOBAL_LOOKUP_TOPOLOGY_KAT_V1,
            mapping,
            wrong_basis,
        )
        .is_err()
    );
}
#[test]
fn tiny_commitment_kat_uses_secret_msm_and_identity_is_rejected() {
    let values = [
        Scalar::from_u64(1),
        Scalar::from_u64(2),
        Scalar::from_u64(3),
        Scalar::from_u64(4),
    ];
    let commitment = source_opening_commitment_for_suite_v1::<TinySourceOpeningSuiteV1>(
        &values,
        &Scalar::from_u64(5),
        4,
    )
    .unwrap();
    let generator = Point::canonical_generator().unwrap();
    assert!(commitment.equals(&generator.mul_scalar(Scalar::from_u64(15))));
    let encoded = SecretT256PointEncodingV1::new(commitment.expose_ref()).unwrap();
    assert_eq!(
        encoded.as_ref(),
        &generator
            .mul_scalar(Scalar::from_u64(15))
            .to_non_identity_wire_bytes()
            .unwrap()
    );
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    {
        let mut hostile = ZeroizingT256ScalarVecV1::with_capacity(1);
        hostile.push(Scalar::one());
        assert!(
            source_opening_commitment_for_suite_v1::<IdentitySourceOpeningSuiteV1>(
                hostile.as_slice(),
                &-Scalar::one(),
                1,
            )
            .is_err()
        );
    }
    assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
    assert!(
        source_opening_commitment_for_suite_v1::<TinySourceOpeningSuiteV1>(
            &values,
            &Scalar::zero(),
            4,
        )
        .is_err()
    );
    assert!(
        source_opening_commitment_for_suite_v1::<TinySourceOpeningSuiteV1>(
            &values[..2],
            &Scalar::one(),
            4,
        )
        .is_err()
    );
}
#[test]
fn canonical_parse_errors_fail_closed() {
    let mut destination = ZeroizingT256ScalarVecV1::with_capacity(256);
    append_canonical_block_scalars_v1(&mut destination, &[0; PHASE23_MAIN_BLOCK_BYTES_V1]).unwrap();
    assert_eq!(destination.len(), 256);
    let mut noncanonical = [0_u8; PHASE23_MAIN_BLOCK_BYTES_V1];
    noncanonical[..32].copy_from_slice(&crate::vega::VEGA_T256_SCALAR_MODULUS_BE_V1);
    assert!(append_canonical_block_scalars_v1(&mut destination, &noncanonical).is_err());
    assert!(
        append_canonical_block_scalars_v1(&mut destination, &[0; PHASE23_MAIN_BLOCK_BYTES_V1 - 1],)
            .is_err()
    );
}
#[test]
fn receipt_kat_and_mutations_keep_every_proof_authority_and_release_gate_false() {
    let mut record = opening_record_v1();
    assert_eq!(
        hex::encode(record.blinding_context_digest),
        "fac01296c402faacf1803533792d57a8134410face578ae21081c8a91a61d9af"
    );
    record.record_digest = source_opening_record_digest_v1(&record).unwrap();
    assert_eq!(
        hex::encode(record.record_digest),
        "af5d0d2832b2dfb03d485e219e86fb33de1cf85c2298a80b9c8309a91db90a66"
    );
    validate_source_opening_record_v1(&record).unwrap();
    let mutations: [fn(&mut SourceOpeningRecordV1); 16] = [
        |record| record.group_count -= 1,
        |record| record.total_source_scalars -= 1,
        |record| record.pedersen_terms_per_group -= 1,
        |record| record.blinding_context_digest[0] ^= 1,
        |record| record.blinding_write_and_seal_read_bytes -= 1,
        |record| record.blinding_file_bytes -= 1,
        |record| record.current_replay_io_bytes -= 1,
        |record| record.later_canonical_authenticated_read_bytes -= 1,
        |record| record.total_lifecycle_io_bytes -= 1,
        |record| record.new_scalar_mirror_file_bytes = 1,
        |record| record.source_same_opening_proved = true,
        |record| record.packing_same_opening_proved = true,
        |record| record.zero_knowledge_accepted = true,
        |record| record.authority_accepted = true,
        |record| record.operational_receipt_accepted = true,
        |record| record.release_complete = true,
    ];
    for mutate in mutations {
        let mut changed = opening_record_v1();
        mutate(&mut changed);
        changed.record_digest = source_opening_record_digest_v1(&changed).unwrap();
        assert!(validate_source_opening_record_v1(&changed).is_err());
    }
}
#[test]
fn private_sink_accepts_zero_weights_rejects_bad_shape_and_has_no_coordinate_arguments() {
    let mut zero_weights = ZeroizingT256ScalarVecV1::with_capacity(SOURCE_OPENING_GROUP_COUNT_V1);
    for _ in 0..SOURCE_OPENING_GROUP_COUNT_V1 {
        zero_weights.push(Scalar::zero());
    }
    let zero_sink = WeightedOpeningColumnsSinkV1::from_seal_v1(
        GlobalLookupCanonicalReopenSealV1::TestOnly(zero_weights),
    )
    .unwrap();
    assert_eq!(zero_sink.group_weights.len(), 344);
    assert!(
        zero_sink
            .group_weights
            .as_slice()
            .iter()
            .copied()
            .all(Scalar::is_zero)
    );
    let mut short_weights =
        ZeroizingT256ScalarVecV1::with_capacity(SOURCE_OPENING_GROUP_COUNT_V1 - 1);
    for _ in 0..SOURCE_OPENING_GROUP_COUNT_V1 - 1 {
        short_weights.push(Scalar::one());
    }
    assert!(
        WeightedOpeningColumnsSinkV1::from_seal_v1(GlobalLookupCanonicalReopenSealV1::TestOnly(
            short_weights,
        ))
        .is_err()
    );
    let sink = WeightedOpeningColumnsSinkV1::from_seal_v1(
        GlobalLookupCanonicalReopenSealV1::deterministic_test_v1(),
    )
    .unwrap();
    assert_eq!(sink.group_weights.len(), 344);
    assert_eq!(sink.source_column.len(), 16_384);
    assert_eq!(sink.packing_column.len(), 16_384);
    let trait_source = PRODUCTION_SOURCE_V1
        .split("trait PurposeBoundCanonicalOpeningSinkV1")
        .nth(1)
        .unwrap()
        .split("struct WeightedOpeningColumnsSinkV1")
        .next()
        .unwrap();
    assert!(trait_source.contains("scalar: &ZeroizingT256ScalarCopyV1"));
    assert!(!trait_source.contains("record:"));
    assert!(!trait_source.contains("group:"));
    assert!(!trait_source.contains("block:"));
    assert!(!trait_source.contains("index:"));
}
#[test]
fn source_identity_write_seal_poison_and_privacy_guards_are_structural() {
    for required in [
        "ZkAmsT256BulletproofSuiteV1::generators()",
        ".reduce(SOURCE_OPENING_SCALARS_PER_GROUP_V1)",
        "SecretMultiexpBuilder::<S>::new(exact_values + 1)",
        "Result<SecretPoint<Point>, ZkAmsMkheErrorV1>",
        "SecretT256PointEncodingV1::new(commitment.expose_ref())",
        "adopt_source_commitment_v1(",
        "commitment.expose_ref(),",
        "live.commitments.push(*commitment.expose_ref())",
        "push(value, generator)",
        "values.iter().zip(generators.g_bold)",
        "push(blinding.as_ref(), &generators.h)",
        "let blinding = ZeroizingT256ScalarCopyV1::new(*blinding);",
        "let mut live = self\n            .live\n            .take()",
        "panic_after_take_for_test_v1",
        "source_opening_materialized: SOURCE_OPENING_MATERIALIZED_V1",
        "post_rho_verifier_weights: Infallible",
        "PurposeBoundCanonicalOpeningSinkV1",
        "SOURCE_OPENING_NEW_SCALAR_MIRROR_FILE_BYTES_V1",
        "ConfidentialSpoolLayoutV1::new_v1(",
        "ConfidentialSpoolWriterV1::create_in_v1(directory, blinding_layout)",
        ".write_slot_v1(u64::from(coordinate.ordinal), blinding_chunk)",
        "blinding_writer.seal_v1()",
        "blinding_snapshot: ConfidentialSpoolSnapshotV1",
        "!= self.record.blinding_snapshot_root",
        "Scalar::from_be_bytes_exact_ref(encoded)",
        "proof_session\n                .sample_source_blinding_v1(u32::from(coordinate.ordinal))",
    ] {
        assert!(
            PRODUCTION_SOURCE_V1.contains(required),
            "missing source-opening guard: {required}"
        );
    }
    let publication = PRODUCTION_SOURCE_V1
        .split_once("let encoded = SecretT256PointEncodingV1::new(commitment.expose_ref())")
        .expect("source commitment encoding")
        .1
        .split_once("live.group_scalars.clear_and_truncate(0);")
        .expect("source commitment publication boundary")
        .0;
    let hash = publication
        .find("live.commitment_hash.update(encoded.as_ref());")
        .expect("borrowed source commitment hash");
    let adopt = publication
        .find("adopt_source_commitment_v1(")
        .expect("borrowed source commitment adoption");
    let public_copy = publication
        .find("live.commitments.push(*commitment.expose_ref());")
        .expect("public source commitment copy");
    assert!(hash < adopt && adopt < public_copy);
    assert!(!PRODUCTION_SOURCE_V1.contains("let mut blinding = *blinding;"));
    assert!(!PRODUCTION_SOURCE_V1.contains("Result<Point, ZkAmsMkheErrorV1>"));
    assert!(!PRODUCTION_SOURCE_V1.contains("commitment.to_non_identity_wire_bytes()"));
    for required in [
        "let mut replay = self\n            .replay\n            .take()",
        "read_canonical_plaintext_block_v1(record, block)",
        "source_receipt_digest != replay.record.source_receipt_digest",
        "after_source_receipt != source_receipt_digest",
    ] {
        assert!(
            CANONICAL_REOPEN_SOURCE_V1.contains(required),
            "missing canonical-reopen guard: {required}"
        );
    }
    for required in [
        "validate_canonical_source_block_v1(bytes)?;",
        ".absorb_next_canonical_block_v1(record, block, bytes)?;",
        "writer.seal_v1()",
        "openings: GlobalLookupSourceOpeningMaterialV1",
    ] {
        assert!(
            REPLAY_SOURCE_V1.contains(required),
            "missing replay guard: {required}"
        );
    }
    for forbidden in [
        "CpackCommitment",
        "cpack_commitments",
        ".push(blinding.get(), generators.h)",
        ".iter()\n        .copied()\n        .zip(generators.g_bold.iter().copied())",
        "packing_commitments: Vec",
        "dyn Fn",
        "derive(Clone",
        "impl Clone for GlobalLookupSourceOpeningMaterialV1",
        "fn into_parts",
        "fn as_bytes",
        "fn plaintext",
        "fn path",
        "fn key",
        "Vec<Scalar>",
        "Serialize",
        "Deserialize",
        "Encode",
        "Decode",
    ] {
        for source in [PRODUCTION_SOURCE_V1, CANONICAL_REOPEN_SOURCE_V1] {
            assert!(
                !source.contains(forbidden),
                "forbidden source-opening surface: {forbidden}"
            );
        }
    }
    assert!(EXTERNAL_SOURCE_V1.contains("snapshot_identity"));
    assert!(EXTERNAL_SOURCE_V1.contains("main_snapshot_digest"));
    assert!(EXTERNAL_SOURCE_V1.contains("receipt_digest_v1"));
    assert!(PRODUCTION_SOURCE_V1.contains("SOURCE_SNAPSHOT_BINDING_RULE_V1"));
}
