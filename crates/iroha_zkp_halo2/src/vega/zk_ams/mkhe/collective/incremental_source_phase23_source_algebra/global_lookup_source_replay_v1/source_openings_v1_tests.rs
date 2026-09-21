use super::*;
use crate::{
    generalized_bulletproof::{ProofGenerators, ProofSuite},
    vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1,
};
use std::sync::OnceLock;
const PRODUCTION_SOURCE_V1: &str = include_str!("source_openings_v1.rs");
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
    assert_eq!(TOTAL_REPLAY_IO_BYTES_V1, 350_087_424);
    assert_eq!(SOURCE_OPENING_NEW_SCALAR_MIRROR_FILE_BYTES_V1, 0);
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
        "f6ca325a2e10aeb1fc4b2348fdc92d98900c92df45968acfee35acf94fa00a77"
    );
    record.record_digest = source_opening_record_digest_v1(&record).unwrap();
    assert_eq!(
        hex::encode(record.record_digest),
        "66dee7e78fb5061f71e60e7a966691a6a471248b8c9d67cce855951d4248d07d"
    );
    validate_source_opening_record_v1(&record).unwrap();
    let mutations: [fn(&mut SourceOpeningRecordV1); 18] = [
        // Retired identities remain invalid even when the record digest is recomputed.
        |record| {
            record.topology_digest = [
                0x3a, 0xf9, 0xa6, 0xad, 0x67, 0x38, 0x3c, 0x32, 0xb0, 0x6b, 0xb5, 0xd9, 0x5a, 0x05,
                0x86, 0x3b, 0x8c, 0xb0, 0xb3, 0x33, 0x86, 0x60, 0x17, 0x7b, 0xc2, 0xa9, 0x2e, 0x1b,
                0xbf, 0x40, 0xb4, 0xab,
            ]
        },
        |record| {
            record.mapping_digest = [
                0x82, 0x16, 0x63, 0x27, 0x03, 0x17, 0x48, 0x65, 0xbc, 0xbf, 0x16, 0xb0, 0x5e, 0xd3,
                0xc8, 0xa3, 0x57, 0x1d, 0xc1, 0x16, 0x72, 0xcf, 0x5d, 0xc1, 0xc2, 0xf0, 0x0c, 0x28,
                0x8f, 0xa9, 0x12, 0xf1,
            ]
        },
        |record| record.group_count -= 1,
        |record| record.total_source_scalars -= 1,
        |record| record.pedersen_terms_per_group -= 1,
        |record| record.blinding_context_digest[0] ^= 1,
        |record| record.blinding_write_and_seal_read_bytes -= 1,
        |record| record.blinding_file_bytes -= 1,
        |record| record.current_replay_io_bytes -= 1,
        |record| record.first_pass_replay_io_bytes -= 1,
        |record| record.retained_blinding_bytes -= 1,
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
        assert!(
            !PRODUCTION_SOURCE_V1.contains(forbidden),
            "forbidden source-opening surface: {forbidden}"
        );
    }
    assert!(EXTERNAL_SOURCE_V1.contains("snapshot_identity"));
    assert!(EXTERNAL_SOURCE_V1.contains("main_snapshot_digest"));
    assert!(EXTERNAL_SOURCE_V1.contains("receipt_digest_v1"));
    assert!(PRODUCTION_SOURCE_V1.contains("SOURCE_SNAPSHOT_BINDING_RULE_V1"));
}

#[path = "source_openings_v1_mapping_tests.rs"]
mod mapping;
