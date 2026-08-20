use super::*;
use crate::vega::zk_ams::mkhe::ZkAmsMkhePartyIdV1;
use std::{collections::BTreeSet, panic::AssertUnwindSafe};

const TX_ID_KAT_V1: &str = "3b0e7afc152418fd7158c31aaf3a8e2c13a18be6da045cb2a7abe5b44485e992";
const PUBLICATION_UNKNOWN_RECORD_KAT_V1: &str = "52314f4a0100000000000000000000003b0e7afc152418fd7158c31aaf3a8e2c13a18be6da045cb2a7abe5b44485e9921111111111111111111111111111111111111111111111111111111111111111030722222222222222222222222222222222222222222222222222222222222222220000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c6e06a44654d5a5c97fc5e326540591d85a5140b39d8e181df8c5118cb1e05d9";
const PUBLISHED_UNBOUND_RECORD_KAT_V1: &str = "52314f4a0101000000000000000000013b0e7afc152418fd7158c31aaf3a8e2c13a18be6da045cb2a7abe5b44485e99211111111111111111111111111111111111111111111111111111111111111110307222222222222222222222222222222222222222222222222222222222222222233333333333333333333333333333333333333333333333333333333333333335a444f500101000000000260000044444444444444444444444444444444444444444444444444444444444444445c1d074f316f6c1d384045487ec4f3512e16b3b24afaeccc72c9374cbcad1ee45a444f5001020000000002600000555555555555555555555555555555555555555555555555555555555555555575c5aadea9f435181af4ee8f24721f082f182d6d2d17e4c3f1427541fb2a20417961664c12196706b9966f33aecd358bcae23ffa6b685be58da603c83633d306";

type RecordV1 = [u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1];
fn fixture_scope() -> DirectRkgOnePublicationScopeV1 {
    DirectRkgOnePublicationScopeV1 {
        context_digest: [0x11; 32],
        party_index: 3,
        digit_index: 7,
        party: ZkAmsMkhePartyIdV1::new([0x22; 32]).expect("nonzero party"),
    }
}
fn pointer(kind: ZkAmsMkheDirectObjectKindV1, byte: u8) -> ZkAmsMkheDirectObjectPointerV1 {
    ZkAmsMkheDirectObjectPointerV1::new(kind, RKG_ONE_POLYNOMIAL_BYTES_V1, [byte; 32])
        .expect("canonical pointer")
}
fn fixture_axes() -> DirectRkgOnePublishedAxesV1 {
    DirectRkgOnePublishedAxesV1 {
        publication_identity: [0x33; 32],
        h0: pointer(ZkAmsMkheDirectObjectKindV1::RkgH0, 0x44),
        h1: pointer(ZkAmsMkheDirectObjectKindV1::RkgH1, 0x55),
    }
}
fn encoded_record(published: Option<DirectRkgOnePublishedAxesV1>) -> RecordV1 {
    let scope = fixture_scope();
    let mut record = [0; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1];
    encode_record_v1(
        scope,
        transaction_id_v1(scope).unwrap(),
        published,
        &mut record,
    )
    .unwrap();
    record
}
fn take_unknown(
    recovered: Result<DirectRkgOneRecoveredOrphanV1, ZkAmsMkheErrorV1>,
) -> DirectRkgOnePublicationUnknownV1 {
    match recovered.unwrap() {
        DirectRkgOneRecoveredOrphanV1::PublicationUnknown(value) => value,
        DirectRkgOneRecoveredOrphanV1::PublishedUnbound(_) => panic!("unexpected state"),
    }
}
fn assert_published(recovered: Result<DirectRkgOneRecoveredOrphanV1, ZkAmsMkheErrorV1>) {
    assert!(matches!(
        recovered.unwrap(),
        DirectRkgOneRecoveredOrphanV1::PublishedUnbound(_)
    ));
}
fn refresh_footer(record: &mut RecordV1) {
    let digest = record_digest_v1(record);
    record[RECORD_DIGEST_RANGE_V1].copy_from_slice(&digest);
}

#[derive(Clone, Copy)]
enum MutationMode {
    Ok,
    ErrorBefore,
    ErrorAfter,
    PanicBefore,
    PanicAfter,
}

struct TestStore {
    value: Option<RecordV1>,
    last_key: [u8; 32],
    loads: usize,
    puts: usize,
    compares: usize,
    dirty_absent: bool,
    load_error_at: Option<usize>,
    put_mode: MutationMode,
    compare_mode: MutationMode,
    put_competitor: Option<RecordV1>,
    compare_competitor: Option<RecordV1>,
}
impl Default for TestStore {
    fn default() -> Self {
        Self {
            value: None,
            last_key: [0; 32],
            loads: 0,
            puts: 0,
            compares: 0,
            dirty_absent: false,
            load_error_at: None,
            put_mode: MutationMode::Ok,
            compare_mode: MutationMode::Ok,
            put_competitor: None,
            compare_competitor: None,
        }
    }
}
impl DirectRkgOneOrphanJournalStoreV1 for TestStore {
    fn load_exact_v1(
        &mut self,
        storage_key: &[u8; 32],
        record: &mut RecordV1,
    ) -> Result<bool, ZkAmsMkheErrorV1> {
        self.loads += 1;
        self.last_key = *storage_key;
        if self.load_error_at == Some(self.loads) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        if let Some(value) = self.value {
            *record = value;
            Ok(true)
        } else {
            record.fill(u8::from(self.dirty_absent));
            Ok(false)
        }
    }

    fn put_if_absent_exact_v1(
        &mut self,
        storage_key: &[u8; 32],
        record: &RecordV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.puts += 1;
        self.last_key = *storage_key;
        match self.put_mode {
            MutationMode::ErrorBefore => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
            MutationMode::PanicBefore => panic!("put before mutation"),
            _ => {}
        }
        if let Some(competing) = self.put_competitor.take() {
            self.value = Some(competing);
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        if self.value.is_some() {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.value = Some(*record);
        match self.put_mode {
            MutationMode::ErrorAfter => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
            MutationMode::PanicAfter => panic!("put after mutation"),
            _ => Ok(()),
        }
    }

    fn compare_exchange_exact_v1(
        &mut self,
        storage_key: &[u8; 32],
        expected: &RecordV1,
        replacement: &RecordV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.compares += 1;
        self.last_key = *storage_key;
        match self.compare_mode {
            MutationMode::ErrorBefore => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
            MutationMode::PanicBefore => panic!("compare before mutation"),
            _ => {}
        }
        if let Some(competing) = self.compare_competitor.take() {
            self.value = Some(competing);
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        if self.value.as_ref() != Some(expected) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.value = Some(*replacement);
        match self.compare_mode {
            MutationMode::ErrorAfter => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
            MutationMode::PanicAfter => panic!("compare after mutation"),
            _ => Ok(()),
        }
    }
}

#[test]
fn independent_transaction_and_full_record_kats_are_literal() {
    let scope = fixture_scope();
    let id = transaction_id_v1(scope).expect("typed transaction identity");
    assert_eq!(hex::encode(id.0), TX_ID_KAT_V1);

    let publication_unknown = encoded_record(None);
    let published = encoded_record(Some(fixture_axes()));
    assert_eq!(publication_unknown.len(), 334);
    assert_eq!(published.len(), 334);
    assert_eq!(
        hex::encode(publication_unknown),
        PUBLICATION_UNKNOWN_RECORD_KAT_V1
    );
    assert_eq!(hex::encode(published), PUBLISHED_UNBOUND_RECORD_KAT_V1);
}

#[test]
fn transaction_identity_is_unique_for_all_release_party_digit_slots() {
    let mut ids = BTreeSet::new();
    for digit_index in 0_u8..38 {
        for party_index in 0_u8..8 {
            let mut context_digest = [0_u8; 32];
            context_digest[..2].copy_from_slice(&[digit_index + 1, party_index + 1]);
            let scope = DirectRkgOnePublicationScopeV1 {
                context_digest,
                party_index,
                digit_index,
                party: ZkAmsMkhePartyIdV1::new([party_index + 1; 32]).expect("nonzero party"),
            };
            assert!(ids.insert(transaction_id_v1(scope).expect("typed identity").0));
        }
    }
    assert_eq!(ids.len(), 8 * 38);
    let id = transaction_id_v1(fixture_scope()).expect("fixture identity");
    assert!(transaction_id_v1(fixture_scope()).unwrap() == id);
}

#[test]
fn semantic_record_mutations_fail_even_with_recomputed_footer() {
    let scope = fixture_scope();
    let id = transaction_id_v1(scope).unwrap();
    let unknown = encoded_record(None);
    let published = encoded_record(Some(fixture_axes()));
    let mut mutations = Vec::new();
    let mut value = unknown;
    value[114] = 1;
    mutations.push(value);
    let mut value = unknown;
    value[5] = 2;
    mutations.push(value);
    let mut value = published;
    value[8..16].copy_from_slice(&0_u64.to_be_bytes());
    mutations.push(value);
    let mut value = published;
    value[PUBLICATION_IDENTITY_RANGE_V1].fill(0);
    mutations.push(value);
    let mut value = published;
    value[16] ^= 1;
    mutations.push(value);
    let mut value = published;
    value[48] ^= 1;
    mutations.push(value);
    let mut value = published;
    value[80] = 8;
    mutations.push(value);
    let wrong_kind = pointer(ZkAmsMkheDirectObjectKindV1::RkgH1, 0x66);
    let mut value = published;
    value[H0_POINTER_RANGE_V1].copy_from_slice(&wrong_kind.encode());
    mutations.push(value);
    let wrong_size = ZkAmsMkheDirectObjectPointerV1::new(
        ZkAmsMkheDirectObjectKindV1::RkgH1,
        RKG_ONE_POLYNOMIAL_BYTES_V1 - 1,
        [0x77; 32],
    )
    .unwrap();
    let mut value = published;
    value[H1_POINTER_RANGE_V1].copy_from_slice(&wrong_size.encode());
    mutations.push(value);

    for mut mutation in mutations {
        refresh_footer(&mut mutation);
        assert!(decode_record_v1(scope, id, &mutation).is_err());
    }
}

#[test]
fn mutations_are_reloaded_once_and_lost_ack_is_reconciled() {
    let scope = fixture_scope();
    let id = transaction_id_v1(scope).unwrap();
    let mut lost_put = TestStore {
        put_mode: MutationMode::ErrorAfter,
        ..TestStore::default()
    };
    let _ = take_unknown(establish_or_recover_scope_v1(scope, &mut lost_put));
    assert_eq!((lost_put.loads, lost_put.puts), (2, 1));
    let mut store = TestStore::default();
    let unknown = take_unknown(establish_or_recover_scope_v1(scope, &mut store));
    assert_eq!((store.loads, store.puts, store.compares), (2, 1, 0));
    assert_eq!(store.last_key, id.0);

    store.compare_mode = MutationMode::ErrorAfter;
    let _published = persist_published_axes_v1(unknown, fixture_axes(), &mut store)
        .expect("durable desired bytes reconcile an error acknowledgement");
    assert_eq!((store.loads, store.puts, store.compares), (3, 1, 1));

    assert_published(establish_or_recover_scope_v1(scope, &mut store));
    assert_eq!((store.loads, store.puts, store.compares), (4, 1, 1));
}

#[test]
fn concurrent_exact_winners_are_reloaded_without_overwrite() {
    let scope = fixture_scope();
    let unknown_record = encoded_record(None);
    let desired = encoded_record(Some(fixture_axes()));
    for (winner, is_published) in [(unknown_record, false), (desired, true)] {
        let mut store = TestStore {
            put_competitor: Some(winner),
            ..TestStore::default()
        };
        let recovered = establish_or_recover_scope_v1(scope, &mut store).unwrap();
        assert_eq!(
            matches!(
                recovered,
                DirectRkgOneRecoveredOrphanV1::PublishedUnbound(_)
            ),
            is_published
        );
        assert_eq!((store.loads, store.puts), (2, 1));
        assert_eq!(store.value, Some(winner));
    }
    let mut different = fixture_axes();
    different.publication_identity = [0x99; 32];
    different.h1 = pointer(ZkAmsMkheDirectObjectKindV1::RkgH1, 0x66);
    let different = encoded_record(Some(different));
    assert_ne!(desired, different);
    for (winner, reconciles) in [(desired, true), (different, false)] {
        let mut store = TestStore::default();
        let unknown = take_unknown(establish_or_recover_scope_v1(scope, &mut store));
        store.compare_competitor = Some(winner);
        let result = persist_published_axes_v1(unknown, fixture_axes(), &mut store);
        assert_eq!(result.is_ok(), reconciles);
        assert_eq!((store.loads, store.compares), (3, 1));
        assert_eq!(store.value, Some(winner));
    }
}

#[test]
fn failed_mutations_return_no_post_state_and_never_overwrite() {
    let scope = fixture_scope();
    let mut before_put = TestStore {
        put_mode: MutationMode::ErrorBefore,
        ..TestStore::default()
    };
    assert!(establish_or_recover_scope_v1(scope, &mut before_put).is_err());
    assert_eq!((before_put.loads, before_put.puts), (2, 1));
    assert!(before_put.value.is_none());

    let mut before_compare = TestStore::default();
    let unknown = take_unknown(establish_or_recover_scope_v1(scope, &mut before_compare));
    let original = before_compare.value;
    before_compare.compare_mode = MutationMode::ErrorBefore;
    assert!(persist_published_axes_v1(unknown, fixture_axes(), &mut before_compare).is_err());
    assert_eq!(before_compare.value, original);
    let _ = take_unknown(establish_or_recover_scope_v1(scope, &mut before_compare));

    let mut malformed_absent = TestStore {
        dirty_absent: true,
        ..TestStore::default()
    };
    assert!(establish_or_recover_scope_v1(scope, &mut malformed_absent).is_err());
    assert_eq!(malformed_absent.puts, 0);
}

#[test]
fn failed_authoritative_reload_returns_nothing_and_fresh_recovery_is_required() {
    let scope = fixture_scope();
    let mut put_store = TestStore {
        load_error_at: Some(2),
        ..TestStore::default()
    };
    assert!(establish_or_recover_scope_v1(scope, &mut put_store).is_err());
    assert!(put_store.value.is_some());
    put_store.load_error_at = None;
    let _ = take_unknown(establish_or_recover_scope_v1(scope, &mut put_store));

    let mut compare_store = TestStore::default();
    let unknown = take_unknown(establish_or_recover_scope_v1(scope, &mut compare_store));
    compare_store.load_error_at = Some(compare_store.loads + 1);
    assert!(persist_published_axes_v1(unknown, fixture_axes(), &mut compare_store).is_err());
    compare_store.load_error_at = None;
    assert_published(establish_or_recover_scope_v1(scope, &mut compare_store));
}

#[test]
fn panic_returns_nothing_and_only_fresh_typed_recovery_observes_storage() {
    let scope = fixture_scope();
    let mut put_store = TestStore {
        put_mode: MutationMode::PanicAfter,
        ..TestStore::default()
    };
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = establish_or_recover_scope_v1(scope, &mut put_store);
    }));
    assert!(result.is_err());
    assert_eq!((put_store.loads, put_store.puts), (1, 1));
    put_store.put_mode = MutationMode::Ok;
    let _ = take_unknown(establish_or_recover_scope_v1(scope, &mut put_store));

    let mut compare_store = TestStore::default();
    let unknown = take_unknown(establish_or_recover_scope_v1(scope, &mut compare_store));
    compare_store.compare_mode = MutationMode::PanicAfter;
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = persist_published_axes_v1(unknown, fixture_axes(), &mut compare_store);
    }));
    assert!(result.is_err());
    compare_store.compare_mode = MutationMode::Ok;
    assert_published(establish_or_recover_scope_v1(scope, &mut compare_store));

    let mut before_panic = TestStore {
        put_mode: MutationMode::PanicBefore,
        ..TestStore::default()
    };
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = establish_or_recover_scope_v1(scope, &mut before_panic);
        }))
        .is_err()
    );
    assert!(before_panic.value.is_none());

    let mut before_compare = TestStore::default();
    let unknown = take_unknown(establish_or_recover_scope_v1(scope, &mut before_compare));
    before_compare.compare_mode = MutationMode::PanicBefore;
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = persist_published_axes_v1(unknown, fixture_axes(), &mut before_compare);
        }))
        .is_err()
    );
    before_compare.compare_mode = MutationMode::Ok;
    let _ = take_unknown(establish_or_recover_scope_v1(scope, &mut before_compare));
}

#[test]
fn scope_privacy_disconnect_and_review_caps_remain_closed() {
    let parent = include_str!("../direct_rkg_one_publication_v1.rs");
    let production = include_str!("direct_rkg_one_orphan_journal_v1.rs");
    let tests = include_str!("direct_rkg_one_orphan_journal_v1_tests.rs");
    let party_local = include_str!("../party_local_rkg_ephemeral_v1.rs");

    for required in [
        "context.target()",
        "ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization",
        "context.evaluated_key_ordinal()",
        "context.galois_exponent()",
        "party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1",
        "context.profile_digest() != roster.profile_digest()",
        "context.roster_digest() != roster.roster_digest()",
        "context.key_material_digest() != roster.key_material_digest()",
        "context.epoch() != roster.epoch()",
        "context.digest() == [0; 32]",
        ".get(party_index)",
        "party: participant.party()",
    ] {
        assert!(parent.contains(required));
    }
    assert!(!parent.contains("roster.validate()"));
    assert_eq!(
        parent
            .matches("mod direct_rkg_one_orphan_journal_v1;")
            .count(),
        1
    );
    assert!(!party_local.contains("direct_rkg_one_orphan_journal_v1"));
    assert!(production.contains("publication_unknown: DirectRkgOnePublicationUnknownV1"));
    let transaction_body = production
        .split("fn transaction_id_v1")
        .nth(1)
        .unwrap()
        .split("fn validate_published_axes_v1")
        .next()
        .unwrap();
    assert!(!transaction_body.contains("publication_identity"));
    assert_eq!(
        production
            .matches("let loaded = load_state_v1(scope, id, store")
            .count(),
        2
    );
    for forbidden in [
        "DirectRkgOneCandidateV1",
        "CompletedDirectRkgOneSemanticVerificationV1",
        "VerifiedDirectRelationProofReceiptV1",
        "ZkAmsMkheDirectVerifiedContributionV1",
        "VerifiedPersistentWitnessBindingV1",
        "ReadyRkg2",
        "ReleaseGate",
        "pub(super) struct DirectRkgOneOrphanTransactionIdV1",
        "Debug for DirectRkgOneOrphanTransactionIdV1",
        "fn transaction_id_bytes",
        "fn decode_transaction",
        "#[derive(Clone, Copy)]\npub(super) struct DirectRkgOnePublicationUnknownV1",
        "#[derive(Clone, Copy)]\npub(super) struct DirectRkgOnePublishedUnboundV1",
    ] {
        assert!(!production.contains(forbidden));
    }
    assert!(parent.lines().count() <= 500 && parent.len() <= 24 * 1024);
    assert!(production.lines().count() <= 500 && production.len() <= 24 * 1024);
    assert!(tests.lines().count() <= 500 && tests.len() <= 24 * 1024);
}
