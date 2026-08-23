use super::*;

const PRODUCTION_SOURCE_V2: &str = include_str!("incremental_source_phase23_radix_range_v2.rs");
const CURSOR_SOURCE_V2: &str = include_str!(
    "incremental_source_phase23_source_algebra/global_lookup_source_replay_v1/radix_source_cursor_v2.rs"
);
const REPLAY_SOURCE_V2: &str =
    include_str!("incremental_source_phase23_source_algebra/global_lookup_source_replay_v1.rs");
const PARENT_SOURCE_V2: &str = include_str!("incremental_source_phase23.rs");
static RADIX_WITNESS_TEST_LOCK_V2: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn radix_witness_test_guard_v2() -> std::sync::MutexGuard<'static, ()> {
    RADIX_WITNESS_TEST_LOCK_V2
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn be_from_u64_v2(value: u64) -> [u8; 32] {
    let mut encoded = [0_u8; 32];
    encoded[24..].copy_from_slice(&value.to_be_bytes());
    encoded
}

fn unpack_lanes_v2(packed: &[u8; 3]) -> (u8, u8, [u8; 18], u8) {
    let b_d = packed[0] & 1;
    let b_s = (packed[0] >> 1) & 1;
    let beta = core::array::from_fn(|index| match index {
        0..=5 => (packed[0] >> (index + 2)) & 1,
        6..=13 => (packed[1] >> (index - 6)) & 1,
        14..=17 => (packed[2] >> (index - 14)) & 1,
        _ => unreachable!(),
    });
    (b_d, b_s, beta, (packed[2] >> 4) & 1)
}

#[test]
fn exact_mapping_packing_geometry_and_io_are_frozen() {
    assert_eq!(RADIX_GROUP_COUNT_V2, 43 * 8);
    assert_eq!(RADIX_WITNESS_SLOT_COUNT_V2, 1_032);
    assert_eq!(RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2, 16_384);
    assert_eq!(RADIX_WITNESS_PLAINTEXT_BYTES_V2, 16_908_288);
    assert_eq!(RADIX_WITNESS_AUTHENTICATION_TAG_BYTES_V2, 16_512);
    assert_eq!(RADIX_WITNESS_FILE_BYTES_V2, 16_924_800);
    assert_eq!(RADIX_WITNESS_SPOOL_IO_BYTES_V2, 33_849_600);
    assert_eq!(RADIX_SOURCE_REREAD_BLOCKS_V2, 43 * 512);
    assert_eq!(RADIX_SOURCE_REREAD_PLAINTEXT_BYTES_V2, 180_355_072);
    assert_eq!(RADIX_SOURCE_REREAD_TAG_BYTES_V2, 352_256);
    assert_eq!(RADIX_SOURCE_REREAD_AUTHENTICATED_BYTES_V2, 180_707_328);
    assert_eq!(RADIX_WITNESS_TOTAL_IO_BYTES_V2, 214_556_928);
    assert_eq!(RADIX_COEFFICIENT_SCRATCH_BUDGET_BYTES_V2, 384);
    assert_eq!(RADIX_WITNESS_NAMED_LIVE_PAYLOAD_BYTES_V2, 57_728);
    assert!(RADIX_WITNESS_NAMED_LIVE_PAYLOAD_BYTES_V2 <= 64 * 1_024);

    let first = radix_witness_coordinate_v2(0, 0, 0, 0).unwrap();
    assert_eq!(
        (
            first.record,
            first.family,
            first.group,
            first.source_block,
            first.coefficient,
            first.source_index,
            first.packing_index,
            first.first_slot,
        ),
        (0, 1, 0, 0, 0, 0, 0, 0)
    );
    let transpose = radix_witness_coordinate_v2(0, 0, 1, 2).unwrap();
    assert_eq!(
        (transpose.source_index, transpose.packing_index),
        (258, 129)
    );
    let last = radix_witness_coordinate_v2(42, 7, 63, 255).unwrap();
    assert_eq!(
        (
            last.family,
            last.source_index,
            last.packing_index,
            last.first_slot,
        ),
        (6, 5_636_095, 5_636_095, 1_029)
    );
    assert_eq!(radix_witness_slot_v2(42, 7, 2).unwrap(), 1_031);
    assert_eq!(radix_witness_packing_index_v2(63, 255).unwrap(), 16_383);
    assert!(radix_witness_coordinate_v2(43, 0, 0, 0).is_err());
    assert!(radix_witness_coordinate_v2(0, 8, 0, 0).is_err());
    assert!(radix_witness_coordinate_v2(0, 0, 64, 0).is_err());
    assert!(radix_witness_coordinate_v2(0, 0, 0, 256).is_err());
    assert!(radix_witness_slot_v2(0, 0, 3).is_err());
    assert!(radix_witness_packing_index_v2(64, 0).is_err());
    assert_ne!(exact_radix_witness_mapping_digest_v2().unwrap(), [0; 32]);
    let mapping_scope = PRODUCTION_SOURCE_V2
        .split("fn exact_radix_witness_mapping_digest_v2")
        .nth(1)
        .unwrap()
        .split("fn radix_witness_context_digest_v2")
        .next()
        .unwrap();
    for required in [
        "for record in 0..PHASE23_RECORD_COUNT_V1",
        "for group in 0..RADIX_GROUPS_PER_RECORD_V2",
        "for source_block in 0..RADIX_SOURCE_BLOCKS_PER_GROUP_V2",
        "for coefficient in 0..RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2",
        "radix_witness_coordinate_v2(",
        "coordinate.source_index.to_be_bytes()",
        "coordinate.packing_index.to_be_bytes()",
        "coordinate.first_slot.to_be_bytes()",
    ] {
        assert!(
            mapping_scope.contains(required),
            "incomplete mapping digest: {required}"
        );
    }
}

#[test]
fn exact_15_bit_decomposition_complement_and_centering_boundaries_hold() {
    let _guard = radix_witness_test_guard_v2();
    let mut threshold_minus_one = RADIX_CENTERING_THRESHOLD_BE_V2;
    let mut one = [0_u8; 32];
    one[31] = 1;
    let mut result = [0_u8; 32];
    let threshold_borrow = fixed_subtract_be_v2(&threshold_minus_one, &one, &mut result);
    assert_eq!(*threshold_borrow.as_ref_v2(), 0);
    threshold_minus_one = result;
    let mut two_to_255 = [0_u8; 32];
    two_to_255[0] = 0x80;
    let mut threshold_digits = [0_u16; RADIX_LOW_LIMBS_V2];
    let threshold_top =
        extract_radix_digits_v2(&RADIX_CENTERING_THRESHOLD_BE_V2, &mut threshold_digits);
    assert_eq!(*threshold_top.as_ref_v2(), 0);
    assert_eq!(
        threshold_digits,
        [
            0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0, 2_048, 0, 24_576, 32_767, 32_767,
        ]
    );
    let boundaries = [
        [0_u8; 32],
        be_from_u64_v2(1),
        threshold_minus_one,
        RADIX_CENTERING_THRESHOLD_BE_V2,
        two_to_255,
        RADIX_MODULUS_MINUS_ONE_BE_V2,
    ];
    for encoded in &boundaries {
        let witness = radix_coefficient_witness_v2(encoded).unwrap();
        let reconstructed_d = reconstruct_radix_v2(&witness.d_low, &witness.b_d).unwrap();
        let reconstructed_s = reconstruct_radix_v2(&witness.s_low, &witness.b_s).unwrap();
        let mut sum = RadixSecretBytesV2::zeroed_v2();
        let sum_carry = fixed_add_be_v2(
            reconstructed_d.as_ref_v2(),
            reconstructed_s.as_ref_v2(),
            sum.as_mut_v2(),
        );
        assert_eq!(*sum_carry.as_ref_v2(), 0);
        assert_eq!(
            *fixed_equal_bytes_v2(reconstructed_d.as_ref_v2(), encoded).as_ref_v2(),
            1
        );
        assert_eq!(
            *fixed_equal_bytes_v2(sum.as_ref_v2(), &RADIX_MODULUS_MINUS_ONE_BE_V2).as_ref_v2(),
            1
        );
        assert!(witness.d_low.iter().all(|digit| *digit < RADIX_BASE_V2));
        assert!(witness.s_low.iter().all(|digit| *digit < RADIX_BASE_V2));
        assert!(witness.b_d <= 1 && witness.b_s <= 1);
        assert_eq!(witness.b_d * witness.b_s, 0);
        assert_eq!(
            witness.beta[17],
            u8::from(*encoded < RADIX_CENTERING_THRESHOLD_BE_V2)
        );
        assert_eq!(witness.m, witness.b_d * witness.beta[16]);
        assert_eq!(witness.beta[17], witness.beta[16] - witness.m);
        let mut previous_borrow = 0_i64;
        for limb in 0..RADIX_LOW_LIMBS_V2 {
            let d = i64::from(witness.d_low[limb]);
            let k = i64::from(threshold_digits[limb]);
            let borrow = i64::from(witness.beta[limb]);
            let delta = d + i64::from(RADIX_BASE_V2) * borrow - k - previous_borrow;
            assert!((0..i64::from(RADIX_BASE_V2)).contains(&delta));
            assert_eq!(
                d - k - previous_borrow,
                delta - i64::from(RADIX_BASE_V2) * borrow
            );
            previous_borrow = borrow;
        }
    }
    assert_eq!(radix_coefficient_witness_v2(&two_to_255).unwrap().b_d, 1);
    assert!(radix_coefficient_witness_v2(&VEGA_T256_SCALAR_MODULUS_BE_V1).is_err());
}

#[test]
fn comparator_lane_mapping_zero_bits_and_transpose_are_exact() {
    let _guard = radix_witness_test_guard_v2();
    let witness = radix_coefficient_witness_v2(&be_from_u64_v2(7)).unwrap();
    let packed = pack_comparator_lanes_v2(&witness).unwrap();
    let (b_d, b_s, beta, m) = unpack_lanes_v2(packed.as_ref_v2());
    assert_eq!(
        (b_d, b_s, beta, m),
        (witness.b_d, witness.b_s, witness.beta, witness.m)
    );
    assert_eq!(packed.as_ref_v2()[2] & 0xe0, 0);

    let synthetic = RadixCoefficientWitnessV2 {
        slack: RadixSecretBytesV2::zeroed_v2(),
        d_low: [0; RADIX_LOW_LIMBS_V2],
        s_low: [0; RADIX_LOW_LIMBS_V2],
        beta: core::array::from_fn(|index| (index & 1) as u8),
        b_d: 1,
        b_s: 0,
        m: 1,
    };
    let packed = pack_comparator_lanes_v2(&synthetic).unwrap();
    assert_eq!(packed.as_ref_v2(), &[0xa9, 0xaa, 0x1a]);
    assert_eq!(packed.as_ref_v2()[2] & 0xe0, 0);
    assert_eq!(
        unpack_lanes_v2(packed.as_ref_v2()),
        (1, 0, synthetic.beta, 1)
    );

    let source: Vec<u16> = (0..2 * 3).collect();
    let packed: Vec<u16> = (0..3)
        .flat_map(|coefficient| {
            (0..2).map({
                let source = &source;
                move |block| source[block * 3 + coefficient]
            })
        })
        .collect();
    assert_eq!(packed, [0, 3, 1, 4, 2, 5]);
}

fn materialization_record_v2() -> RadixWitnessMaterializationRecordV2 {
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

#[test]
fn binding_record_seal_and_all_downstream_gates_are_strict() {
    let mut record = materialization_record_v2();
    record.record_digest = radix_witness_record_digest_v2(&record).unwrap();
    validate_radix_witness_record_v2(&record).unwrap();
    let seal = RadixWitnessMaterializationSealV2::mint_v2(
        record.replay_record_digest,
        record.spool_context_digest,
        record.snapshot_root,
        record.record_digest,
    )
    .unwrap();
    seal.validate_for_replay_v2(record.replay_record_digest)
        .unwrap();
    seal.validate_for_materialized_record_v2(&record).unwrap();
    assert!(seal.validate_for_replay_v2([0x99; 32]).is_err());
    let seal_axis_mutations: [fn(&mut RadixWitnessMaterializationRecordV2); 3] = [
        |record| record.replay_record_digest[0] ^= 1,
        |record| record.spool_context_digest[0] ^= 1,
        |record| record.snapshot_root[0] ^= 1,
    ];
    for mutate in seal_axis_mutations {
        let mut changed = materialization_record_v2();
        mutate(&mut changed);
        changed.record_digest = radix_witness_record_digest_v2(&changed).unwrap();
        assert!(seal.validate_for_materialized_record_v2(&changed).is_err());
    }
    let mut changed_record_digest = record;
    changed_record_digest.record_digest[0] ^= 1;
    assert!(
        seal.validate_for_materialized_record_v2(&changed_record_digest)
            .is_err()
    );
    let mutations: [fn(&mut RadixWitnessMaterializationRecordV2); 5] = [
        |record| record.source_reread_authenticated_bytes += 1,
        |record| record.output_authentication_tag_bytes += 1,
        |record| record.total_io_bytes += 1,
        |record| record.commitments_constructed = true,
        |record| record.authority_minted = true,
    ];
    for mutate in mutations {
        let mut changed = materialization_record_v2();
        mutate(&mut changed);
        changed.record_digest = radix_witness_record_digest_v2(&changed).unwrap();
        assert!(validate_radix_witness_record_v2(&changed).is_err());
    }
    assert!(AUTHENTICATED_CANONICAL_REREAD_COMPLETE_V2);
    assert!(COMPACT_RADIX_WITNESS_MATERIALIZED_V2);
    assert!(!COMMITMENTS_CONSTRUCTED_V2);
    assert!(!TRANSCRIPT_BOUND_V2);
    assert!(!FINAL_ARITHMETIC_PLANE_CONSTRUCTED_V2);
    assert!(!RADIX_PROOF_VERIFIED_V2);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V2);
    assert!(!AUTHORITY_MINTED_V2);
    assert!(!RSS_QUALIFIED_V2);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V2);
    assert!(!RELEASE_READY_V2);
    assert!(!RELEASE_COMPLETE_V2);
}

#[test]
fn secret_scratch_zeroizes_on_success_error_and_unwind() {
    let _guard = radix_witness_test_guard_v2();
    RADIX_SECRET_BYTE_DROPS_V2.store(0, Ordering::SeqCst);
    RADIX_COEFFICIENT_WITNESS_DROPS_V2.store(0, Ordering::SeqCst);
    RADIX_PACKED_COMPARATOR_DROPS_V2.store(0, Ordering::SeqCst);
    RADIX_SECRET_COPY_DROPS_V2.store(0, Ordering::SeqCst);
    {
        let witness = radix_coefficient_witness_v2(&be_from_u64_v2(9)).unwrap();
        let _packed = pack_comparator_lanes_v2(&witness).unwrap();
    }
    assert!(RADIX_SECRET_BYTE_DROPS_V2.load(Ordering::SeqCst) >= 5);
    assert_eq!(RADIX_COEFFICIENT_WITNESS_DROPS_V2.load(Ordering::SeqCst), 1);
    assert_eq!(RADIX_PACKED_COMPARATOR_DROPS_V2.load(Ordering::SeqCst), 1);
    assert!(RADIX_SECRET_COPY_DROPS_V2.load(Ordering::SeqCst) > 100);
    let witness_drops = RADIX_COEFFICIENT_WITNESS_DROPS_V2.load(Ordering::SeqCst);
    assert!(radix_coefficient_witness_v2(&VEGA_T256_SCALAR_MODULUS_BE_V1).is_err());
    assert_eq!(
        RADIX_COEFFICIENT_WITNESS_DROPS_V2.load(Ordering::SeqCst),
        witness_drops + 1
    );
    let before = RADIX_COEFFICIENT_WITNESS_DROPS_V2.load(Ordering::SeqCst);
    let unwind = std::panic::catch_unwind(|| {
        let _witness = radix_coefficient_witness_v2(&be_from_u64_v2(11)).unwrap();
        panic!("exercise radix witness scratch unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(
        RADIX_COEFFICIENT_WITNESS_DROPS_V2.load(Ordering::SeqCst),
        before + 1
    );
}

#[test]
fn secret_copy_source_uses_drop_owners_for_all_named_arithmetic_scratch() {
    for required in [
        "trait RadixSecretCopyValueV2: Copy",
        "fn zeroize_v2(&mut self)",
        "struct RadixSecretCopyV2<T: RadixSecretCopyValueV2>(T);",
        "impl<T: RadixSecretCopyValueV2> Drop for RadixSecretCopyV2<T>",
        "let mut borrow = RadixSecretCopyV2::new(0_u16)",
        "let left_byte = RadixSecretCopyV2::new",
        "let right_byte = RadixSecretCopyV2::new",
        "let next_borrow = RadixSecretCopyV2::new",
        "let mut carry = RadixSecretCopyV2::new(0_u16)",
        "let sum = RadixSecretCopyV2::new",
        "let mut difference = RadixSecretCopyV2::new(0_u8)",
        "let mut digit = RadixSecretCopyV2::new(0_u16)",
        "let mut prior_borrow = RadixSecretCopyV2::new(0_u16)",
        "let mut invalid = RadixSecretCopyV2::new",
        "let right = RadixSecretCopyV2::new",
        "let borrow = RadixSecretCopyV2::new",
        "let delta = RadixSecretCopyV2::new",
    ] {
        assert!(
            PRODUCTION_SOURCE_V2.contains(required),
            "missing owner guard: {required}"
        );
    }
    for forbidden in [
        "let mut borrow = 0_u16",
        "let left_byte = u16::from",
        "let right_byte = u16::from",
        "let mut carry = 0_u16",
        "let sum = u16::from",
        "let mut difference = 0_u8",
        "let mut digit = 0_u16",
        ".iter().copied()",
        "for beta in witness.beta",
        "let mut prior_borrow = 0_u16",
        "let right = threshold_low[limb]",
        "let borrow = u16::from(witness.d_low",
        "let delta = witness.d_low",
    ] {
        assert!(
            !PRODUCTION_SOURCE_V2.contains(forbidden),
            "unguarded secret copy: {forbidden}"
        );
    }
}

#[test]
fn cursor_materializer_and_authority_source_guards_forbid_bypass_and_escape() {
    for required in [
        "evidence: Option<Phase23GlobalLookupSourceReplayEvidenceV1<K, P>>",
        "read_next_canonical_block_v2",
        "read_canonical_plaintext_block_v1(record, block)?",
        "validate_canonical_source_block_v1(source.as_mut_bytes_v1())?",
        "complete_for_radix_materializer_v2",
        "schedule.finish_v2()?",
        "into_radix_witness_materialized_v2",
    ] {
        assert!(
            CURSOR_SOURCE_V2.contains(required),
            "missing cursor guard: {required}"
        );
    }
    let completion_calls = [PRODUCTION_SOURCE_V2, CURSOR_SOURCE_V2, REPLAY_SOURCE_V2]
        .into_iter()
        .map(|source| {
            source
                .matches("complete_for_radix_materializer_v2()?")
                .count()
        })
        .sum::<usize>();
    assert_eq!(completion_calls, 1);
    assert!(PRODUCTION_SOURCE_V2.contains("cursor.complete_for_radix_materializer_v2()?"));
    assert!(
        PRODUCTION_SOURCE_V2.contains("materialization_seal: RadixWitnessMaterializationSealV2")
    );
    assert!(PRODUCTION_SOURCE_V2.contains("snapshot: ConfidentialSpoolSnapshotV1"));
    assert!(PRODUCTION_SOURCE_V2.contains("confidential_spool_directory: Infallible"));
    assert!(
        REPLAY_SOURCE_V2.contains("materialization: Option<RadixWitnessMaterializationSealV2>")
    );
    assert!(REPLAY_SOURCE_V2.contains("materialization.validate_for_replay_v2"));
    assert!(REPLAY_SOURCE_V2.contains("bind_radix_hyrax_replay_after_materialization_v2"));
    assert!(PRODUCTION_SOURCE_V2.contains("fn bind_materialized_radix_hyrax_replay_v2"));
    assert!(
        PRODUCTION_SOURCE_V2
            .contains("materialized: Option<Phase23RadixWitnessMaterializedV2<K, P>>")
    );
    assert!(
        PRODUCTION_SOURCE_V2
            .contains("let materialized = self\n            .materialized\n            .take()")
    );
    assert!(PRODUCTION_SOURCE_V2.contains("let evidence = evidence\n            .take()"));
    let whole_owner_binding = PRODUCTION_SOURCE_V2
        .split("fn finish_v2")
        .nth(1)
        .unwrap()
        .split("fn materialize_radix_group_v2")
        .next()
        .unwrap();
    let materialized_take = whole_owner_binding
        .find(".materialized\n            .take()")
        .unwrap();
    let proof_take = whole_owner_binding
        .find(".radix_hyrax_proof\n            .take()")
        .unwrap();
    let evidence_take = whole_owner_binding
        .find("evidence\n            .take()")
        .unwrap();
    let record_validation = whole_owner_binding
        .find("validate_radix_witness_record_v2(&record)")
        .unwrap();
    let snapshot_validation = whole_owner_binding
        .find("snapshot.slot_count_v1()")
        .unwrap();
    let seal_validation = whole_owner_binding
        .find("materialization_seal.validate_for_materialized_record_v2(&record)")
        .unwrap();
    let replay_binding = whole_owner_binding
        .find("bind_radix_hyrax_replay_after_materialization_v2(")
        .unwrap();
    assert!(
        materialized_take < proof_take
            && proof_take < evidence_take
            && evidence_take < record_validation
            && record_validation < snapshot_validation
            && snapshot_validation < seal_validation
            && seal_validation < replay_binding
    );
    assert_eq!(
        whole_owner_binding
            .matches("bind_radix_hyrax_replay_after_materialization_v2(")
            .count(),
        1
    );
    assert!(!REPLAY_SOURCE_V2.contains("into_radix_hyrax_bound_replay_v2"));
    for source in [PRODUCTION_SOURCE_V2, CURSOR_SOURCE_V2] {
        for forbidden in [
            "derive(Clone",
            "impl Clone",
            "Serialize",
            "Deserialize",
            "Encode",
            "Decode",
            "FnOnce",
            "dyn Fn",
            "fn into_parts",
            "fn snapshot(",
            "fn evidence(",
            "fn materialization_seal(",
            "fn replay_record_digest(",
            "fn source_receipt_digest(",
            "mem::forget",
        ] {
            assert!(!source.contains(forbidden), "forbidden escape: {forbidden}");
        }
    }
    assert!(PARENT_SOURCE_V2.contains("mod radix_range_v2;"));
    assert!(!PARENT_SOURCE_V2.contains("pub mod radix_range_v2;"));
    assert!(!PARENT_SOURCE_V2.contains("RadixWitnessMaterializationSealV2"));
    let seal_scope = PRODUCTION_SOURCE_V2
        .split("struct RadixWitnessMaterializationSealV2")
        .nth(1)
        .unwrap()
        .split("fn radix_witness_seal_digest_v2")
        .next()
        .unwrap();
    assert!(seal_scope.contains("fn mint_v2("));
    assert!(!seal_scope.contains("pub fn mint_v2("));
    assert!(!seal_scope.contains("pub(super) fn mint_v2("));
}

#[test]
fn materializer_source_has_fixed_loops_exact_reads_and_no_final_plane_or_proof_surface() {
    for required in [
        "for record in 0..PHASE23_RECORD_COUNT_V1",
        "for group in 0..RADIX_GROUPS_PER_RECORD_V2",
        "for local_block in 0..RADIX_SOURCE_BLOCKS_PER_GROUP_V2",
        "for coefficient in 0..RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2",
        "for limb in 0..RADIX_LOW_LIMBS_V2",
        "ConfidentialSpoolLayoutV1::new_v1(",
        "ConfidentialSpoolWriterV1::create_in_v1",
        "writer.seal_v1()",
        "packed.0[2] & 0xe0 != 0",
        "authenticated_canonical_reread_complete: AUTHENTICATED_CANONICAL_REREAD_COMPLETE_V2",
        "compact_radix_witness_materialized: COMPACT_RADIX_WITNESS_MATERIALIZED_V2",
    ] {
        assert!(
            PRODUCTION_SOURCE_V2.contains(required),
            "missing materializer guard: {required}"
        );
    }
    assert_eq!(
        PRODUCTION_SOURCE_V2
            .matches("read_next_canonical_block_v2(")
            .count(),
        1
    );
    assert_eq!(PRODUCTION_SOURCE_V2.matches("write_slot_v1(").count(), 3);
    for forbidden in [
        "VegaT256PointV1",
        "ProverTranscript",
        "FinalArithmeticPlane",
        "proof_minted: true",
        "authority_minted: true",
        "rss_qualified: true",
        "release_complete: true",
    ] {
        assert!(
            !PRODUCTION_SOURCE_V2.contains(forbidden),
            "forbidden downstream surface: {forbidden}"
        );
    }
}
