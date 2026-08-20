use super::record_v2::*;
use super::*;
use crate::vega::zk_ams::mkhe::{
    ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2, ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2,
    ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2, ZkAmsMkhePartyIdV1,
    direct_object_transport::{ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1},
};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[expect(
    clippy::large_enum_variant,
    reason = "the test store keeps both exact persisted widths inline for Copy CAS snapshots"
)]
#[derive(Clone, Copy)]
enum Stored {
    Legacy([u8; LEGACY_RECORD_BYTES_V1]),
    Lifecycle(RecordV2),
}

#[derive(Clone, Copy, Default)]
enum MutationMode {
    #[default]
    Normal,
    ErrorBefore,
    ErrorAfter,
    PanicAfter,
}

struct TestStore {
    value: Option<Stored>,
    put_mode: MutationMode,
    cas_mode: MutationMode,
    put_competitor: Option<Stored>,
    cas_competitor: Option<Stored>,
    load_error_at: Option<usize>,
    dirty_absent: bool,
    dirty_legacy_tail: bool,
    loads: usize,
    puts: usize,
    compares: usize,
}

impl Default for TestStore {
    fn default() -> Self {
        Self {
            value: None,
            put_mode: MutationMode::Normal,
            cas_mode: MutationMode::Normal,
            put_competitor: None,
            cas_competitor: None,
            load_error_at: None,
            dirty_absent: false,
            dirty_legacy_tail: false,
            loads: 0,
            puts: 0,
            compares: 0,
        }
    }
}

impl ZkAmsMkheDirectRkgOneLifecycleStoreV2 for TestStore {
    fn load_exact_v2(
        &mut self,
        _storage_key: &[u8; 32],
        record: &mut RecordV2,
    ) -> Result<ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2, ZkAmsMkheErrorV1> {
        self.loads += 1;
        if self.load_error_at == Some(self.loads) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        record.fill(0);
        match self.value {
            None => {
                if self.dirty_absent {
                    record[RECORD_BYTES_V2 - 1] = 1;
                }
                Ok(ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2::Absent)
            }
            Some(Stored::Legacy(value)) => {
                record[..LEGACY_RECORD_BYTES_V1].copy_from_slice(&value);
                if self.dirty_legacy_tail {
                    record[LEGACY_RECORD_BYTES_V1] = 1;
                }
                Ok(ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2::Legacy334)
            }
            Some(Stored::Lifecycle(value)) => {
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
        if matches!(self.put_mode, MutationMode::ErrorBefore) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        if let Some(competitor) = self.put_competitor.take() {
            self.value = Some(competitor);
            return Ok(ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2::AlreadyPresent);
        }
        if self.value.is_some() {
            return Ok(ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2::AlreadyPresent);
        }
        self.value = Some(Stored::Lifecycle(*record));
        match self.put_mode {
            MutationMode::Normal => {
                Ok(ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2::InsertedByThisCall)
            }
            MutationMode::ErrorAfter => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
            MutationMode::PanicAfter => panic!("put panic after mutation"),
            MutationMode::ErrorBefore => unreachable!(),
        }
    }

    fn compare_exchange_exact_v2(
        &mut self,
        _storage_key: &[u8; 32],
        expected: &RecordV2,
        replacement: &RecordV2,
    ) -> Result<ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2, ZkAmsMkheErrorV1> {
        self.compares += 1;
        if matches!(self.cas_mode, MutationMode::ErrorBefore) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        if let Some(competitor) = self.cas_competitor.take() {
            self.value = Some(competitor);
        }
        let outcome = match self.value {
            Some(Stored::Lifecycle(value)) if value == *expected => {
                self.value = Some(Stored::Lifecycle(*replacement));
                ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::ExchangedByThisCall
            }
            Some(Stored::Lifecycle(value)) if value == *replacement => {
                ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::ExactReplay
            }
            _ => ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::Conflict,
        };
        match self.cas_mode {
            MutationMode::Normal => Ok(outcome),
            MutationMode::ErrorAfter => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
            MutationMode::PanicAfter => panic!("CAS panic after mutation"),
            MutationMode::ErrorBefore => unreachable!(),
        }
    }
}

fn fixture_scope() -> DirectRkgOnePublicationScopeV1 {
    DirectRkgOnePublicationScopeV1 {
        context_digest: [0x11; 32],
        party_index: 3,
        digit_index: 7,
        party: ZkAmsMkhePartyIdV1::new([0x22; 32]).expect("nonzero party"),
    }
}

fn fixture_records() -> (RecordV2, RecordV2, RecordV2) {
    let scope = fixture_scope();
    let key = stable_storage_key_v2(scope).expect("stable key");
    let published_axes = PublishedAxesV2 {
        publication_identity: [0x33; 32],
        h0: ZkAmsMkheDirectObjectPointerV1::new(
            ZkAmsMkheDirectObjectKindV1::RkgH0,
            39_845_888,
            [0x44; 32],
        )
        .expect("H0 pointer"),
        h1: ZkAmsMkheDirectObjectPointerV1::new(
            ZkAmsMkheDirectObjectKindV1::RkgH1,
            39_845_888,
            [0x55; 32],
        )
        .expect("H1 pointer"),
        receipt_set_digest: [0x66; 32],
        provider_identity: [0x77; 32],
        snapshot_identity: [0x88; 32],
    };
    let proof_axes = ProofAxesV2 {
        publication_identity: published_axes.publication_identity,
        pointer: ZkAmsMkheDirectObjectPointerV1::new(
            ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
            25_248_766,
            [0x99; 32],
        )
        .expect("proof pointer"),
        receipt_digest: [0xaa; 32],
    };
    let mut fresh = [0; RECORD_BYTES_V2];
    let mut published = [0; RECORD_BYTES_V2];
    let mut proof = [0; RECORD_BYTES_V2];
    encode_fresh_v2(scope, key, &mut fresh).expect("fresh record");
    encode_published_v2(scope, key, published_axes, &mut published).expect("published record");
    encode_proof_v2(scope, key, published_axes, proof_axes, &mut proof).expect("proof record");
    (fresh, published, proof)
}

#[test]
fn fresh_permit_requires_absent_inserted_by_this_call_and_exact_reload() {
    let scope = fixture_scope();
    let mut store = TestStore::default();
    assert!(matches!(
        reserve_scope_v2(scope, &mut store),
        Ok(DirectRkgOneFreshReservationOutcomeV2::Reserved(_))
    ));
    assert_eq!((store.loads, store.puts), (2, 1));
    assert!(matches!(
        reserve_scope_v2(scope, &mut store),
        Ok(DirectRkgOneFreshReservationOutcomeV2::Quarantined(
            DirectRkgOneLifecycleObservationV2::FreshQuarantined
        ))
    ));
    assert_eq!((store.loads, store.puts), (3, 1));
}

#[test]
fn legacy_and_racing_fresh_are_quarantined_without_a_permit() {
    let scope = fixture_scope();
    let (fresh, _, _) = fixture_records();
    let mut legacy = TestStore {
        value: Some(Stored::Legacy([0x42; LEGACY_RECORD_BYTES_V1])),
        ..TestStore::default()
    };
    assert!(matches!(
        reserve_scope_v2(scope, &mut legacy),
        Ok(DirectRkgOneFreshReservationOutcomeV2::Quarantined(
            DirectRkgOneLifecycleObservationV2::LegacyV1Quarantined
        ))
    ));
    assert_eq!((legacy.loads, legacy.puts), (1, 0));

    let mut race = TestStore {
        put_competitor: Some(Stored::Lifecycle(fresh)),
        ..TestStore::default()
    };
    assert!(matches!(
        reserve_scope_v2(scope, &mut race),
        Ok(DirectRkgOneFreshReservationOutcomeV2::Quarantined(
            DirectRkgOneLifecycleObservationV2::FreshQuarantined
        ))
    ));
    assert_eq!((race.loads, race.puts), (2, 1));
}

#[test]
fn lost_ack_and_panic_after_put_never_recreate_a_fresh_permit() {
    let scope = fixture_scope();
    let mut error_before = TestStore {
        put_mode: MutationMode::ErrorBefore,
        ..TestStore::default()
    };
    assert!(reserve_scope_v2(scope, &mut error_before).is_err());
    assert_eq!((error_before.loads, error_before.puts), (2, 1));
    assert!(error_before.value.is_none());

    let mut lost_ack = TestStore {
        put_mode: MutationMode::ErrorAfter,
        ..TestStore::default()
    };
    assert!(reserve_scope_v2(scope, &mut lost_ack).is_err());
    assert_eq!((lost_ack.loads, lost_ack.puts), (2, 1));
    lost_ack.put_mode = MutationMode::Normal;
    assert!(matches!(
        reserve_scope_v2(scope, &mut lost_ack),
        Ok(DirectRkgOneFreshReservationOutcomeV2::Quarantined(_))
    ));

    let mut reload_error = TestStore {
        load_error_at: Some(2),
        ..TestStore::default()
    };
    assert!(reserve_scope_v2(scope, &mut reload_error).is_err());
    assert_eq!((reload_error.loads, reload_error.puts), (2, 1));
    reload_error.load_error_at = None;
    assert!(matches!(
        reserve_scope_v2(scope, &mut reload_error),
        Ok(DirectRkgOneFreshReservationOutcomeV2::Quarantined(_))
    ));

    let mut panic_after = TestStore {
        put_mode: MutationMode::PanicAfter,
        ..TestStore::default()
    };
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = reserve_scope_v2(scope, &mut panic_after);
        }))
        .is_err()
    );
    panic_after.put_mode = MutationMode::Normal;
    assert!(matches!(
        reserve_scope_v2(scope, &mut panic_after),
        Ok(DirectRkgOneFreshReservationOutcomeV2::Quarantined(_))
    ));
}

#[test]
fn every_cas_is_reloaded_once_and_only_this_calls_exchange_succeeds() {
    let scope = fixture_scope();
    let key = stable_storage_key_v2(scope).expect("stable key");
    let (fresh, published, proof) = fixture_records();
    let mut store = TestStore {
        value: Some(Stored::Lifecycle(fresh)),
        ..TestStore::default()
    };
    exchange_exact_v2(scope, key, &fresh, &published, &mut store).expect("own exchange");
    assert_eq!((store.loads, store.compares), (1, 1));
    assert!(exchange_exact_v2(scope, key, &fresh, &published, &mut store).is_err());
    assert_eq!((store.loads, store.compares), (2, 2));

    store.value = Some(Stored::Lifecycle(proof));
    assert!(exchange_exact_v2(scope, key, &fresh, &published, &mut store).is_err());
    assert_eq!((store.loads, store.compares), (3, 3));
}

#[test]
fn cas_error_or_panic_after_write_returns_no_successor_and_leaves_observation_only() {
    let scope = fixture_scope();
    let key = stable_storage_key_v2(scope).expect("stable key");
    let (fresh, published, _) = fixture_records();
    let mut lost_ack = TestStore {
        value: Some(Stored::Lifecycle(fresh)),
        cas_mode: MutationMode::ErrorAfter,
        ..TestStore::default()
    };
    assert!(exchange_exact_v2(scope, key, &fresh, &published, &mut lost_ack).is_err());
    assert_eq!((lost_ack.loads, lost_ack.compares), (1, 1));
    assert!(matches!(
        load_v2(scope, key, &mut lost_ack, &mut [0; RECORD_BYTES_V2]),
        Ok(LoadedV2::Lifecycle(DecodedStateV2::PublishedUnbound(_)))
    ));

    let mut error_before = TestStore {
        value: Some(Stored::Lifecycle(fresh)),
        cas_mode: MutationMode::ErrorBefore,
        ..TestStore::default()
    };
    assert!(exchange_exact_v2(scope, key, &fresh, &published, &mut error_before).is_err());
    assert_eq!((error_before.loads, error_before.compares), (1, 1));

    let mut reload_error = TestStore {
        value: Some(Stored::Lifecycle(fresh)),
        load_error_at: Some(1),
        ..TestStore::default()
    };
    assert!(exchange_exact_v2(scope, key, &fresh, &published, &mut reload_error).is_err());
    assert_eq!((reload_error.loads, reload_error.compares), (1, 1));

    let mut panic_after = TestStore {
        value: Some(Stored::Lifecycle(fresh)),
        cas_mode: MutationMode::PanicAfter,
        ..TestStore::default()
    };
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = exchange_exact_v2(scope, key, &fresh, &published, &mut panic_after);
        }))
        .is_err()
    );
    panic_after.cas_mode = MutationMode::Normal;
    assert!(matches!(
        load_v2(scope, key, &mut panic_after, &mut [0; RECORD_BYTES_V2]),
        Ok(LoadedV2::Lifecycle(DecodedStateV2::PublishedUnbound(_)))
    ));
}

#[test]
fn actual_width_and_clean_padding_are_mandatory() {
    let scope = fixture_scope();
    let mut dirty_absent = TestStore {
        dirty_absent: true,
        ..TestStore::default()
    };
    assert!(reserve_scope_v2(scope, &mut dirty_absent).is_err());
    assert_eq!(dirty_absent.puts, 0);

    let mut dirty_legacy = TestStore {
        value: Some(Stored::Legacy([0x42; LEGACY_RECORD_BYTES_V1])),
        dirty_legacy_tail: true,
        ..TestStore::default()
    };
    assert!(reserve_scope_v2(scope, &mut dirty_legacy).is_err());
    assert_eq!(dirty_legacy.puts, 0);
}

#[test]
fn proof_identity_axes_reject_all_six_h0_h1_reuses() {
    let proof = [[0x11; 32], [0x12; 32], [0x13; 32]];
    let h0 = [[0x21; 32], [0x22; 32], [0x23; 32]];
    let h1 = [[0x31; 32], [0x32; 32], [0x33; 32]];
    assert!(proof_identity_axes_are_distinct_v2(proof, h0, h1));
    for axis in 0..3 {
        for reused in [h0[axis], h1[axis]] {
            let mut candidate = proof;
            candidate[axis] = reused;
            assert!(!proof_identity_axes_are_distinct_v2(candidate, h0, h1));
        }
    }
}

fn assert_ordered(source: &str, snippets: &[&str]) {
    let mut cursor = 0;
    for snippet in snippets {
        let offset = source[cursor..]
            .find(snippet)
            .unwrap_or_else(|| panic!("missing ordered source: {snippet}"));
        cursor += offset + snippet.len();
    }
}

#[test]
fn source_corridor_is_ordered_private_and_has_no_runtime_backend() {
    let lifecycle = include_str!("direct_rkg_one_lifecycle_v2.rs");
    let record = include_str!("direct_rkg_one_lifecycle_record_v2.rs");
    let creator = include_str!("../direct_rkg_one_creator_v2.rs");
    let publication = include_str!("../direct_rkg_one_publication_v1.rs");
    let sealed = include_str!("../direct_rkg_one_sealed_candidate_v1.rs");
    let prover = include_str!(
        "../../active_exact_binding/direct_relation_wire_v1/rkg_one_creator_prover_v1.rs"
    );
    let mkhe = include_str!("../../../mkhe.rs");
    assert_ordered(
        record,
        &[
            "fn receipt_identity_axes_v2(",
            "receipt.staging_identity()",
            "receipt.seal_identity()",
            ".published_object_identity()",
            "fn proof_axes_v2(",
            "published_axes_v2(publication)? != published",
            "receipt_identity_axes_v2(receipt)",
            "receipt_identity_axes_v2(h0)",
            "receipt_identity_axes_v2(h1)",
            "proof_identity_axes_are_distinct_v2(",
        ],
    );
    assert!(lifecycle.contains(
        "record_v2::proof_axes_v2(\n        published.axes,\n        &publication_owner,\n        proof_owner.publication_receipt_v2(),\n    )?;"
    ));
    assert_ordered(
        creator,
        &[
            "reserve_direct_rkg_one_fresh_v2(",
            "take_ready_direct_rkg_one_prover_session_v1(",
            "publish_direct_rkg_one_h0_h1_v1(",
            "prepare_direct_rkg_one_statement_permit_v1(",
            "seal_direct_rkg_one_proof_owner_v1(",
            "publish_unverified_v2(provider)?",
            "persist_direct_rkg_one_proof_published_unverified_v2(",
            "from_durable_parts_v2(lifecycle_owner)",
            "verify_semantic_candidate_v1(context, statement_objects, provider)",
        ],
    );
    let after_take = creator
        .split_once("let prover_session = take_ready_direct_rkg_one_prover_session_v1(")
        .expect("state take")
        .1;
    assert!(!after_take.contains("state."));
    assert_eq!(
        publication
            .matches("fn publish_direct_rkg_one_h0_h1_v1")
            .count(),
        1
    );
    assert!(
        publication
            .contains("fresh: direct_rkg_one_lifecycle_v2::DirectRkgOneFreshPublishPermitV2")
    );
    assert_ordered(
        publication,
        &[
            "validate_publication_pair_v1(&owner)?",
            "persist_direct_rkg_one_published_unbound_v2(",
            "Ok((completed_and_h1.0, owner, lifecycle))",
        ],
    );
    assert!(
        prover.contains("for chunk in proof_bytes.chunks(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1)")
    );
    assert_ordered(prover, &["transaction.finish()?", "sealed: self"]);
    assert!(!prover.contains("impl<'a> SealedDirectRkgOneProofOwnerV1<'a> {\n    pub(in crate::vega::zk_ams::mkhe) fn verify_semantic_candidate_v1"));
    assert!(sealed.contains("pub(super) const fn from_durable_parts_v2"));
    assert!(mkhe.contains("pub trait ZkAmsMkheDirectRkgOneLifecycleStoreV2"));
    assert!(!mkhe.contains("impl ZkAmsMkheDirectRkgOneLifecycleStoreV2 for"));
    assert!(!creator.contains("pub fn create_direct_rkg_one_sealed_candidate_v2"));
    assert!(!record.contains("ReadyRkg2"));
    assert!(!lifecycle.contains("callback"));
    for source in [
        lifecycle,
        record,
        creator,
        sealed,
        prover,
        include_str!("direct_rkg_one_lifecycle_v2_tests.rs"),
        include_str!("direct_rkg_one_lifecycle_v2_kats.rs"),
    ] {
        assert!(source.lines().count() <= 500);
        assert!(source.len() <= 24 * 1024);
    }
}
