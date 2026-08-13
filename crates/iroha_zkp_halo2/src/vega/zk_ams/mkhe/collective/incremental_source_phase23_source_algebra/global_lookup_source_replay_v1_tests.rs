use super::*;
const PRODUCTION_SOURCE_V1: &str = include_str!("global_lookup_source_replay_v1.rs");
const PARENT_SOURCE_V1: &str = include_str!("../incremental_source_phase23_source_algebra.rs");
const EXTERNAL_SOURCE_V1: &str = include_str!("../../phase23_rns_link_external_source.rs");
const LEAF_SOURCE_V1: &str =
    include_str!("../../../../../../../iroha_confidential_spool/src/lib.rs");
fn context_axes_v1() -> SourceReplayContextAxesV1 {
    SourceReplayContextAxesV1 {
        source_receipt_digest: [0x11; 32],
        prerequisite_record_digest: [0x22; 32],
        source_formula_digest: [0x33; 32],
        source_mapping_digest: [0x44; 32],
        ordered_bundle_root: [0x55; 32],
        source_lineage_root: [0x66; 32],
        output_lineage_root: [0x77; 32],
        preflight_digest: [0x88; 32],
        aggregate_schedule_digest: [0x99; 32],
    }
}
fn encoded_i64_block_v1(value: i64) -> Vec<u8> {
    let mut bytes = vec![0_u8; PHASE23_MAIN_BLOCK_BYTES_V1];
    for encoded in bytes.chunks_exact_mut(8) {
        encoded.copy_from_slice(&value.to_be_bytes());
    }
    bytes
}
fn replay_record_v1() -> GlobalLookupSourceReplayRecordV1 {
    GlobalLookupSourceReplayRecordV1 {
        source_receipt_digest: [0x11; 32],
        prerequisite_record_digest: [0x22; 32],
        topology_digest: [0x33; 32],
        plane_mapping_digest: [0x44; 32],
        spool_context_digest: [0x55; 32],
        authenticated_read_schedule_root: [0x66; 32],
        snapshot_root: [0x77; 32],
        source_read_blocks: TOTAL_SOURCE_READ_BLOCKS_V1 as u32,
        source_plaintext_read_bytes: SOURCE_PLAINTEXT_READ_BYTES_V1,
        source_authenticated_read_bytes: SOURCE_AUTHENTICATED_READ_BYTES_V1,
        output_plane_count: COMPACT_PLANE_COUNT_V1 as u16,
        output_plaintext_bytes: COMPACT_SPOOL_PLAINTEXT_BYTES_V1,
        output_file_bytes: COMPACT_SPOOL_FILE_BYTES_V1,
        output_write_and_seal_read_bytes: COMPACT_SPOOL_WRITE_AND_SEAL_READ_BYTES_V1,
        total_replay_io_bytes: TOTAL_REPLAY_IO_BYTES_V1,
        authenticated_source_replay_complete: true,
        source_same_opening_proved: false,
        global_lookup_proof_verified: false,
        zero_knowledge_accepted: false,
        operational_receipt_accepted: false,
        release_ready: false,
        release_complete: false,
        record_digest: [0; 32],
    }
}
#[test]
fn exact_read_plane_and_file_accounting_is_frozen() {
    assert_eq!(SOURCE_BLOCKS_PER_COMPACT_PLANE_V1, 16);
    assert_eq!(COMPACT_PLANES_PER_ROLE_V1, 8);
    assert_eq!(COMPACT_PLANES_PER_RECORD_V1, 24);
    assert_eq!(COMPACT_PLANE_COUNT_V1, 1_032);
    assert_eq!(CANONICAL_SOURCE_READ_BLOCKS_V1, 43 * 512);
    assert_eq!(SIGNED_SOURCE_READ_BLOCKS_V1, 43 * 3 * 128);
    assert_eq!(TOTAL_SOURCE_READ_BLOCKS_V1, 38_528);
    assert_eq!(SOURCE_PLAINTEXT_READ_BYTES_V1, 315_621_376);
    assert_eq!(SOURCE_AUTHENTICATED_READ_BYTES_V1, 316_237_824);
    assert_eq!(
        SOURCE_AUTHENTICATED_READ_BYTES_V1,
        38_528 * (8_192 + AUTHENTICATION_TAG_BYTES_V1)
    );
    assert_eq!(COMPACT_SPOOL_PLAINTEXT_BYTES_V1, 16_908_288);
    assert_eq!(COMPACT_SPOOL_FILE_BYTES_V1, 16_924_800);
    assert_eq!(COMPACT_SPOOL_WRITE_AND_SEAL_READ_BYTES_V1, 33_849_600);
    assert_eq!(
        TOTAL_REPLAY_IO_BYTES_V1,
        SOURCE_AUTHENTICATED_READ_BYTES_V1 + COMPACT_SPOOL_WRITE_AND_SEAL_READ_BYTES_V1
    );
    assert_eq!(TOTAL_REPLAY_IO_BYTES_V1, 350_087_424);
    assert!(AUTHENTICATED_SOURCE_REPLAY_COMPLETE_V1);
    assert!(!SOURCE_SAME_OPENING_PROVED_V1);
    assert!(!GLOBAL_LOOKUP_PROOF_VERIFIED_V1);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V1);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V1);
    assert!(!RELEASE_READY_V1);
    assert!(!RELEASE_COMPLETE_V1);
}
#[test]
fn replay_receipt_kat_binds_plaintext_authenticated_and_total_io_separately() {
    let mut record = replay_record_v1();
    record.record_digest = replay_record_digest_v1(&record).unwrap();
    assert_eq!(
        hex::encode(record.record_digest),
        "7bdc5d30dc1c4ec734942ffb50305ebe6b5ef106a8e24891edb0973ef3d44e10"
    );
    validate_replay_record_v1(&record).unwrap();
    let mutations: [fn(&mut GlobalLookupSourceReplayRecordV1); 5] = [
        |record: &mut GlobalLookupSourceReplayRecordV1| {
            record.source_plaintext_read_bytes += 1;
        },
        |record: &mut GlobalLookupSourceReplayRecordV1| {
            record.source_authenticated_read_bytes += 1;
        },
        |record: &mut GlobalLookupSourceReplayRecordV1| {
            record.total_replay_io_bytes += 1;
        },
        |record: &mut GlobalLookupSourceReplayRecordV1| {
            record.authenticated_source_replay_complete = false;
        },
        |record: &mut GlobalLookupSourceReplayRecordV1| {
            record.global_lookup_proof_verified = true;
        },
    ];
    for mutate in mutations {
        let mut changed = replay_record_v1();
        mutate(&mut changed);
        changed.record_digest = replay_record_digest_v1(&changed).unwrap();
        assert!(validate_replay_record_v1(&changed).is_err());
    }
}
#[test]
fn plane_coordinate_mapping_is_exact_bijective_and_bounded() {
    for slot in 0..COMPACT_PLANE_COUNT_V1 {
        let coordinate = compact_plane_coordinate_v1(slot).unwrap();
        assert_eq!(usize::from(coordinate.slot), slot);
        assert_eq!(usize::from(coordinate.record), slot / 24);
        assert_eq!(coordinate.role.index_v1(), slot % 24 / 8);
        assert_eq!(usize::from(coordinate.plane), slot % 8);
        assert_eq!(usize::from(coordinate.first_source_block), slot % 8 * 16);
    }
    assert_eq!(compact_plane_coordinate_v1(0).unwrap().role.tag_v1(), 1);
    assert_eq!(compact_plane_coordinate_v1(8).unwrap().role.tag_v1(), 2);
    assert_eq!(compact_plane_coordinate_v1(16).unwrap().role.tag_v1(), 3);
    let last = compact_plane_coordinate_v1(1_031).unwrap();
    assert_eq!(
        (
            last.record,
            last.role.tag_v1(),
            last.plane,
            last.first_source_block
        ),
        (42, 3, 7, 112)
    );
    assert!(compact_plane_coordinate_v1(1_032).is_err());
}
#[test]
fn literal_topology_mapping_and_context_kats_reject_reorder_dup_trailing_and_context() {
    assert_eq!(
        global_lookup_topology_digest_v1(),
        GLOBAL_LOOKUP_TOPOLOGY_KAT_V1
    );
    let exact: [u16; COMPACT_PLANE_COUNT_V1] = core::array::from_fn(|index| index as u16);
    let mapping = mapping_digest_for_plane_order_v1(&exact).unwrap();
    assert_eq!(
        hex::encode(mapping),
        "df87ce02f22af1a5e961cda99b1bcab0582271ead7a6aeb9f3dd113e7ffc084c"
    );
    assert_eq!(mapping, exact_mapping_digest_v1().unwrap());
    let mut reordered = exact;
    reordered.swap(0, 1);
    assert_ne!(
        mapping_digest_for_plane_order_v1(&reordered).unwrap(),
        mapping
    );
    let mut duplicated = exact;
    duplicated[1] = 0;
    assert!(mapping_digest_for_plane_order_v1(&duplicated).is_err());
    assert!(mapping_digest_for_plane_order_v1(&exact[..1_031]).is_err());
    let mut trailing = exact.to_vec();
    trailing.push(1_032);
    assert!(mapping_digest_for_plane_order_v1(&trailing).is_err());
    let context =
        spool_context_digest_v1(context_axes_v1(), mapping, GLOBAL_LOOKUP_TOPOLOGY_KAT_V1).unwrap();
    assert_eq!(
        hex::encode(context),
        "95ed1042c52ac3dad526a45fc0dfdbb40aa5152743e51d119170d07b7aed8bd4"
    );
    let mut changed = context_axes_v1();
    changed.source_receipt_digest[0] ^= 1;
    assert_ne!(
        spool_context_digest_v1(changed, mapping, GLOBAL_LOOKUP_TOPOLOGY_KAT_V1).unwrap(),
        context
    );
    let mut wrong_topology = GLOBAL_LOOKUP_TOPOLOGY_KAT_V1;
    wrong_topology[0] ^= 1;
    assert!(spool_context_digest_v1(context_axes_v1(), mapping, wrong_topology).is_err());
}
#[test]
fn signed_i64_to_i8_narrowing_is_exact_and_hostile_encodings_fail() {
    for role in CompactSourceRoleV1::ALL {
        for value in -role.bound_v1()..=role.bound_v1() {
            let source = encoded_i64_block_v1(i64::from(value));
            let mut output = [0_u8; SIGNED_SOURCE_COEFFICIENTS_PER_BLOCK_V1];
            narrow_signed_source_block_v1(&role, &source, &mut output).unwrap();
            assert!(output.iter().all(|byte| *byte == value as u8));
        }
    }
    let mut bad_extension = encoded_i64_block_v1(1);
    bad_extension[0] = 1;
    assert!(
        narrow_signed_source_block_v1(
            &CompactSourceRoleV1::Ephemeral,
            &bad_extension,
            &mut [0; SIGNED_SOURCE_COEFFICIENTS_PER_BLOCK_V1],
        )
        .is_err()
    );
    for (role, value) in [
        (CompactSourceRoleV1::Ephemeral, 2),
        (CompactSourceRoleV1::ErrorZero, 3),
        (CompactSourceRoleV1::ErrorOne, -3),
    ] {
        assert!(
            narrow_signed_source_block_v1(
                &role,
                &encoded_i64_block_v1(value),
                &mut [0; SIGNED_SOURCE_COEFFICIENTS_PER_BLOCK_V1],
            )
            .is_err()
        );
    }
    assert!(
        narrow_signed_source_block_v1(
            &CompactSourceRoleV1::ErrorOne,
            &encoded_i64_block_v1(0)[..PHASE23_MAIN_BLOCK_BYTES_V1 - 1],
            &mut [0; SIGNED_SOURCE_COEFFICIENTS_PER_BLOCK_V1],
        )
        .is_err()
    );
}
#[test]
fn canonical_scalar_blocks_reject_modulus_and_trailing_width() {
    let zero = [0_u8; PHASE23_MAIN_BLOCK_BYTES_V1];
    validate_canonical_source_block_v1(&zero).unwrap();
    let mut modulus = zero;
    modulus[..32].copy_from_slice(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    assert!(validate_canonical_source_block_v1(&modulus).is_err());
    let mut below = VEGA_T256_SCALAR_MODULUS_BE_V1;
    let last = below.last_mut().unwrap();
    *last -= 1;
    modulus[..32].copy_from_slice(&below);
    validate_canonical_source_block_v1(&modulus).unwrap();
    assert!(validate_canonical_source_block_v1(&zero[..zero.len() - 1]).is_err());
}
#[test]
fn privacy_poison_auth_sink_and_unwind_guards_are_structural() {
    for required in [
        "prerequisite: Phase23SourceAlgebraPrerequisiteV2<K, P>",
        "struct Phase23GlobalLookupSourceReplayEvidenceV1<K, P>",
        "replay: Option<Phase23GlobalLookupSourceReplayEvidenceV1<K, P>>",
        "materialization: Option<RadixWitnessMaterializationSealV2>",
        "radix_hyrax_proof: Option<RadixHyraxProofSealV2>",
        "_radix_witness_materialization: RadixWitnessMaterializationSealV2",
        "_radix_hyrax_proof: RadixHyraxProofSealV2",
        "snapshot: ConfidentialSpoolSnapshotV1",
        "confidential_spool_directory: Infallible",
        "read_canonical_plaintext_block_v1",
        "read_ephemeral_block_v1",
        "read_error_zero_block_v1",
        "read_error_one_block_v1",
        "let mut live = self\n            .live\n            .take()",
        "writer.seal_v1()",
        "global_lookup_topology_digest_v1()",
        "encoded[..7].iter().any",
        "AUTHENTICATED_READ_SCHEDULE_DOMAIN_V1",
        "SOURCE_PLAINTEXT_READ_BYTES_V1",
        "SOURCE_AUTHENTICATED_READ_BYTES_V1",
        "COMPACT_SPOOL_WRITE_AND_SEAL_READ_BYTES_V1",
        "TOTAL_REPLAY_IO_BYTES_V1",
        "source_plaintext_read_bytes: SOURCE_PLAINTEXT_READ_BYTES_V1",
        "source_authenticated_read_bytes: SOURCE_AUTHENTICATED_READ_BYTES_V1",
        "total_replay_io_bytes: TOTAL_REPLAY_IO_BYTES_V1",
        "panic_after_take_for_test_v1",
        "panic_after_authority_take_for_test_v2",
    ] {
        assert!(
            PRODUCTION_SOURCE_V1.contains(required),
            "missing guard: {required}"
        );
    }
    for forbidden in [
        "pub fn",
        "pub(crate)",
        "pub struct",
        "pub enum",
        "pub trait",
        "pub use",
        "derive(Clone",
        "impl Clone for Phase23GlobalLookupSourceReplayEvidenceV1",
        "impl Clone for Phase23GlobalLookupSourceReplayV1",
        "impl core::fmt::Debug",
        "fn into_parts",
        "fn as_bytes",
        "fn path",
        "fn key",
        "fn snapshot",
        "fn prerequisite",
        "fn record",
        "dyn Fn",
        "Serialize",
        "Deserialize",
        "Encode",
        "Decode",
        "authenticated_source_hash.update(bytes)",
        "authenticated_source_hash.update(source_bytes)",
        "authenticated_read_schedule_hash.update(bytes)",
        "authenticated_read_schedule_hash.update(source_bytes)",
    ] {
        assert!(
            !PRODUCTION_SOURCE_V1.contains(forbidden),
            "forbidden surface: {forbidden}"
        );
    }
    assert!(PARENT_SOURCE_V1.contains("fn into_global_lookup_source_replay_v1("));
    assert!(PARENT_SOURCE_V1.contains("self,"));
    assert!(EXTERNAL_SOURCE_V1.contains(".read_main_v1(slot)"));
    assert!(
        LEAF_SOURCE_V1
            .contains("let mut resources = self\n            .resources\n            .take()")
    );
    assert!(LEAF_SOURCE_V1.contains("ConfidentialSpoolErrorV1::Authentication"));
}
#[test]
fn authority_dag_is_replay_then_materialization_then_radix_hyrax_and_poisoned_before_validation() {
    let replay_entry = PRODUCTION_SOURCE_V1
        .split("fn replay_global_lookup_source_v1")
        .nth(1)
        .unwrap();
    let replay_signature = replay_entry.split('{').next().unwrap();
    assert!(replay_signature.contains("Phase23GlobalLookupSourceReplayEvidenceV1"));
    assert!(!replay_signature.contains("RadixHyraxProofSealV2"));
    let evidence = PRODUCTION_SOURCE_V1
        .split("struct Phase23GlobalLookupSourceReplayEvidenceV1")
        .nth(1)
        .unwrap()
        .split("struct ReplayRadixHyraxBindingV2")
        .next()
        .unwrap();
    assert!(!evidence.contains("RadixHyraxProofSealV2"));
    assert!(!evidence.contains("RadixWitnessMaterializationSealV2"));
    let binding = PRODUCTION_SOURCE_V1
        .split("fn finish_radix_hyrax_binding_v2")
        .nth(1)
        .unwrap()
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    let replay_take = binding.find(".replay\n            .take()").unwrap();
    let materialization_take = binding
        .find(".materialization\n            .take()")
        .unwrap();
    let proof_take = binding
        .find(".radix_hyrax_proof\n            .take()")
        .unwrap();
    let validate = binding
        .find("validate_replay_evidence_v1(&replay)")
        .unwrap();
    let materialization_validate = binding
        .find("materialization.validate_for_replay_v2")
        .unwrap();
    assert!(
        replay_take < materialization_take
            && materialization_take < proof_take
            && proof_take < validate
            && validate < materialization_validate
    );
    assert!(binding.contains("Result<Phase23GlobalLookupSourceReplayV1<K, P>"));
    assert!(!binding.contains("Ok(("));
    assert_eq!(
        PRODUCTION_SOURCE_V1
            .matches("Ok(Phase23GlobalLookupSourceReplayV1 {")
            .count(),
        1
    );
    assert!(PRODUCTION_SOURCE_V1.contains("authenticated_source_replay_complete"));
    assert!(PRODUCTION_SOURCE_V1.contains("bind_radix_hyrax_replay_after_materialization_v2"));
    assert!(!PRODUCTION_SOURCE_V1.contains("into_radix_hyrax_bound_replay_v2"));
}
