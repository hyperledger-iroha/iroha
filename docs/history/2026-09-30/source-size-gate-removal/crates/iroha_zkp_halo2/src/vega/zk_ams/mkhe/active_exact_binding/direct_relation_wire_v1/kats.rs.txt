use super::*;
use crate::vega::sponge::{Keccak256, keccak256};
#[test]
fn canonical_schema_digest_is_pinned() {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.direct-relation-wire.schema-kat");
    hash.update(&DIRECT_RELATION_WIRE_MAGIC_V1);
    hash.update(&DIRECT_RELATION_STATEMENT_MAGIC_V1);
    hash.update(&[DIRECT_RELATION_CODEC_VERSION_V1]);
    for width in [
        HEADER_BYTES_V1,
        STATEMENT_PREFIX_BYTES_V1,
        OBJECT_ENTRY_BYTES_V1,
        RKG_ONE_STATEMENT_BYTES_V1,
        RKG_TWO_STATEMENT_BYTES_V1,
        NORMALIZE_STATEMENT_BYTES_V1,
        GALOIS_STATEMENT_BYTES_V1,
        MEMBERSHIP_BYTES_V1,
        RESPONSE_BYTES_V1,
        BLIND_RESPONSE_BYTES_V1,
        CHALLENGE_SEED_BYTES_V1,
        BODY_BYTES_V1,
        RELEASE_RNS_LIMBS_V1,
        RELEASE_RING_COEFFICIENTS_V1,
        RECONSTRUCTED_COMMITMENT_BYTES_V1,
        super::predecode_v1::MEMBERSHIP_HEADER_BYTES_V1,
        super::predecode_v1::BOUND_ONE_CHUNK_WIRE_BYTES_V1,
        super::predecode_v1::BOUND_TWO_CHUNK_WIRE_BYTES_V1,
        super::predecode_v1::INNER_COMMITMENT_OFFSET_V1,
    ] {
        hash.update(&(width as u64).to_be_bytes());
    }
    for offset in MEMBERSHIP_FRAME_OFFSETS_V1 {
        hash.update(&(offset as u64).to_be_bytes());
    }
    for relation in [
        PersistentDirectRelationV1::RkgRoundOne,
        PersistentDirectRelationV1::RkgRoundTwo,
        PersistentDirectRelationV1::RkgNormalize,
        PersistentDirectRelationV1::Galois,
    ] {
        hash.update(&[
            relation as u8,
            relation.object_count() as u8,
            relation.active_witness_mask(),
            relation.forced_zero_witness_mask(),
        ]);
        let (rows, count) = relation.rns_row_tags();
        hash.update(&[count as u8]);
        hash.update(&rows[..count]);
    }
    for domain in [
        RELATION_CORE_DOMAIN_V1,
        FINAL_STATEMENT_DOMAIN_V1,
        MEMBERSHIP_SLOT_DOMAIN_V1,
        ORDERED_COMMITMENT_ROOT_DOMAIN_V1,
        ORDERED_MEMBERSHIP_ROOT_DOMAIN_V1,
        RELATION_LINEAGE_DOMAIN_V1,
        RNS_FIRST_MESSAGE_DOMAIN_V1,
        COMMITMENT_FIRST_MESSAGE_DOMAIN_V1,
    ] {
        hash.update(&(domain.len() as u16).to_be_bytes());
        hash.update(domain);
    }
    assert_eq!(
        hex::encode(hash.finalize()),
        "d247f76ddf02ded9055cb6fad73ff784f8861759b81f25e6b699c2723c344d14"
    );
}
#[test]
fn canonical_wire_accounting_kat_is_exact() {
    assert_eq!(BODY_BYTES_V1, 25_247_858);
    assert_eq!(
        HEADER_BYTES_V1 + RKG_ONE_STATEMENT_BYTES_V1 + BODY_BYTES_V1,
        25_248_766
    );
    assert_eq!(
        HEADER_BYTES_V1 + RKG_TWO_STATEMENT_BYTES_V1 + BODY_BYTES_V1,
        25_248_876
    );
    assert_eq!(
        HEADER_BYTES_V1 + NORMALIZE_STATEMENT_BYTES_V1 + BODY_BYTES_V1,
        25_248_766
    );
    assert_eq!(
        HEADER_BYTES_V1 + GALOIS_STATEMENT_BYTES_V1 + BODY_BYTES_V1,
        25_248_656
    );
}
#[test]
fn all_four_header_encodings_have_fixed_kats() {
    let fixtures = [
        (
            PersistentDirectRelationV1::RkgRoundOne,
            "891f2e36fdf9e73eb1e5718391acabd4c15d02454bd4e1ce49509cffd0c18ad9",
        ),
        (
            PersistentDirectRelationV1::RkgRoundTwo,
            "57a5451b600aea14e7d82ac8f33ca9714eb61c3c1724ad174570e423e708cb85",
        ),
        (
            PersistentDirectRelationV1::RkgNormalize,
            "cc9bafec670af20476adccef7fb006c5d90cbe49f32c0a857ae3fb308ade233e",
        ),
        (
            PersistentDirectRelationV1::Galois,
            "09b97e486efa913e8004e123b9faed3e8bf6f4c3b5fbfc70afff93d045e9ce0a",
        ),
    ];
    for (relation, kat) in fixtures {
        let statement =
            ExpectedDirectRelationStatementV1::layout_fixture(relation, [relation as u8; 32]);
        assert_eq!(hex::encode(keccak256(&canonical_header(&statement))), kat);
    }
}
#[test]
fn all_four_statement_core_and_final_hash_frames_are_pinned() {
    let fixtures = [
        (
            1_u8,
            828,
            2_u8,
            1_u8,
            "e0e19d5c63280387a038d0cddb9dff5d7b33fddab79c3fd2405be60ff8568f3f",
            "c7fbf67203c0abab9d7805e67f2db548abf4daf460793efc249bd913d82b8195",
        ),
        (
            2,
            938,
            3,
            1,
            "d844848b1a06f8e9682048162031d5199070c3964722a9e72d8a3a8e6e1f0276",
            "adbee0f7daac331e6294c9d3d2e73bb3707e0bc941dd6f80baf23b852cb46a6a",
        ),
        (
            3,
            828,
            2,
            0,
            "4a7c6dc22d13e1c2188251e38a4619b02351bb38370816e978170fc001dc35df",
            "f70cdb3931352dbcd598b5bee733dfb866c50f7643d8882bb2050b12109c41ab",
        ),
        (
            4,
            718,
            1,
            0,
            "1f98a40d1bcdb613d1fb5da84e7ef12c7dc3df29d9227507e11e82159d0a9774",
            "74afc22d333137a52bc5a2a9e49df842ab88fe68ce2a6c39bfebb1eb43255b86",
        ),
    ];
    for (relation, length, objects, ephemeral, core_kat, final_kat) in fixtures {
        let mut bytes = (0..length)
            .map(|index| (index as u8).wrapping_mul(17).wrapping_add(relation))
            .collect::<Vec<_>>();
        bytes[..4].copy_from_slice(&DIRECT_RELATION_STATEMENT_MAGIC_V1);
        bytes[4] = DIRECT_RELATION_CODEC_VERSION_V1;
        bytes[5] = relation;
        bytes[6] = objects;
        bytes[7] = ephemeral;
        bytes[8..12].copy_from_slice(&(length as u32).to_be_bytes());
        assert_eq!(
            hex::encode(super::statement_v1::domain_hash_for_test(
                RELATION_CORE_DOMAIN_V1,
                &bytes[..length - STATEMENT_TRAILER_BYTES_V1],
            )),
            core_kat
        );
        assert_eq!(
            hex::encode(super::statement_v1::domain_hash_for_test(
                FINAL_STATEMENT_DOMAIN_V1,
                &bytes,
            )),
            final_kat
        );
    }
}
#[test]
fn first_message_challenge_seed_and_coordinates_are_pinned() {
    let context = ExactBindingTranscriptContextV1 {
        profile_digest: [1; 32],
        roster_digest: [2; 32],
        key_material_digest: [3; 32],
        epoch: 4,
        protocol_transcript_digest: [5; 32],
        round_tag: 1,
        party_index: 0,
        party: [6; 32],
        record_index: 7,
        relation_index: 8,
        statement_digest: [9; 32],
        commitment_set_digest: [10; 32],
        membership_proof_set_digest: [11; 32],
        persistent_graph_digest: [12; 32],
    };
    let first_messages = DirectRelationFirstMessageDigestsV1::new(
        core::array::from_fn(|index| [13 + index as u8; 32]),
        core::array::from_fn(|index| [17 + index as u8; 32]),
    )
    .expect("nonzero first messages");
    let (seed, coordinates) =
        challenge_vector_from_first_messages(context, first_messages).expect("challenge KAT");
    assert_eq!(
        hex::encode(seed),
        "0904346cfa9f051e59f3bff80220c037685d9e385a9b77dd622ddb391bca0284"
    );
    assert_eq!(
        coordinates,
        [2_768_221_376, 3_188_246_095, 2_792_925_365, 3_420_889_530]
    );
}

#[test]
fn rkg_one_core_trailer_and_absolute_proof_offsets_are_exact() {
    assert_eq!(STATEMENT_PREFIX_BYTES_V1 + 2 * OBJECT_ENTRY_BYTES_V1, 764);
    assert_eq!(RKG_ONE_STATEMENT_BYTES_V1, 764 + 32 + 32);
    assert_eq!(
        MEMBERSHIP_FRAME_OFFSETS_V1,
        [0, 12_291, 24_582, 37_401, 50_220, 63_039]
    );
    assert_eq!(
        [
            0,
            HEADER_BYTES_V1,
            HEADER_BYTES_V1 + RKG_ONE_STATEMENT_BYTES_V1,
            HEADER_BYTES_V1 + RKG_ONE_STATEMENT_BYTES_V1 + MEMBERSHIP_BYTES_V1,
            HEADER_BYTES_V1 + RKG_ONE_STATEMENT_BYTES_V1 + MEMBERSHIP_BYTES_V1 + RESPONSE_BYTES_V1,
            HEADER_BYTES_V1
                + RKG_ONE_STATEMENT_BYTES_V1
                + MEMBERSHIP_BYTES_V1
                + RESPONSE_BYTES_V1
                + BLIND_RESPONSE_BYTES_V1,
            HEADER_BYTES_V1 + RKG_ONE_STATEMENT_BYTES_V1 + BODY_BYTES_V1,
        ],
        [0, 80, 908, 76_766, 25_242_590, 25_248_734, 25_248_766]
    );
    assert_eq!(
        [
            HEADER_BYTES_V1,
            HEADER_BYTES_V1 + 764,
            HEADER_BYTES_V1 + 796,
            HEADER_BYTES_V1 + 828,
        ],
        [80, 844, 876, 908]
    );

    let digest = [0xa5; 32];
    let header = canonical_header_fields_v1(
        PersistentDirectRelationV1::RkgRoundOne,
        RKG_ONE_STATEMENT_BYTES_V1,
        digest,
    );
    assert_eq!(&header[..4], b"ZAXR");
    assert_eq!(&header[44..48], &[0; 4]);
    assert_eq!(&header[48..80], &digest);
    let word = |offset| u32::from_be_bytes(header[offset..offset + 4].try_into().unwrap());
    assert_eq!(
        [
            word(12),
            word(16),
            word(20),
            word(24),
            word(28),
            word(32),
            word(36),
            word(40)
        ],
        [
            80, 828, 75_858, 25_165_824, 6_144, 32, 25_247_858, 25_248_766
        ]
    );
    let core = include_str!("statement_v1/rkg_one_creator_core_v1.rs");
    assert!(core.contains("const PREPARED_RKG_ONE_STATEMENT_CORE_BYTES_V1: usize = 764"));
    assert_eq!(core.matches("fn build_statement_core_v1(").count(), 1);
}

#[test]
fn rkg_one_membership_roles_rows_and_sequential_slot_order_are_pinned() {
    use crate::vega::bulletproof_t256::ZkAmsT256MembershipBoundV1;
    use crate::vega::zk_ams::mkhe::exact_eight_chunk_membership::{
        DirectRelationBoundOneMembershipRoleV1, DirectRelationBoundTwoMembershipRoleV1,
        ExactEightChunkMembershipRoleV1,
    };

    assert_eq!(DirectRelationBoundOneMembershipRoleV1::MAGIC, *b"ZDB1");
    assert_eq!(
        DirectRelationBoundOneMembershipRoleV1::BOUND,
        ZkAmsT256MembershipBoundV1::One
    );
    assert_eq!(DirectRelationBoundOneMembershipRoleV1::WIRE_BYTES, 12_291);
    assert_eq!(DirectRelationBoundTwoMembershipRoleV1::MAGIC, *b"ZDB2");
    assert_eq!(
        DirectRelationBoundTwoMembershipRoleV1::BOUND,
        ZkAmsT256MembershipBoundV1::Two
    );
    assert_eq!(DirectRelationBoundTwoMembershipRoleV1::WIRE_BYTES, 12_819);
    assert_eq!(
        PersistentDirectRelationV1::RkgRoundOne.rns_row_tags(),
        ([1, 2, 0x84, 0x85, 0], 4)
    );

    let source = include_str!("rkg_one_creator_membership_v1.rs");
    let loop_start = source.find("for slot in 0..6").unwrap();
    let bound_one = source[loop_start..].find("prove_bound_one_v1").unwrap();
    let bound_two = source[loop_start..].find("prove_bound_two_v1").unwrap();
    assert!(source[loop_start..].contains("if slot < 2"));
    assert!(bound_one < bound_two);
    for legacy_magic in ["ZPME", "ZRME", "ZCEM"] {
        assert!(!source.contains(legacy_magic));
    }
    assert!(source.contains("commitment_root.update(&[slot as u8])"));
    assert!(source.contains("membership_root.update(&[slot as u8])"));
    let scalar_codec = include_str!("../../../../bulletproof_t256.rs");
    let scalar_encode = scalar_codec
        .split("fn encode(self) -> [u8; 32]")
        .nth(1)
        .unwrap();
    assert!(scalar_encode[..160].contains("self.to_le_bytes()"));
}

#[test]
fn rkg_one_response_attempt_is_fresh_whole_box_and_memberships_precede_it() {
    assert_eq!(super::super::OUTER_RETRY_CEILING_V1, 128);
    assert_eq!(CHALLENGE_REPETITIONS_V1 * WITNESS_COUNT_V1, 24);
    assert_eq!(RESPONSE_BYTES_V1 / 8, 24 * RELEASE_RING_COEFFICIENTS_V1);
    assert_eq!(BLIND_RESPONSE_BYTES_V1 / 32, 24 * CHUNKS_PER_WITNESS_V1);
    let response = include_str!("rkg_one_creator_response_v1.rs");
    let loop_start = response.find("for _ in 0..OUTER_RETRY_CEILING_V1").unwrap();
    let attempt = &response[loop_start..];
    let masks = attempt.find("fill_exact_masks_v1").unwrap();
    let blindings = attempt.find("ZeroizingMaskBlindingsV1::sample").unwrap();
    let first_messages = attempt.find("rkg_one_rns_first_messages_v1").unwrap();
    let transform = attempt.find("transform_responses_in_place_v1").unwrap();
    assert!(masks < blindings && blindings < first_messages && first_messages < transform);
    assert!(attempt.contains("if !accepted"));
    assert!(attempt.contains("responses.fill(0)"));
    assert!(
        attempt.find("if !accepted").unwrap() < attempt.find("encode_blind_responses_v1").unwrap()
    );
    assert!(response.contains("assert!(MASK_BLINDING_COUNT_V1 == 192)"));
    assert!(
        response.contains("output[offset..offset + 32].copy_from_slice(&response.to_be_bytes())")
    );
    assert!(response.contains("word.copy_from_slice(&response.to_be_bytes())"));
    let transform = response
        .split("fn transform_responses_in_place_v1(")
        .nth(1)
        .unwrap()
        .split("fn encode_blind_responses_v1(")
        .next()
        .unwrap();
    assert!(!transform.contains("continue"));
    assert!(!transform.contains("random"));

    let prover = include_str!("rkg_one_creator_prover_v1.rs");
    assert!(
        prover
            .find("let memberships = generate_direct_rkg_one_memberships_v1(")
            .unwrap()
            < prover
                .find("try_reserve_exact(DIRECT_RKG_ONE_PROOF_BYTES_V1 - RESPONSE_START_V1)")
                .unwrap()
    );
    assert!(
        prover
            .find("let memberships = generate_direct_rkg_one_memberships_v1(")
            .unwrap()
            < prover.find("create_direct_rkg_one_responses_v1(").unwrap()
    );
    assert!(prover.contains("builder.bytes[SEED_START_V1..].copy_from_slice(&seed)"));
    assert!(prover.contains("let request = permit.finalize_request_v1(seed)?"));
    assert!(prover.contains("core::hint::black_box(self.bytes.as_mut_slice())"));
    let first_messages = include_str!("rkg_one_creator_response_v1/rns_first_messages_v1.rs");
    assert!(!first_messages.contains("core::ops::Deref"));
    assert!(!first_messages.contains("core::ops::DerefMut"));
    assert!(first_messages.contains("core::hint::black_box(self.0.as_mut_slice())"));
    let adapter = include_str!("../direct_rkg_one_creator_adapter_v1.rs");
    assert!(adapter.contains(
        "destination[32..]\n            .copy_from_slice(&self.capability.selector.proof_commitment_transcript_digest)"
    ));
}

#[test]
fn rkg_one_creator_production_areas_stay_within_review_caps() {
    let within_cap = |sources: &[&str]| {
        sources
            .iter()
            .map(|source| source.lines().count())
            .sum::<usize>()
            <= 500
            && sources.iter().map(|source| source.len()).sum::<usize>() <= 24 * 1024
    };
    assert!(within_cap(&[include_str!(
        "../direct_rkg_one_creator_adapter_v1.rs"
    )]));
    assert!(within_cap(&[include_str!(
        "statement_v1/rkg_one_creator_core_v1.rs"
    )]));
    assert!(within_cap(&[include_str!(
        "rkg_one_creator_membership_v1.rs"
    )]));
    assert!(within_cap(&[
        include_str!("rkg_one_creator_prover_v1.rs"),
        include_str!("rkg_one_creator_prover_v1/transcript_v1.rs"),
    ]));
    assert!(within_cap(&[
        include_str!("rkg_one_creator_response_v1.rs"),
        include_str!("rkg_one_creator_response_v1/rns_first_messages_v1.rs"),
    ]));
    assert!(include_str!("kats.rs").lines().count() <= 500);
    assert!(include_str!("kats.rs").len() <= 24 * 1024);
}
