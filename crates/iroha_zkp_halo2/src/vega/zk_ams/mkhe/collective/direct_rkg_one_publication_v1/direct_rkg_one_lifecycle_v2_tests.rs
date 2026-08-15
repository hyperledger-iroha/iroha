use super::*;
use crate::vega::zk_ams::mkhe::{
    ZkAmsMkhePartyIdV1,
    active_exact_binding::SealedDirectRkgOneProofOwnerV1,
    direct_object_transport::{ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1},
};

fn scope() -> DirectRkgOnePublicationScopeV1 {
    DirectRkgOnePublicationScopeV1 {
        context_digest: [0x11; 32],
        party_index: 2,
        digit_index: 3,
        party: ZkAmsMkhePartyIdV1::new([0x22; 32]).expect("nonzero party"),
    }
}

fn published_axes() -> PublishedAxesV2 {
    PublishedAxesV2 {
        publication_identity: [0x33; 32],
        h0: ZkAmsMkheDirectObjectPointerV1::new(
            ZkAmsMkheDirectObjectKindV1::RkgH0,
            super::super::RKG_ONE_POLYNOMIAL_BYTES_V1,
            [0x44; 32],
        )
        .expect("H0 pointer"),
        h1: ZkAmsMkheDirectObjectPointerV1::new(
            ZkAmsMkheDirectObjectKindV1::RkgH1,
            super::super::RKG_ONE_POLYNOMIAL_BYTES_V1,
            [0x55; 32],
        )
        .expect("H1 pointer"),
        receipt_set_digest: [0x66; 32],
        provider_identity: [0x77; 32],
        snapshot_identity: [0x88; 32],
    }
}

fn proof_axes() -> record_v2::ProofAxesV2 {
    record_v2::ProofAxesV2 {
        publication_identity: [0x33; 32],
        pointer: ZkAmsMkheDirectObjectPointerV1::new(
            ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
            SealedDirectRkgOneProofOwnerV1::CANONICAL_PROOF_BYTES_V1,
            [0x99; 32],
        )
        .expect("proof pointer"),
        receipt_digest: [0xaa; 32],
    }
}

#[test]
fn every_committed_lifecycle_state_roundtrips() {
    let scope = scope();
    let key = record_v2::stable_storage_key_v2(scope).expect("stable key");
    let mut record = [0; RECORD_BYTES_V2];
    record_v2::encode_fresh_v2(scope, key, &mut record).expect("fresh record");
    assert!(matches!(
        record_v2::decode_record_v2(scope, key, &record),
        Ok(DecodedStateV2::Fresh)
    ));

    let published = published_axes();
    record_v2::encode_published_v2(scope, key, published, &mut record).expect("published record");
    assert!(matches!(
        record_v2::decode_record_v2(scope, key, &record),
        Ok(DecodedStateV2::PublishedUnbound(found)) if found == published
    ));

    let proof = proof_axes();
    record_v2::encode_proof_v2(scope, key, published, proof, &mut record).expect("proof record");
    assert!(matches!(
        record_v2::decode_record_v2(scope, key, &record),
        Ok(DecodedStateV2::ProofPublishedUnverified(found, found_proof))
            if found == published && found_proof == proof
    ));
}

#[test]
fn lifecycle_records_reject_corruption_reserved_state_and_cross_identity_proofs() {
    let scope = scope();
    let key = record_v2::stable_storage_key_v2(scope).expect("stable key");
    let published = published_axes();
    let proof = proof_axes();
    let mut record = [0; RECORD_BYTES_V2];
    record_v2::encode_proof_v2(scope, key, published, proof, &mut record).expect("proof record");
    for offset in [0, 4, 5, 6, 7, 16, 48, 80, 81, 82, 114, 607, 608, 639] {
        let mut corrupted = record;
        corrupted[offset] ^= 1;
        assert!(
            record_v2::decode_record_v2(scope, key, &corrupted).is_err(),
            "accepted corruption at byte {offset}"
        );
    }

    record_v2::encode_reserved_verified_for_test_v2(
        scope,
        key,
        published,
        proof,
        [0xbb; 32],
        [0xcc; 32],
        &mut record,
    );
    assert!(record_v2::decode_record_v2(scope, key, &record).is_err());

    let mismatched = record_v2::ProofAxesV2 {
        publication_identity: [0xdd; 32],
        ..proof
    };
    assert!(record_v2::encode_proof_v2(scope, key, published, mismatched, &mut record).is_err());
}

#[test]
fn lifecycle_store_outcomes_remain_distinct() {
    fn assert_distinct<T>(values: &[T]) {
        for (index, value) in values.iter().enumerate() {
            for other in &values[index + 1..] {
                assert_ne!(
                    core::mem::discriminant(value),
                    core::mem::discriminant(other)
                );
            }
        }
    }

    assert_distinct(&[
        ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2::Absent,
        ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2::Legacy334,
        ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2::Lifecycle640,
    ]);
    assert_distinct(&[
        ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2::InsertedByThisCall,
        ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2::AlreadyPresent,
    ]);
    assert_distinct(&[
        ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::ExchangedByThisCall,
        ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::ExactReplay,
        ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::Conflict,
    ]);
}
