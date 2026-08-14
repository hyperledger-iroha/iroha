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
