use super::super::DirectRkgOnePublicationScopeV1;
use super::record_v2::*;
use crate::vega::zk_ams::mkhe::{
    ZkAmsMkhePartyIdV1,
    direct_object_transport::{ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1},
};
use std::collections::BTreeSet;

const LEGACY_KEY_KAT_V1: &str = "3b0e7afc152418fd7158c31aaf3a8e2c13a18be6da045cb2a7abe5b44485e992";
const FRESH_KAT_V2: &str = "52314c320102000000000000000000003b0e7afc152418fd7158c31aaf3a8e2c13a18be6da045cb2a7abe5b44485e9921111111111111111111111111111111111111111111111111111111111111111030722222222222222222222222222222222222222222222222222222222222222220000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000bfb6a3fb2312ba90259ce2fe87e1efa8ff9e6487af2dd9a68d4d6003bc8cad01";
const PUBLISHED_KAT_V2: &str = "52314c320102010000000000000000013b0e7afc152418fd7158c31aaf3a8e2c13a18be6da045cb2a7abe5b44485e99211111111111111111111111111111111111111111111111111111111111111110307222222222222222222222222222222222222222222222222222222222222222233333333333333333333333333333333333333333333333333333333333333335a444f500101000000000260000044444444444444444444444444444444444444444444444444444444444444445c1d074f316f6c1d384045487ec4f3512e16b3b24afaeccc72c9374cbcad1ee45a444f5001020000000002600000555555555555555555555555555555555555555555555555555555555555555575c5aadea9f435181af4ee8f24721f082f182d6d2d17e4c3f1427541fb2a204166666666666666666666666666666666666666666666666666666666666666667777777777777777777777777777777777777777777777777777777777777777888888888888888888888888888888888888888888888888888888888888888800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000091f99a3b83ff0a1ad1dd545dcdb988a169e1defd5a8e9f654e241d369b679570";
const PROOF_KAT_V2: &str = "52314c320102020000000000000000023b0e7afc152418fd7158c31aaf3a8e2c13a18be6da045cb2a7abe5b44485e99211111111111111111111111111111111111111111111111111111111111111110307222222222222222222222222222222222222222222222222222222222222222233333333333333333333333333333333333333333333333333333333333333335a444f500101000000000260000044444444444444444444444444444444444444444444444444444444444444445c1d074f316f6c1d384045487ec4f3512e16b3b24afaeccc72c9374cbcad1ee45a444f5001020000000002600000555555555555555555555555555555555555555555555555555555555555555575c5aadea9f435181af4ee8f24721f082f182d6d2d17e4c3f1427541fb2a204166666666666666666666666666666666666666666666666666666666666666667777777777777777777777777777777777777777777777777777777777777777888888888888888888888888888888888888888888888888888888888888888833333333333333333333333333333333333333333333333333333333333333335a444f50010800000000018143fe9999999999999999999999999999999999999999999999999999999999999999e60064d007f4ecb582623716729ddd7a2424d48cecd64fd7ff504bad441a130caaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c12c8bbdc303e823f777dc5cee6b61319fa8a4e83d1f12373176a74087bdab9f";
const RESERVED_VERIFIED_KAT_V2: &str = "52314c320102030000000000000000033b0e7afc152418fd7158c31aaf3a8e2c13a18be6da045cb2a7abe5b44485e99211111111111111111111111111111111111111111111111111111111111111110307222222222222222222222222222222222222222222222222222222222222222233333333333333333333333333333333333333333333333333333333333333335a444f500101000000000260000044444444444444444444444444444444444444444444444444444444444444445c1d074f316f6c1d384045487ec4f3512e16b3b24afaeccc72c9374cbcad1ee45a444f5001020000000002600000555555555555555555555555555555555555555555555555555555555555555575c5aadea9f435181af4ee8f24721f082f182d6d2d17e4c3f1427541fb2a204166666666666666666666666666666666666666666666666666666666666666667777777777777777777777777777777777777777777777777777777777777777888888888888888888888888888888888888888888888888888888888888888833333333333333333333333333333333333333333333333333333333333333335a444f50010800000000018143fe9999999999999999999999999999999999999999999999999999999999999999e60064d007f4ecb582623716729ddd7a2424d48cecd64fd7ff504bad441a130caaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc0000000043f04c3e561855a64250e304f369ad3aca3d281c332d30a5f598f598b0896af1";

fn fixture_scope() -> DirectRkgOnePublicationScopeV1 {
    DirectRkgOnePublicationScopeV1 {
        context_digest: [0x11; 32],
        party_index: 3,
        digit_index: 7,
        party: ZkAmsMkhePartyIdV1::new([0x22; 32]).expect("nonzero party"),
    }
}

fn pointer(kind: ZkAmsMkheDirectObjectKindV1, byte: u8) -> ZkAmsMkheDirectObjectPointerV1 {
    let payload_bytes = match kind {
        ZkAmsMkheDirectObjectKindV1::ProofEnvelope => 25_248_766,
        _ => 39_845_888,
    };
    ZkAmsMkheDirectObjectPointerV1::new(kind, payload_bytes, [byte; 32])
        .expect("valid pointer fixture")
}

fn fixture_axes() -> (PublishedAxesV2, ProofAxesV2) {
    let published = PublishedAxesV2 {
        publication_identity: [0x33; 32],
        h0: pointer(ZkAmsMkheDirectObjectKindV1::RkgH0, 0x44),
        h1: pointer(ZkAmsMkheDirectObjectKindV1::RkgH1, 0x55),
        receipt_set_digest: [0x66; 32],
        provider_identity: [0x77; 32],
        snapshot_identity: [0x88; 32],
    };
    let proof = ProofAxesV2 {
        publication_identity: published.publication_identity,
        pointer: pointer(ZkAmsMkheDirectObjectKindV1::ProofEnvelope, 0x99),
        receipt_digest: [0xaa; 32],
    };
    (published, proof)
}

fn fixture_records() -> (RecordV2, RecordV2, RecordV2, RecordV2) {
    let scope = fixture_scope();
    let key = stable_storage_key_v2(scope).expect("stable key");
    let (published_axes, proof_axes) = fixture_axes();
    let mut fresh = [0; RECORD_BYTES_V2];
    let mut published = [0; RECORD_BYTES_V2];
    let mut proof = [0; RECORD_BYTES_V2];
    let mut reserved_verified = [0; RECORD_BYTES_V2];
    encode_fresh_v2(scope, key, &mut fresh).expect("fresh record");
    encode_published_v2(scope, key, published_axes, &mut published).expect("published record");
    encode_proof_v2(scope, key, published_axes, proof_axes, &mut proof).expect("proof record");
    encode_reserved_verified_for_test_v2(
        scope,
        key,
        published_axes,
        proof_axes,
        [0xbb; 32],
        [0xcc; 32],
        &mut reserved_verified,
    );
    (fresh, published, proof, reserved_verified)
}

#[test]
fn stable_key_is_exactly_the_legacy_v1_key() {
    assert_eq!(
        hex::encode(stable_storage_key_v2(fixture_scope()).expect("stable key")),
        LEGACY_KEY_KAT_V1
    );
    assert_eq!(
        hex::encode(publication_receipt_set_digest_v2([0xaa; 32], [0xbb; 32])),
        "9efdd631ebad19c28b0a4ab51dbb964981207b0673e17a8b9a87af47f38e15cb"
    );
}

#[test]
fn stable_key_is_unique_across_all_release_party_digit_slots() {
    let mut keys = BTreeSet::new();
    for digit_index in 0_u8..38 {
        for party_index in 0_u8..8 {
            let mut context_digest = [0; 32];
            context_digest[..2].copy_from_slice(&[digit_index + 1, party_index + 1]);
            let scope = DirectRkgOnePublicationScopeV1 {
                context_digest,
                party_index,
                digit_index,
                party: ZkAmsMkhePartyIdV1::new([party_index + 1; 32]).expect("party"),
            };
            assert!(keys.insert(stable_storage_key_v2(scope).expect("stable key")));
        }
    }
    assert_eq!(keys.len(), 8 * 38);
}

#[test]
fn all_four_full_record_kats_are_literal_and_exact() {
    let (fresh, published, proof, reserved_verified) = fixture_records();
    assert_eq!(hex::encode(fresh), FRESH_KAT_V2);
    assert_eq!(hex::encode(published), PUBLISHED_KAT_V2);
    assert_eq!(hex::encode(proof), PROOF_KAT_V2);
    assert_eq!(hex::encode(reserved_verified), RESERVED_VERIFIED_KAT_V2);
    assert_eq!(fresh.len(), 640);
    assert_eq!(&proof[302..334], &[0x66; 32]);
    assert_eq!(&proof[398..430], &[0x33; 32]);
    assert_eq!(&proof[508..540], &[0xaa; 32]);
    assert_eq!(&proof[540..604], &[0; 64]);
    assert_eq!(&reserved_verified[540..572], &[0xbb; 32]);
    assert_eq!(&reserved_verified[572..604], &[0xcc; 32]);
}

#[test]
fn production_decoder_accepts_only_the_first_three_monotone_states() {
    let scope = fixture_scope();
    let key = stable_storage_key_v2(scope).expect("stable key");
    let (fresh, published, proof, reserved_verified) = fixture_records();
    assert!(matches!(
        decode_record_v2(scope, key, &fresh),
        Ok(DecodedStateV2::Fresh)
    ));
    assert!(matches!(
        decode_record_v2(scope, key, &published),
        Ok(DecodedStateV2::PublishedUnbound(_))
    ));
    assert!(matches!(
        decode_record_v2(scope, key, &proof),
        Ok(DecodedStateV2::ProofPublishedUnverified(_, _))
    ));
    assert!(decode_record_v2(scope, key, &reserved_verified).is_err());
}

#[test]
fn semantic_mutation_is_rejected_even_with_a_recomputed_footer() {
    let scope = fixture_scope();
    let key = stable_storage_key_v2(scope).expect("stable key");
    let (_, _, proof, _) = fixture_records();
    for index in [4, 5, 6, 7, 8, 80, 81, 114, 398, 430, 540, 604] {
        let mut mutation = proof;
        mutation[index] ^= 1;
        let digest = record_digest_v2(&mutation);
        mutation[RECORD_PREFIX_BYTES_V2..].copy_from_slice(&digest);
        assert!(
            decode_record_v2(scope, key, &mutation).is_err(),
            "offset {index}"
        );
    }
    for index in [302, 334, 366, 508] {
        let mut unauthenticated_corruption = proof;
        unauthenticated_corruption[index] ^= 1;
        assert!(
            decode_record_v2(scope, key, &unauthenticated_corruption).is_err(),
            "checksum offset {index}"
        );
    }
}
