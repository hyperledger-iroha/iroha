use super::*;
use crate::vega::zk_ams::mkhe::{
    ZkAmsMkhePartyIdV1,
    active_exact_binding::SealedDirectRkgOneProofOwnerV1,
    direct_object_transport::{ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1},
};

pub(super) fn fixture_scope_v2() -> DirectRkgOnePublicationScopeV1 {
    DirectRkgOnePublicationScopeV1 {
        context_digest: [0x11; 32],
        party_index: 3,
        digit_index: 7,
        party: ZkAmsMkhePartyIdV1::new([0x22; 32]).expect("nonzero party"),
    }
}

fn pointer_v2(
    kind: ZkAmsMkheDirectObjectKindV1,
    payload_bytes: u64,
    byte: u8,
) -> ZkAmsMkheDirectObjectPointerV1 {
    ZkAmsMkheDirectObjectPointerV1::new(kind, payload_bytes, [byte; 32]).expect("canonical pointer")
}

pub(super) fn fixture_published_axes_v2() -> PublishedAxesV2 {
    PublishedAxesV2 {
        publication_identity: [0x33; 32],
        h0: pointer_v2(
            ZkAmsMkheDirectObjectKindV1::RkgH0,
            super::super::RKG_ONE_POLYNOMIAL_BYTES_V1,
            0x44,
        ),
        h1: pointer_v2(
            ZkAmsMkheDirectObjectKindV1::RkgH1,
            super::super::RKG_ONE_POLYNOMIAL_BYTES_V1,
            0x55,
        ),
        receipt_set_digest: [0x66; 32],
        provider_identity: [0x77; 32],
        snapshot_identity: [0x88; 32],
    }
}

pub(super) fn fixture_proof_axes_v2() -> record_v2::ProofAxesV2 {
    record_v2::ProofAxesV2 {
        publication_identity: fixture_published_axes_v2().publication_identity,
        pointer: pointer_v2(
            ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
            SealedDirectRkgOneProofOwnerV1::CANONICAL_PROOF_BYTES_V1,
            0x99,
        ),
        receipt_digest: [0xaa; 32],
    }
}

pub(super) fn fixture_records_v2() -> (RecordV2, RecordV2, RecordV2) {
    let scope = fixture_scope_v2();
    let key = record_v2::stable_storage_key_v2(scope).expect("stable key");
    let [mut fresh, mut published, mut proof] = [[0; RECORD_BYTES_V2]; 3];
    record_v2::encode_fresh_v2(scope, key, &mut fresh).expect("fresh record");
    record_v2::encode_published_v2(scope, key, fixture_published_axes_v2(), &mut published)
        .expect("published record");
    record_v2::encode_proof_v2(
        scope,
        key,
        fixture_published_axes_v2(),
        fixture_proof_axes_v2(),
        &mut proof,
    )
    .expect("proof record");
    (fresh, published, proof)
}

fn refresh_record_v2(record: &mut RecordV2) {
    let digest = record_v2::record_digest_v2(record);
    record[RECORD_BYTES_V2 - 32..].copy_from_slice(&digest);
}

#[derive(Clone, Copy)]
enum StoredV2 {
    Legacy([u8; LEGACY_RECORD_BYTES_V1]),
    Lifecycle(RecordV2),
}

#[derive(Clone, Copy, Default)]
enum PutModeV2 {
    #[default]
    Normal,
    AlreadyExact,
    InsertedCorrupt,
    ErrorAfterExact,
}

#[derive(Clone, Copy, Default)]
enum CasModeV2 {
    #[default]
    Normal,
    ExactReplay,
    Conflict,
    ExchangedCorrupt,
    ErrorAfterExact,
}

#[derive(Default)]
struct TestStoreV2 {
    value: Option<StoredV2>,
    dirty_absent: bool,
    dirty_legacy_tail: bool,
    put_mode: PutModeV2,
    cas_mode: CasModeV2,
    loads: usize,
    puts: usize,
    compares: usize,
}

impl TestStoreV2 {
    fn lifecycle(record: RecordV2) -> Self {
        Self {
            value: Some(StoredV2::Lifecycle(record)),
            ..Self::default()
        }
    }
}

impl ZkAmsMkheDirectRkgOneLifecycleStoreV2 for TestStoreV2 {
    fn load_exact_v2(
        &mut self,
        _storage_key: &[u8; 32],
        record: &mut RecordV2,
    ) -> Result<ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2, ZkAmsMkheErrorV1> {
        self.loads += 1;
        match self.value {
            None => {
                record.fill(u8::from(self.dirty_absent));
                Ok(ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2::Absent)
            }
            Some(StoredV2::Legacy(value)) => {
                record[..LEGACY_RECORD_BYTES_V1].copy_from_slice(&value);
                record[LEGACY_RECORD_BYTES_V1..].fill(u8::from(self.dirty_legacy_tail));
                Ok(ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2::Legacy334)
            }
            Some(StoredV2::Lifecycle(value)) => {
                *record = value;
                Ok(ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2::Lifecycle640)
            }
        }
    }

    fn put_if_absent_exact_v2(
        &mut self,
        _storage_key: &[u8; 32],
        record: &RecordV2,
    ) -> Result<ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2, ZkAmsMkheErrorV1> {
        self.puts += 1;
        match self.put_mode {
            PutModeV2::Normal if self.value.is_none() => {
                self.value = Some(StoredV2::Lifecycle(*record));
                Ok(ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2::InsertedByThisCall)
            }
            PutModeV2::Normal => Ok(ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2::AlreadyPresent),
            PutModeV2::AlreadyExact => {
                self.value = Some(StoredV2::Lifecycle(*record));
                Ok(ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2::AlreadyPresent)
            }
            PutModeV2::InsertedCorrupt => {
                let mut corrupt = *record;
                corrupt[RECORD_BYTES_V2 - 1] ^= 1;
                self.value = Some(StoredV2::Lifecycle(corrupt));
                Ok(ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2::InsertedByThisCall)
            }
            PutModeV2::ErrorAfterExact => {
                self.value = Some(StoredV2::Lifecycle(*record));
                Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
            }
        }
    }

    fn compare_exchange_exact_v2(
        &mut self,
        _storage_key: &[u8; 32],
        expected: &RecordV2,
        replacement: &RecordV2,
    ) -> Result<ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2, ZkAmsMkheErrorV1> {
        self.compares += 1;
        match self.cas_mode {
            CasModeV2::Normal if matches!(self.value, Some(StoredV2::Lifecycle(value)) if value == *expected) =>
            {
                self.value = Some(StoredV2::Lifecycle(*replacement));
                Ok(ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::ExchangedByThisCall)
            }
            CasModeV2::Normal => Ok(ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::Conflict),
            CasModeV2::ExactReplay => {
                self.value = Some(StoredV2::Lifecycle(*replacement));
                Ok(ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::ExactReplay)
            }
            CasModeV2::Conflict => {
                let mut competing = *replacement;
                competing[302] ^= 1;
                refresh_record_v2(&mut competing);
                self.value = Some(StoredV2::Lifecycle(competing));
                Ok(ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::Conflict)
            }
            CasModeV2::ExchangedCorrupt => {
                let mut corrupt = *replacement;
                corrupt[RECORD_BYTES_V2 - 1] ^= 1;
                self.value = Some(StoredV2::Lifecycle(corrupt));
                Ok(ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::ExchangedByThisCall)
            }
            CasModeV2::ErrorAfterExact => {
                self.value = Some(StoredV2::Lifecycle(*replacement));
                Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
            }
        }
    }
}

#[test]
fn reservation_mints_only_the_unique_insert_winner_and_recovery_is_observation_only() {
    let scope = fixture_scope_v2();
    let mut store = TestStoreV2::default();
    let permit = match reserve_scope_v2(scope, &mut store).unwrap() {
        DirectRkgOneFreshReservationOutcomeV2::Reserved(permit) => permit,
        DirectRkgOneFreshReservationOutcomeV2::Quarantined(_) => panic!("unique insert lost"),
    };
    assert!(matches!(
        recover_scope_v2(scope, &mut store),
        Ok(Some(DirectRkgOneLifecycleObservationV2::FreshQuarantined))
    ));
    assert!(matches!(
        reserve_scope_v2(scope, &mut store),
        Ok(DirectRkgOneFreshReservationOutcomeV2::Quarantined(
            DirectRkgOneLifecycleObservationV2::FreshQuarantined
        ))
    ));
    assert_eq!(store.puts, 1);

    let mut wrong_scope = permit.scope;
    wrong_scope.context_digest[0] ^= 1;
    assert!(validate_fresh_publish_permit_v2(&permit, wrong_scope).is_err());

    let mut ambiguous = TestStoreV2 {
        put_mode: PutModeV2::AlreadyExact,
        ..TestStoreV2::default()
    };
    assert!(matches!(
        reserve_scope_v2(scope, &mut ambiguous),
        Ok(DirectRkgOneFreshReservationOutcomeV2::Quarantined(
            DirectRkgOneLifecycleObservationV2::FreshQuarantined
        ))
    ));

    let mut lost_error = TestStoreV2 {
        put_mode: PutModeV2::ErrorAfterExact,
        ..TestStoreV2::default()
    };
    assert!(reserve_scope_v2(scope, &mut lost_error).is_err());
    assert!(matches!(
        recover_scope_v2(scope, &mut lost_error),
        Ok(Some(DirectRkgOneLifecycleObservationV2::FreshQuarantined))
    ));

    let mut false_winner = TestStoreV2 {
        put_mode: PutModeV2::InsertedCorrupt,
        ..TestStoreV2::default()
    };
    assert!(reserve_scope_v2(scope, &mut false_winner).is_err());
}

#[test]
fn exact_width_corruption_scope_and_reserved_state_are_fail_closed() {
    let scope = fixture_scope_v2();
    let key = record_v2::stable_storage_key_v2(scope).unwrap();
    let (fresh, mut published, proof) = fixture_records_v2();
    let mut out = [0xa5; RECORD_BYTES_V2];
    let mut absent = TestStoreV2::default();
    assert!(matches!(
        load_v2(scope, key, &mut absent, &mut out),
        Ok(LoadedV2::Absent)
    ));
    assert_eq!(out, [0; RECORD_BYTES_V2]);

    let mut dirty_absent = TestStoreV2 {
        dirty_absent: true,
        ..TestStoreV2::default()
    };
    assert!(load_v2(scope, key, &mut dirty_absent, &mut out).is_err());

    let mut legacy = TestStoreV2 {
        value: Some(StoredV2::Legacy([0x5a; LEGACY_RECORD_BYTES_V1])),
        ..TestStoreV2::default()
    };
    assert!(matches!(
        load_v2(scope, key, &mut legacy, &mut out),
        Ok(LoadedV2::Legacy)
    ));
    assert_eq!(
        out[LEGACY_RECORD_BYTES_V1..],
        [0; RECORD_BYTES_V2 - LEGACY_RECORD_BYTES_V1]
    );
    legacy.dirty_legacy_tail = true;
    assert!(load_v2(scope, key, &mut legacy, &mut out).is_err());

    let mut torn = fresh;
    torn[200] ^= 1;
    assert!(load_v2(scope, key, &mut TestStoreV2::lifecycle(torn), &mut out).is_err());

    published[600] = 1;
    refresh_record_v2(&mut published);
    assert!(record_v2::decode_record_v2(scope, key, &published).is_err());

    let mut spliced_proof = proof;
    spliced_proof[398] ^= 1;
    refresh_record_v2(&mut spliced_proof);
    assert!(record_v2::decode_record_v2(scope, key, &spliced_proof).is_err());

    let mut reserved = [0; RECORD_BYTES_V2];
    record_v2::encode_reserved_verified_for_test_v2(
        scope,
        key,
        fixture_published_axes_v2(),
        fixture_proof_axes_v2(),
        [0xbb; 32],
        [0xcc; 32],
        &mut reserved,
    );
    assert!(record_v2::decode_record_v2(scope, key, &reserved).is_err());

    let mut other_scope = scope;
    other_scope.context_digest[0] ^= 1;
    let other_key = record_v2::stable_storage_key_v2(other_scope).unwrap();
    assert!(
        load_v2(
            other_scope,
            other_key,
            &mut TestStoreV2::lifecycle(fresh),
            &mut out
        )
        .is_err()
    );
}

#[test]
fn exact_cas_replay_conflict_mutation_error_and_false_winner_mint_nothing() {
    let scope = fixture_scope_v2();
    let key = record_v2::stable_storage_key_v2(scope).unwrap();
    let (fresh, published, proof) = fixture_records_v2();
    let axes = fixture_published_axes_v2();

    let state = exchange_by_this_call_v2(
        scope,
        &key,
        &fresh,
        &published,
        &mut TestStoreV2::lifecycle(fresh),
    )
    .expect("unique CAS winner");
    assert!(matches!(
        state,
        DecodedStateV2::PublishedUnbound(found) if found == axes
    ));

    for mode in [
        CasModeV2::ExactReplay,
        CasModeV2::Conflict,
        CasModeV2::ErrorAfterExact,
        CasModeV2::ExchangedCorrupt,
    ] {
        let mut store = TestStoreV2 {
            cas_mode: mode,
            ..TestStoreV2::lifecycle(fresh)
        };
        assert!(exchange_by_this_call_v2(scope, &key, &fresh, &published, &mut store).is_err());
        assert_eq!(store.compares, 1);
        assert_eq!(store.loads, 1);
    }

    let state = exchange_by_this_call_v2(
        scope,
        &key,
        &published,
        &proof,
        &mut TestStoreV2::lifecycle(published),
    )
    .expect("unique proof CAS winner");
    assert!(matches!(
        state,
        DecodedStateV2::ProofPublishedUnverified(found, proof_found)
            if found == axes && proof_found == fixture_proof_axes_v2()
    ));
}

#[test]
fn every_committed_lifecycle_state_roundtrips() {
    let scope = fixture_scope_v2();
    let key = record_v2::stable_storage_key_v2(scope).expect("stable key");
    let mut record = [0; RECORD_BYTES_V2];
    record_v2::encode_fresh_v2(scope, key, &mut record).expect("fresh record");
    assert!(matches!(
        record_v2::decode_record_v2(scope, key, &record),
        Ok(DecodedStateV2::Fresh)
    ));

    let published = fixture_published_axes_v2();
    record_v2::encode_published_v2(scope, key, published, &mut record).expect("published record");
    assert!(matches!(
        record_v2::decode_record_v2(scope, key, &record),
        Ok(DecodedStateV2::PublishedUnbound(found)) if found == published
    ));

    let proof = fixture_proof_axes_v2();
    record_v2::encode_proof_v2(scope, key, published, proof, &mut record).expect("proof record");
    assert!(matches!(
        record_v2::decode_record_v2(scope, key, &record),
        Ok(DecodedStateV2::ProofPublishedUnverified(found, found_proof))
            if found == published && found_proof == proof
    ));
}

#[test]
fn lifecycle_records_reject_corruption_reserved_state_and_cross_identity_proofs() {
    let scope = fixture_scope_v2();
    let key = record_v2::stable_storage_key_v2(scope).expect("stable key");
    let published = fixture_published_axes_v2();
    let proof = fixture_proof_axes_v2();
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
