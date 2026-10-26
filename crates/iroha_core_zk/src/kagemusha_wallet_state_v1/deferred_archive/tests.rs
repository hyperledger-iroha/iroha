//! Bounded queue index ordering and current persisted schema limits; explicit memory store.
use super::*;
use crate::kagemusha_wallet_state_v1::tests::{MemoryArchive, bootstrap, fixture, wallet};

#[test]
fn ready_order_uses_intake_ordinal_even_when_verification_arrives_in_reverse_order() {
    let mut archive = MemoryArchive::new();
    let mut index = DeferredIndex {
        next_ordinal: 3,
        ..DeferredIndex::default()
    };
    index
        .insert_ready(&mut archive, [2; 32], [12; 32], 2)
        .unwrap();
    assert_eq!(
        index.ready.predecessor(&mut archive, &[255; 32]).unwrap(),
        Some((order_key(2), vec![2; 32]))
    );
    index
        .insert_ready(&mut archive, [1; 32], [11; 32], 1)
        .unwrap();
    index
        .insert_ready(&mut archive, [3; 32], [13; 32], 3)
        .unwrap();
    let mut reopened: DeferredIndex = archive::decode(&archive::encode(&index).unwrap()).unwrap();
    for ordinal in 1..=3 {
        let (key, value) = reopened
            .ready
            .predecessor(&mut archive, &[255; 32])
            .unwrap()
            .unwrap();
        assert_eq!(key, order_key(ordinal));
        assert_eq!(value, vec![ordinal as u8; 32]);
        reopened.ready = reopened.ready.remove(&mut archive, &key).unwrap();
    }
    assert!(
        reopened
            .ready
            .predecessor(&mut archive, &[255; 32])
            .unwrap()
            .is_none()
    );
}

#[test]
fn count_trigger_is_not_a_cap_and_overflow_or_duplicate_cannot_rewrite_the_root() {
    let mut archive = MemoryArchive::new();
    let mut index = DeferredIndex {
        next_ordinal: 18,
        ..DeferredIndex::default()
    };
    for ordinal in 1..=17 {
        index
            .insert_ready(
                &mut archive,
                [ordinal as u8; 32],
                [ordinal as u8; 32],
                ordinal,
            )
            .unwrap();
    }
    assert_eq!(
        index.count, 17,
        "sixteen is a scheduling trigger, not capacity"
    );
    let before = archive::encode(&index).unwrap();
    assert!(
        index
            .insert_ready(&mut archive, [1; 32], [99; 32], 18)
            .is_err()
    );
    assert_eq!(archive::encode(&index).unwrap(), before);
    index.count = u64::MAX;
    let before = archive::encode(&index).unwrap();
    assert!(matches!(
        index.insert_ready(&mut archive, [18; 32], [18; 32], 18),
        Err(Error::Invalid("deferred count exhausted"))
    ));
    assert_eq!(archive::encode(&index).unwrap(), before);
    assert!(index.sends.get(&mut archive, &[18; 32]).unwrap().is_none());
}

#[test]
fn current_maximum_queue_records_and_populated_manifest_fit_provider_bounds() {
    let record = Receipt {
        send_request: [255; 32],
        capsule: [255; 32],
        commitment: [255; 32],
        operation: [255; 32],
        original: [255; 32],
        ordinal: u128::MAX,
        first_unix_ms: Some(u64::MAX),
        state: ReceiptState::Rejected,
    };
    for state in [
        ReceiptState::Unverified,
        ReceiptState::Verified,
        ReceiptState::Rejected,
    ] {
        let bytes = archive::encode(&Receipt {
            state,
            ..record.clone()
        })
        .unwrap();
        assert!(bytes.len() <= index::INDEX_VALUE_LIMIT);
        let parsed: Receipt = archive::decode(&bytes).unwrap();
        assert_eq!(archive::encode(&parsed).unwrap(), bytes);
    }
    let bytes = archive::encode(&Send {
        anchor: [255; 32],
        ordinal: u128::MAX,
        complete: true,
    })
    .unwrap();
    assert!(bytes.len() <= index::INDEX_VALUE_LIMIT);
    let mut w = wallet();
    w.commit(bootstrap()).unwrap();
    let list: KagemushaWalletBlacklistV1 = fixture("KagemushaWalletBlacklistV1");
    let certificates =
        KagemushaWalletCertificateSetV1::new(vec![fixture("KagemushaWalletSignerCertificateV1")])
            .unwrap();
    w.install_blacklist(
        &list.to_canonical_bytes().unwrap(),
        &archive::encode(&certificates).unwrap(),
    )
    .unwrap();
    let (_, mut manifest) = w.manifest().unwrap();
    assert!(
        manifest.blacklist.is_some(),
        "populate even the private fixed blacklist record"
    );
    manifest.indexed = Some(u128::MAX);
    manifest.folded = Some(u128::MAX);
    manifest.activation = Some([255; 32]);
    manifest.ledger_checkpoint = Some([255; 32]);
    manifest.finality_epoch = Some(u64::MAX);
    manifest.ledger_retired = Some([255; 32]);
    manifest.ledger_unload_outcomes = IndexRoot([255; 32]);
    manifest.checkpoint_count = u32::MAX;
    manifest.checkpoint_digest = [255; 32];
    manifest.collection = Some(collection::CollectionIntent {
        sequence: u128::MAX,
        covering_sequence: u128::MAX,
        covering_fold: [255; 32],
        next_checkpoint: u32::MAX,
        phase: u8::MAX,
    });
    // All remaining manifest/tree fields are fixed-width, including empty versus nonempty
    // roots. Set every new root and maximal scalar so schema growth cannot hide in defaults.
    manifest.deferred_archive = DeferredIndex {
        receipts: IndexRoot([255; 32]),
        sends: IndexRoot([255; 32]),
        unverified: IndexRoot([255; 32]),
        ready: IndexRoot([255; 32]),
        next_ordinal: u128::MAX,
        count: u64::MAX,
    };
    let bytes = archive::encode(&manifest).unwrap();
    assert!(
        bytes.len()
            <= crate::kagemusha_wallet_advance_v1::KAGEMUSHA_WALLET_ARCHIVE_MANIFEST_MAX_BYTES_V1,
        "maximally populated Manifest is {} bytes",
        bytes.len()
    );
    let parsed: manifest::Manifest = archive::decode(&bytes).unwrap();
    assert_eq!(archive::encode(&parsed).unwrap(), bytes);
}

#[test]
fn inconsistent_count_and_ready_root_never_describe_an_empty_queue() {
    assert!(
        DeferredIndex {
            count: 1,
            ..DeferredIndex::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        DeferredIndex {
            ready: IndexRoot([1; 32]),
            next_ordinal: 1,
            ..DeferredIndex::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        DeferredIndex {
            receipts: IndexRoot([1; 32]),
            ..DeferredIndex::default()
        }
        .validate()
        .is_err()
    );
    assert_ne!(order_key(1), [255; 32]);
    assert!(order_key(1) < [255; 32]);
    assert!(order_key(u128::MAX) < order_key(1));
}

#[test]
fn exhausted_intake_ordinal_never_wraps_or_reuses_a_removed_identity() {
    let mut index = DeferredIndex::default();
    assert_eq!(index.next_intake_ordinal().unwrap(), 1);
    index.next_ordinal = u128::MAX - 1;
    assert_eq!(index.next_intake_ordinal().unwrap(), u128::MAX);
    index.next_ordinal = u128::MAX;
    assert!(matches!(
        index.next_intake_ordinal(),
        Err(Error::Invalid("deferred intake ordinal exhausted"))
    ));
    assert_eq!(index.next_ordinal, u128::MAX);
}
