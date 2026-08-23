//! Candidate tests for the undeclared Offline Cash V2 compact-SHA source.
//!
//! This file is also undeclared and therefore records no compiler or execution evidence by
//! itself.  Its lightweight counting path deliberately avoids materializing the 932,944-row
//! fixed-batch trace.

use std::collections::HashMap;

use halo2_base::halo2_proofs::{
    dev::MockProver,
    halo2curves::{
        ff::PrimeField,
        pasta::{Fp, Fq},
    },
    plonk::ConstraintSystem,
    poly::Rotation,
};
use sha2::{Digest as _, Sha256};

use super::{
    compact_sha::{
        COMPACT_SHA_ABI_BINDING_ROWS_V2, COMPACT_SHA_BATCH_REQUIRED_ROWS_V2,
        COMPACT_SHA_BATCH_ROW_EXCESS_V2, COMPACT_SHA_BATCH_SHA_ROWS_V2,
        COMPACT_SHA_COMPRESSION_ROWS_PER_BLOCK_V2, COMPACT_SHA_FEED_FORWARD_ROWS_PER_BLOCK_V2,
        COMPACT_SHA_PROOF_SHAPE_V2, COMPACT_SHA_REJECTED_NON_BLOCK_ROWS_V2,
        COMPACT_SHA_REJECTED_PACKED_BLOCK_ROWS_V2, COMPACT_SHA_REJECTED_PACKED_ROWS_PER_BLOCK_V2,
        COMPACT_SHA_REJECTED_SOURCE_BOUND_V2, COMPACT_SHA_ROWS_PER_BLOCK_V2,
        COMPACT_SHA_SCHEDULE_ROWS_PER_BLOCK_V2, COMPACT_SHA_SPREAD_WIDTHS_V2,
        COMPACT_SHA_TABLE_ROWS_V2, COMPACT_SHA_TYPED_SPREAD_ROWS_V2, CompactShaCircuitV2,
        CompactShaConfigV2, CompactShaDiagnosticCircuitV2, CompactShaFailureV2, CompactShaRowsV2,
        compact_sha_counting_audit_v2, compact_sha_padded_blocks_v2, compact_sha_table_entries_v2,
    },
    compact_sha_abi::{
        COMPACT_SHA_ARTIFACT_EVIDENCE_AVAILABLE_V2, COMPACT_SHA_BATCH_FINAL_ZERO_WORDS_V2,
        COMPACT_SHA_BATCH_INSTANCE_CELLS_V2, COMPACT_SHA_BATCH_MACHINE_SOURCE_IMPLEMENTED_V2,
        COMPACT_SHA_BATCH_ROW_QUALIFIED_V2, COMPACT_SHA_BATCH_WORDS_V2,
        COMPACT_SHA_CANONICALITY_TARGET_V2, COMPACT_SHA_COMPILE_EVIDENCE_AVAILABLE_V2,
        COMPACT_SHA_FIXED_BLOCKS_V2, COMPACT_SHA_FIXED_MESSAGE_BLOCKS_V2,
        COMPACT_SHA_FIXED_MESSAGE_BYTES_V2, COMPACT_SHA_FIXED_MESSAGE_TOTAL_BYTES_V2,
        COMPACT_SHA_HELPER_WORDS_V2, COMPACT_SHA_K_V2, COMPACT_SHA_PRODUCTION_AVAILABLE_V2,
        COMPACT_SHA_PUBLIC_ABI_REVISION_V2, COMPACT_SHA_RAW_TBS_AGGREGATE_ABI_IMPLEMENTED_V2,
        COMPACT_SHA_RAW_TBS_CIRCUIT_IMPLEMENTED_V2, COMPACT_SHA_RAW_TBS_FINAL_ZERO_WORDS_V2,
        COMPACT_SHA_RAW_TBS_INSTANCE_CELLS_V2, COMPACT_SHA_RAW_TBS_MAX_BLOCKS_V2,
        COMPACT_SHA_RAW_TBS_MAX_BYTES_V2, COMPACT_SHA_RAW_TBS_PAYLOAD_CODEC_IMPLEMENTED_V2,
        COMPACT_SHA_RAW_TBS_WORDS_V2, COMPACT_SHA_RECURSIVE_ADAPTER_AVAILABLE_V2,
        COMPACT_SHA_RELEASE_ELIGIBLE_V2, COMPACT_SHA_SOURCE_HELPER_K_V1,
        COMPACT_SHA_TRANSCRIPT_TARGET_V2, COMPACT_SHA_USABLE_ROWS_V2, CompactShaAbiErrorV2,
        CompactShaBatchPublicAbiV2, CompactShaPublicModeV2, CompactShaRawTbsContractV2,
        compact_sha_source_helper_protocol_digest_v1,
    },
};
use crate::zk::offline_cash_v1::{
    OfflineCashHalo2CircuitRoleV1, OfflineCashHalo2ParityV1,
    offline_cash_halo2_protocol_identity_v1,
};
use crate::zk::pasta_ipa_recursion::{PastaIpaInstanceQueryV1, pasta_ipa_augmented_proof_shape_v1};

const DIGEST_OFFSETS: [usize; 20] = [
    24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128, 136, 144, 152, 160, 168, 176,
];
const JOB_DIGEST_OFFSETS: [usize; 9] = [88, 96, 112, 104, 120, 128, 152, 168, 176];

fn fixture_parts() -> ([u32; COMPACT_SHA_HELPER_WORDS_V2], [u8; 65], [u8; 65]) {
    let mut words = [1_u32; COMPACT_SHA_HELPER_WORDS_V2];
    words[2] = COMPACT_SHA_SOURCE_HELPER_K_V1;
    words[3] = 1;
    words[4] = 2;
    words[5] = 1;
    words[6] = 1;
    words[7] = 8;
    words[8] = 41;
    words[9] = 0;
    words[10] = 42;
    words[11] = 0;
    words[12] = 21;
    words[13] = 7;
    words[14] = 27;
    words[15] = 0;
    for (digest, offset) in DIGEST_OFFSETS.into_iter().enumerate() {
        for word in 0..8 {
            words[offset + word] = u32::try_from(1 + digest * 8 + word).expect("fixture fits u32");
        }
    }
    write_digest(
        &mut words,
        16,
        compact_sha_source_helper_protocol_digest_v1(1, 2)
            .expect("Eq GuardUse V1 protocol identity is pinned"),
    );
    let mut platform = [0_u8; 65];
    let mut issuer = [0_u8; 65];
    platform[0] = 4;
    issuer[0] = 4;
    for index in 1..65 {
        platform[index] = u8::try_from(index).expect("fixture index fits u8");
        issuer[index] = u8::try_from(255 - index).expect("fixture index fits u8");
    }
    (words, platform, issuer)
}

fn fixture_abi() -> CompactShaBatchPublicAbiV2 {
    let (words, platform, issuer) = fixture_parts();
    CompactShaBatchPublicAbiV2::new(words, platform, issuer).expect("valid compact-SHA fixture")
}

fn write_digest(words: &mut [u32; COMPACT_SHA_HELPER_WORDS_V2], offset: usize, digest: [u8; 32]) {
    for (index, bytes) in digest.chunks_exact(4).enumerate() {
        words[offset + index] = u32::from_le_bytes(bytes.try_into().expect("four-byte chunk"));
    }
}

fn sequential_kat_abi() -> CompactShaBatchPublicAbiV2 {
    let (mut words, platform, issuer) = fixture_parts();
    for (job, offset) in JOB_DIGEST_OFFSETS.into_iter().enumerate() {
        let abi = CompactShaBatchPublicAbiV2::new(words, platform, issuer)
            .expect("intermediate sequential fixture is valid");
        let messages = abi.fixed_messages().expect("fixed framing is valid");
        let digest: [u8; 32] = Sha256::digest(&messages[job]).into();
        words = *abi.helper_words();
        write_digest(&mut words, offset, digest);
    }
    CompactShaBatchPublicAbiV2::new(words, platform, issuer)
        .expect("final sequential fixture is valid")
}

fn assert_configured_shape<F: halo2_base::utils::BigPrimeField>() {
    let mut meta = ConstraintSystem::<F>::default();
    let _ = CompactShaConfigV2::configure(&mut meta);
    assert_eq!(meta.degree(), 7);
    assert_eq!(meta.num_advice_columns(), 8);
    assert_eq!(meta.num_instance_columns(), 1);
    assert_eq!(meta.num_fixed_columns(), 4);
    assert_eq!(meta.num_selectors(), 0);
    assert_eq!(meta.advice_queries().len(), 8);
    assert_eq!(meta.instance_queries().len(), 1);
    assert_eq!(meta.fixed_queries().len(), 4);
    assert_eq!(meta.permutation().get_columns().len(), 8);
    assert_eq!(meta.lookups().len(), 2);
    assert_eq!(meta.blinding_factors() + 1, 9);
    assert!(
        meta.advice_queries()
            .iter()
            .all(|(_, rotation)| *rotation == Rotation::cur())
    );
    assert!(
        meta.instance_queries()
            .iter()
            .all(|(_, rotation)| *rotation == Rotation::cur())
    );
    assert!(
        meta.fixed_queries()
            .iter()
            .all(|(_, rotation)| *rotation == Rotation::cur())
    );
    let shape = pasta_ipa_augmented_proof_shape_v1(
        &meta,
        COMPACT_SHA_K_V2,
        PastaIpaInstanceQueryV1::Direct,
    )
    .expect("shared proof-shape calculator accepts compact SHA");
    assert_eq!(COMPACT_SHA_PROOF_SHAPE_V2.k, shape.k());
    assert_eq!(COMPACT_SHA_PROOF_SHAPE_V2.k, COMPACT_SHA_K_V2);
    assert_eq!(shape.instance_query(), PastaIpaInstanceQueryV1::Direct);
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.advice_columns,
        shape.advice_columns()
    );
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.advice_queries,
        meta.advice_queries().len()
    );
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.instance_columns,
        shape.instance_columns()
    );
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.instance_queries,
        meta.instance_queries().len()
    );
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.fixed_columns,
        meta.num_fixed_columns()
    );
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.fixed_queries,
        meta.fixed_queries().len()
    );
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.selector_columns,
        meta.num_selectors()
    );
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.lookup_arguments,
        meta.lookups().len()
    );
    assert_eq!(COMPACT_SHA_PROOF_SHAPE_V2.maximum_degree, shape.degree());
    assert_eq!(COMPACT_SHA_PROOF_SHAPE_V2.rotations, 1);
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.equality_columns,
        meta.permutation().get_columns().len()
    );
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.permutation_chunks,
        shape.permutation_chunks()
    );
    assert_eq!(COMPACT_SHA_PROOF_SHAPE_V2.point_sets, shape.point_sets());
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.point_elements,
        shape.commitments()
    );
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.scalar_elements,
        shape.evaluations()
    );
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.raw_bytes,
        (shape.commitments() + shape.evaluations()) * 32
    );
    assert_eq!(
        COMPACT_SHA_PROOF_SHAPE_V2.augmented_bytes,
        usize::try_from(shape.augmented_proof_bytes()).expect("proof bytes fit usize")
    );
}

#[test]
fn configured_shape_is_exact_in_both_pasta_fields() {
    assert_configured_shape::<Fp>();
    assert_configured_shape::<Fq>();
}

#[test]
fn fixed_batch_row_ledger_is_exact_and_fail_closed() {
    assert_eq!(COMPACT_SHA_SCHEDULE_ROWS_PER_BLOCK_V2, 3_072);
    assert_eq!(COMPACT_SHA_COMPRESSION_ROWS_PER_BLOCK_V2, 12_288);
    assert_eq!(COMPACT_SHA_FEED_FORWARD_ROWS_PER_BLOCK_V2, 256);
    assert_eq!(COMPACT_SHA_ROWS_PER_BLOCK_V2, 15_616);
    assert_eq!(COMPACT_SHA_FIXED_BLOCKS_V2, 59);
    assert_eq!(COMPACT_SHA_BATCH_SHA_ROWS_V2, 921_344);
    assert_eq!(COMPACT_SHA_ABI_BINDING_ROWS_V2, 11_600);
    assert_eq!(COMPACT_SHA_BATCH_REQUIRED_ROWS_V2, 932_944);
    assert_eq!(COMPACT_SHA_BATCH_ROW_EXCESS_V2, 801_881);
    assert_eq!(COMPACT_SHA_REJECTED_PACKED_ROWS_PER_BLOCK_V2, 2_127);
    assert_eq!(COMPACT_SHA_REJECTED_PACKED_BLOCK_ROWS_V2, 125_493);
    assert_eq!(COMPACT_SHA_REJECTED_NON_BLOCK_ROWS_V2, 4_690);
    assert_eq!(COMPACT_SHA_REJECTED_SOURCE_BOUND_V2, 130_183);
    assert_eq!(COMPACT_SHA_USABLE_ROWS_V2, 131_063);
    let rows = CompactShaRowsV2::fixed_batch();
    assert_eq!(rows.assigned, COMPACT_SHA_BATCH_REQUIRED_ROWS_V2);
    assert_eq!(rows.lookup_table, 93_716);
    assert!(!rows.fits());

    let circuit = CompactShaCircuitV2::<Fp>::new(fixture_abi());
    assert_eq!(
        circuit.preflight(),
        Err(CompactShaFailureV2::RowLimit {
            required: 932_944,
            available: 131_063,
        })
    );

    let audit = compact_sha_counting_audit_v2::<Fp>(&fixture_abi())
        .expect("lightweight circuit routing/counting audit");
    assert_eq!(audit.materialized_abi_rows, 11_600);
    assert_eq!(audit.counted_schedule_rows, 181_248);
    assert_eq!(audit.counted_compression_rows, 724_992);
    assert_eq!(audit.counted_feed_forward_rows, 15_104);
    assert_eq!(audit.counted_assigned_rows, 932_944);
    assert_eq!(audit.lookup_table_rows, 93_716);
}

fn local_spread(value: u64, width: usize) -> u64 {
    (0..width).fold(0, |spread, bit| {
        spread | (((value >> bit) & 1) << (2 * bit))
    })
}

#[test]
fn typed_spread_carry_and_logic_table_is_exact_and_collision_free() {
    assert_eq!(COMPACT_SHA_TYPED_SPREAD_ROWS_V2, 93_662);
    assert_eq!(COMPACT_SHA_TABLE_ROWS_V2, 93_716);
    let entries = compact_sha_table_entries_v2();
    assert_eq!(entries.len(), COMPACT_SHA_TABLE_ROWS_V2);
    let table = entries.into_iter().collect::<HashMap<_, _>>();
    assert_eq!(table.len(), COMPACT_SHA_TABLE_ROWS_V2);
    assert_eq!(table.get(&0), Some(&0));
    for width in COMPACT_SHA_SPREAD_WIDTHS_V2 {
        let maximum = (1_u64 << width) - 1;
        let base = (u64::try_from(width).expect("width fits u64")) << 16;
        for value in 0..=maximum {
            assert_eq!(
                table.get(&(base | value)),
                Some(&local_spread(value, width))
            );
        }
    }
    for carry in 0..5_u64 {
        assert_eq!(table.get(&((1_u64 << 22) + carry)), Some(&carry));
    }
    for packed in 0..8_u64 {
        let x = packed & 1 != 0;
        let y = packed & 2 != 0;
        let z = packed & 4 != 0;
        let expected = [
            u64::from(x ^ y ^ z),
            u64::from((x & y) ^ (!x & z)),
            u64::from((x & y) ^ (x & z) ^ (y & z)),
            u64::from(x),
            0,
            1,
        ];
        for (mode, output) in expected.into_iter().enumerate() {
            let key = (1_u64 << 23) + u64::try_from(mode).expect("mode fits u64") * 8 + packed;
            assert_eq!(table.get(&key), Some(&output));
        }
    }
}

#[test]
fn padding_endpoints_and_fixed_job_geometry_are_canonical() {
    for (length, blocks) in [
        (0, 1),
        (1, 1),
        (55, 1),
        (56, 2),
        (63, 2),
        (64, 2),
        (119, 2),
        (120, 3),
    ] {
        assert_eq!(compact_sha_padded_blocks_v2(length), Ok(blocks));
    }
    assert_eq!(
        compact_sha_padded_blocks_v2(usize::MAX),
        Err(CompactShaFailureV2::MessageGeometry)
    );
    if let Ok(unencodable) = usize::try_from(u64::MAX / 8 + 1) {
        assert_eq!(
            compact_sha_padded_blocks_v2(unencodable),
            Err(CompactShaFailureV2::MessageGeometry)
        );
    }
    assert_eq!(
        compact_sha_padded_blocks_v2(COMPACT_SHA_RAW_TBS_MAX_BYTES_V2),
        Ok(COMPACT_SHA_RAW_TBS_MAX_BLOCKS_V2)
    );
    assert_eq!(
        compact_sha_padded_blocks_v2(COMPACT_SHA_RAW_TBS_MAX_BYTES_V2 + 1),
        Ok(COMPACT_SHA_RAW_TBS_MAX_BLOCKS_V2 + 1)
    );
    let messages = fixture_abi().fixed_messages().expect("fixed framing");
    assert_eq!(
        messages.each_ref().map(Vec::len),
        COMPACT_SHA_FIXED_MESSAGE_BYTES_V2
    );
    assert_eq!(messages.iter().map(Vec::len).sum::<usize>(), 3_419);
    assert_eq!(COMPACT_SHA_FIXED_MESSAGE_TOTAL_BYTES_V2, 3_419);
    assert_eq!(
        messages
            .each_ref()
            .map(|message| compact_sha_padded_blocks_v2(message.len()).unwrap()),
        COMPACT_SHA_FIXED_MESSAGE_BLOCKS_V2
    );
}

#[test]
fn fixed_abi_has_exact_words_cells_and_canonical_final_padding() {
    let abi = fixture_abi();
    assert_eq!(abi.mode(), CompactShaPublicModeV2::FixedNineJob);
    assert_eq!(COMPACT_SHA_BATCH_WORDS_V2, 218);
    assert_eq!(COMPACT_SHA_BATCH_INSTANCE_CELLS_V2, 32);
    assert_eq!(COMPACT_SHA_BATCH_FINAL_ZERO_WORDS_V2, 6);
    let words = abi.words();
    assert_eq!(words.len(), 218);
    assert_eq!(&words[..COMPACT_SHA_HELPER_WORDS_V2], abi.helper_words());
    assert_eq!(words[COMPACT_SHA_HELPER_WORDS_V2].to_le_bytes()[0], 0x04);
    assert_eq!(
        &words[COMPACT_SHA_HELPER_WORDS_V2 + 16].to_le_bytes()[1..],
        &[0, 0, 0]
    );
    assert_eq!(
        words[COMPACT_SHA_HELPER_WORDS_V2 + 17].to_le_bytes()[0],
        0x04
    );
    assert_eq!(&words[217].to_le_bytes()[1..], &[0, 0, 0]);
    assert!(Fp::NUM_BITS > 224 && Fq::NUM_BITS > 224);
    let fp_instances = abi.field_instances::<Fp>();
    let fq_instances = abi.field_instances::<Fq>();
    assert_eq!(fp_instances.len(), 32);
    assert_eq!(fq_instances.len(), 32);
    assert_eq!(fp_instances[31], Fp::from(u64::from(words[217])));
    assert_eq!(fq_instances[31], Fq::from(u64::from(words[217])));
    assert_eq!(
        CompactShaCircuitV2::<Fp>::new(abi).public_instances().len(),
        32
    );
}

#[test]
fn duplicated_v1_helper_protocol_identities_match_live_v1_source() {
    for (parity_word, parity) in [
        (1, OfflineCashHalo2ParityV1::Eq),
        (2, OfflineCashHalo2ParityV1::Ep),
    ] {
        for (role_word, role) in [
            (2, OfflineCashHalo2CircuitRoleV1::GuardUse),
            (3, OfflineCashHalo2CircuitRoleV1::PlatformBind),
            (4, OfflineCashHalo2CircuitRoleV1::AndroidKeyCert),
            (5, OfflineCashHalo2CircuitRoleV1::GuardBundle),
        ] {
            assert_eq!(
                compact_sha_source_helper_protocol_digest_v1(parity_word, role_word),
                Some(offline_cash_halo2_protocol_identity_v1(parity, role).digest())
            );
        }
    }
    assert_eq!(compact_sha_source_helper_protocol_digest_v1(1, 1), None);
    assert_eq!(compact_sha_source_helper_protocol_digest_v1(3, 2), None);
}

#[test]
fn abi_endpoint_rejections_are_specific() {
    let (words, platform, issuer) = fixture_parts();

    let mut invalid = words;
    invalid[2] = COMPACT_SHA_K_V2;
    assert_eq!(
        CompactShaBatchPublicAbiV2::new(invalid, platform, issuer),
        Err(CompactShaAbiErrorV2::InvalidHeader)
    );

    let mut invalid = words;
    invalid[16] ^= 1;
    assert_eq!(
        CompactShaBatchPublicAbiV2::new(invalid, platform, issuer),
        Err(CompactShaAbiErrorV2::InvalidProtocolIdentity)
    );

    let mut invalid = words;
    invalid[5] = 3;
    assert_eq!(
        CompactShaBatchPublicAbiV2::new(invalid, platform, issuer),
        Err(CompactShaAbiErrorV2::InvalidOperation)
    );

    let mut invalid = words;
    invalid[6] = 0;
    assert_eq!(
        CompactShaBatchPublicAbiV2::new(invalid, platform, issuer),
        Err(CompactShaAbiErrorV2::AndroidCertificateRequired)
    );

    let mut invalid = words;
    invalid[10] = 43;
    assert_eq!(
        CompactShaBatchPublicAbiV2::new(invalid, platform, issuer),
        Err(CompactShaAbiErrorV2::InvalidSequence)
    );

    let mut invalid_platform = platform;
    invalid_platform[0] = 2;
    assert_eq!(
        CompactShaBatchPublicAbiV2::new(words, invalid_platform, issuer),
        Err(CompactShaAbiErrorV2::InvalidSec1Key)
    );

    let mut invalid_issuer = issuer;
    invalid_issuer[0] = 2;
    assert_eq!(
        CompactShaBatchPublicAbiV2::new(words, platform, invalid_issuer),
        Err(CompactShaAbiErrorV2::InvalidSec1Key)
    );

    let mut invalid = words;
    invalid[24..32].fill(0);
    assert_eq!(
        CompactShaBatchPublicAbiV2::new(invalid, platform, issuer),
        Err(CompactShaAbiErrorV2::InvalidDigest)
    );

    let mut invalid = words;
    let current_guard: [u32; 8] = invalid[88..96].try_into().expect("eight guard words");
    invalid[96..104].copy_from_slice(&current_guard);
    assert_eq!(
        CompactShaBatchPublicAbiV2::new(invalid, platform, issuer),
        Err(CompactShaAbiErrorV2::InvalidGuardTransition)
    );

    let mut invalid = words;
    let current_head: [u32; 8] = invalid[40..48].try_into().expect("eight head words");
    invalid[56..64].copy_from_slice(&current_head);
    assert_eq!(
        CompactShaBatchPublicAbiV2::new(invalid, platform, issuer),
        Err(CompactShaAbiErrorV2::InvalidGuardTransition)
    );
}

#[test]
fn sequential_nine_job_known_answer_fixture_matches_sha256() {
    let abi = sequential_kat_abi();
    let messages = abi.fixed_messages().expect("sequential KAT framing");
    let expected = abi.expected_digests();
    let audit = compact_sha_counting_audit_v2::<Fp>(&abi)
        .expect("circuit-side fixed routing fits the ABI-only audit prefix");
    assert_eq!(audit.circuit_messages, messages);
    assert_eq!(audit.circuit_expected_digests, expected);
    for index in 0..9 {
        let digest: [u8; 32] = Sha256::digest(&audit.circuit_messages[index]).into();
        assert_eq!(digest, expected[index], "fixed SHA job {index}");
    }
}

#[test]
fn raw_tbs_contract_is_separate_bounded_and_ineligible() {
    assert_eq!(COMPACT_SHA_RAW_TBS_MAX_BYTES_V2, 3_767);
    assert_eq!(COMPACT_SHA_RAW_TBS_MAX_BLOCKS_V2, 59);
    assert_eq!(COMPACT_SHA_RAW_TBS_WORDS_V2, 942);
    assert_eq!(COMPACT_SHA_RAW_TBS_INSTANCE_CELLS_V2, 164);
    assert_eq!(COMPACT_SHA_RAW_TBS_FINAL_ZERO_WORDS_V2, 4);
    assert_eq!(
        CompactShaRawTbsContractV2::new(Vec::new()),
        Err(CompactShaAbiErrorV2::EmptyRawTbs)
    );
    assert_eq!(
        CompactShaRawTbsContractV2::from_canonical_payload_words(
            0,
            [0; COMPACT_SHA_RAW_TBS_WORDS_V2]
        ),
        Err(CompactShaAbiErrorV2::EmptyRawTbs)
    );
    assert_eq!(
        CompactShaRawTbsContractV2::from_canonical_payload_words(
            3_768,
            [0; COMPACT_SHA_RAW_TBS_WORDS_V2]
        ),
        Err(CompactShaAbiErrorV2::RawTbsCapExceeded {
            actual: 3_768,
            maximum: 3_767,
        })
    );
    let maximum = CompactShaRawTbsContractV2::new(vec![0x5a; 3_767]).expect("cap is inclusive");
    assert_eq!(maximum.mode(), CompactShaPublicModeV2::RawTbs);
    assert_eq!(maximum.exact_bytes().len(), 3_767);
    assert_eq!(maximum.exact_length_word(), 3_767);
    let canonical = maximum.canonical_payload_words();
    assert_eq!(canonical[0], u32::from_le_bytes([0x5a; 4]));
    assert_eq!(canonical[941].to_le_bytes(), [0x5a, 0x5a, 0x5a, 0]);
    assert_eq!(
        CompactShaRawTbsContractV2::from_canonical_payload_words(3_767, canonical),
        Ok(maximum.clone())
    );
    let mut noncanonical = canonical;
    noncanonical[941] |= 0xff00_0000;
    assert_eq!(
        CompactShaRawTbsContractV2::from_canonical_payload_words(3_767, noncanonical),
        Err(CompactShaAbiErrorV2::NonCanonicalRawTbsPayload)
    );
    assert!(!maximum.activation_eligible());
    assert_eq!(
        CompactShaRawTbsContractV2::new(vec![0x5a; 3_768]),
        Err(CompactShaAbiErrorV2::RawTbsCapExceeded {
            actual: 3_768,
            maximum: 3_767,
        })
    );
}

#[test]
fn every_readiness_and_release_gate_remains_false() {
    assert!(COMPACT_SHA_BATCH_MACHINE_SOURCE_IMPLEMENTED_V2);
    assert!(COMPACT_SHA_RAW_TBS_PAYLOAD_CODEC_IMPLEMENTED_V2);
    assert!(!COMPACT_SHA_RAW_TBS_AGGREGATE_ABI_IMPLEMENTED_V2);
    assert!(!COMPACT_SHA_COMPILE_EVIDENCE_AVAILABLE_V2);
    assert!(!COMPACT_SHA_BATCH_ROW_QUALIFIED_V2);
    assert!(!COMPACT_SHA_RAW_TBS_CIRCUIT_IMPLEMENTED_V2);
    assert!(!COMPACT_SHA_PRODUCTION_AVAILABLE_V2);
    assert!(!COMPACT_SHA_ARTIFACT_EVIDENCE_AVAILABLE_V2);
    assert!(!COMPACT_SHA_RECURSIVE_ADAPTER_AVAILABLE_V2);
    assert!(!COMPACT_SHA_RELEASE_ELIGIBLE_V2);
    assert_eq!(
        COMPACT_SHA_PUBLIC_ABI_REVISION_V2,
        b"offline-cash-v2-conditional-compact-sha/u32le-v1-helper-words184-continuously-repacked+sec1-17+sec1-17/pack7/direct-one-column/final-zero6/not-v1-cell-prefix/v2"
    );
    assert_eq!(
        COMPACT_SHA_TRANSCRIPT_TARGET_V2,
        b"Blake2bRead+Blake2bWrite/Challenge255/direct-instance/future-parent-mode-before-instances/exact-proof-length/v1"
    );
    assert_eq!(
        COMPACT_SHA_CANONICALITY_TARGET_V2,
        b"pasta-field-capacity-at-least-225/canonical-field-encoding/no-reduction-alias/sec1-prefix04+terminal-zero3/exact-3232-byte-proof+32-byte-augmentation/no-trailing-bytes/future-verifier-derived-instances/v2"
    );
}

#[test]
#[ignore = "full k=17 table assignment is intentionally opt-in"]
fn diagnostic_abc_sha256_mock_prover_accepts() {
    let expected: [u8; 32] = Sha256::digest(b"abc").into();
    let circuit = CompactShaDiagnosticCircuitV2::<Fp>::new(b"abc".to_vec(), expected);
    let instances = circuit.public_instances();
    let prover = MockProver::run(COMPACT_SHA_K_V2, &circuit, vec![instances])
        .expect("diagnostic compact SHA circuit synthesizes");
    prover.assert_satisfied();
}

#[test]
#[ignore = "full k=17 table assignment is intentionally opt-in"]
fn diagnostic_wrong_digest_mock_prover_rejects() {
    let mut expected: [u8; 32] = Sha256::digest(b"abc").into();
    expected[0] ^= 1;
    let circuit = CompactShaDiagnosticCircuitV2::<Fp>::new(b"abc".to_vec(), expected);
    let instances = circuit.public_instances();
    let prover = MockProver::run(COMPACT_SHA_K_V2, &circuit, vec![instances])
        .expect("diagnostic compact SHA circuit synthesizes");
    assert!(prover.verify().is_err());
}
