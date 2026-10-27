//! Durable queue transitions over explicit mock custody/proofs; no monetary qualification.
use super::*;
use credit_projection::{originals, payer_projection_wallet, receive_projection_wallet};

fn reopen(w: Wallet) -> Wallet {
    Coordinator::new(w.custody, w.archive, w.proofs, w.scheme_id, w.wallet_id).unwrap()
}
fn newer_receipt() -> Vec<u8> {
    let (mut receiver, _, receive) = receive_projection_wallet(false);
    snapshot_test_fold(&mut receiver);
    receiver
        .receive_credit_projection(&receive)
        .unwrap()
        .bytes()[92..]
        .to_vec()
}
fn preparing() -> (Wallet, [u8; 32], Vec<u8>) {
    let (mut w, send, _) = payer_projection_wallet(true, Some(true));
    let original = newer_receipt();
    let (companion, request, _) = originals();
    w.retain_delivery_test_preparing(&NativeIntentV1::retained_delivery_test_originals(
        &fixture("KagemushaWalletPaymentV1"),
        [84; 32],
        archive::encode(&request).unwrap(),
        original.clone(),
        companion.payer_credential.to_canonical_bytes().unwrap(),
        archive::encode(&companion.certificates).unwrap(),
    ))
    .unwrap();
    (w, send, original)
}

#[test]
fn age_wake_hint_reopens_exact_oldest_and_never_hides_missing_originals() {
    let (mut w, send, original) = preparing();
    assert_eq!(
        w.deferred_archive_wake_hint().unwrap(),
        DeferredArchiveWakeHintV1::NoAgeWake
    );
    let retained = w
        .retain_deferred_credited_with_clock(&send, &original, || None)
        .unwrap();
    assert_eq!(
        w.deferred_archive_wake_hint().unwrap(),
        DeferredArchiveWakeHintV1::NoAgeWake,
        "unverified intake has no age candidate"
    );
    w.verify_deferred_credited(&retained).unwrap();
    let mut w = reopen(w);
    let checkpoint = w.custody.checkpoint.clone();
    assert_eq!(
        w.deferred_archive_wake_hint().unwrap(),
        DeferredArchiveWakeHintV1::NoAgeWake,
        "missing first observation cannot be backfilled by a timer"
    );
    let digest =
        crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1(&original);
    w.archive.hide_object(digest);
    assert!(matches!(
        w.deferred_archive_wake_hint(),
        Err(Error::WitnessLost(_))
    ));
    assert!(matches!(
        w.deferred_archive_summary(),
        Err(Error::WitnessLost(_))
    ));
    w.archive.reveal_object(digest);
    assert_eq!(
        w.deferred_archive_wake_hint().unwrap(),
        DeferredArchiveWakeHintV1::NoAgeWake
    );
    assert_eq!(w.custody.checkpoint, checkpoint);
    assert_eq!(w.deferred_archive_summary().unwrap().count, 1);
    assert_eq!(w.custody.signatures, 0);
}

#[test]
fn exact_intake_is_selected_before_verification_and_survives_reopen() {
    let (mut w, send, original) = preparing();
    let retained = w.retain_deferred_credited(&send, &original).unwrap();
    // Inspect the authoritative checkpoint before entering the verifier, not raw put success.
    let checkpoint = w.custody.checkpoint.clone().unwrap();
    assert_eq!(w.next_deferred_credited().unwrap(), Some(retained.clone()));
    assert_eq!(w.deferred_archive_summary().unwrap().count, 0);
    assert_eq!(w.custody.signatures, 0);
    let mut w = reopen(w);
    assert_eq!(w.custody.checkpoint.as_ref(), Some(&checkpoint));
    w.proofs.reject = true; // generic source/proof refusal is not a receipt verdict
    assert!(w.verify_deferred_credited(&retained).is_err());
    assert_eq!(w.next_deferred_credited().unwrap(), Some(retained.clone()));
    assert_eq!(w.custody.checkpoint.as_ref(), Some(&checkpoint));
    w.proofs.reject = false;
    assert_eq!(w.verify_deferred_credited(&retained).unwrap().bytes()[3], 4);
    assert_eq!(w.deferred_archive_summary().unwrap().count, 1);
    assert!(w.next_deferred_credited().unwrap().is_none());
}

#[test]
fn malformed_oldest_is_retained_but_does_not_block_later_valid_receipt_after_restart() {
    let (mut w, send, original) = preparing();
    let bad = w
        .retain_deferred_credited(&send, b"not a canonical Credited")
        .unwrap();
    let good = w.retain_deferred_credited(&send, &original).unwrap();
    let mut w = reopen(w);
    assert_eq!(w.next_deferred_credited().unwrap(), Some(bad.clone()));
    assert!(matches!(
        w.verify_deferred_credited(&bad),
        Err(Error::ReceiptRejected(_))
    ));
    let selected = w.custody.checkpoint.clone();
    let mut w = reopen(w);
    assert_eq!(w.next_deferred_credited().unwrap(), Some(good.clone()));
    assert_eq!(
        w.retain_deferred_credited(&send, b"not a canonical Credited")
            .unwrap(),
        bad
    );
    assert_eq!(
        w.custody.checkpoint, selected,
        "rejected exact original stays selected"
    );
    assert!(matches!(
        w.verify_deferred_credited(&bad),
        Err(Error::ReceiptRejected(_))
    ));
    assert_eq!(
        w.custody.checkpoint, selected,
        "terminal retry has no mutation"
    );
    w.verify_deferred_credited(&good).unwrap();
    assert_eq!(
        w.deferred_archive_summary()
            .unwrap()
            .oldest
            .unwrap()
            .original(),
        original
    );
}

#[test]
fn duplicate_retry_never_resamples_missing_age_or_needs_fresh_capacity() {
    let (mut w, send, original) = preparing();
    let retained = w
        .retain_deferred_credited_with_clock(&send, &original, || None)
        .unwrap();
    let selected = w.custody.checkpoint.clone();
    let mut w = reopen(w);
    w.custody.fail_resources = true;
    assert_eq!(
        w.retain_deferred_credited_with_clock(&send, &original, || panic!(
            "duplicate cannot backfill first time"
        ))
        .unwrap(),
        retained
    );
    assert_eq!(w.custody.checkpoint, selected);
    w.custody.fail_resources = false;
    w.verify_deferred_credited(&retained).unwrap();
    let summary = w.deferred_archive_summary().unwrap();
    assert_eq!(summary.oldest_age, DeferredArchiveAgeV1::Unavailable);
    assert_eq!(summary.oldest.unwrap().ordinal(), 1);
    let selected = w.custody.checkpoint.clone();
    w.custody.fail_resources = true;
    w.verify_deferred_credited(&retained).unwrap();
    assert_eq!(
        w.custody.checkpoint, selected,
        "verified retry is read-only"
    );
}

#[test]
fn intake_uncertain_publication_reconciles_selected_bytes_without_resetting_time() {
    for after in [false, true] {
        let (mut w, send, original) = preparing();
        w.custody.fail_publication = Some(after);
        assert!(
            w.retain_deferred_credited_with_clock(&send, &original, || Some(u64::MAX))
                .is_err()
        );
        let mut w = reopen(w);
        assert_eq!(w.next_deferred_credited().unwrap().is_some(), after);
        let selected = w.custody.checkpoint.clone();
        let receipt = w
            .retain_deferred_credited_with_clock(&send, &original, || {
                assert!(
                    !after,
                    "selected uncertain publication keeps first observation"
                );
                Some(u64::MAX)
            })
            .unwrap();
        if after {
            assert_eq!(w.custody.checkpoint, selected);
        }
        w.verify_deferred_credited(&receipt).unwrap();
        let summary = w.deferred_archive_summary().unwrap();
        assert_eq!(summary.count, 1);
        assert_eq!(summary.oldest.unwrap().ordinal(), 1);
        assert_eq!(summary.oldest_age, DeferredArchiveAgeV1::Regression);
        assert_eq!(
            w.deferred_archive_wake_hint().unwrap(),
            DeferredArchiveWakeHintV1::RecheckAfter(Duration::from_secs(900))
        );
    }
}

#[test]
fn missing_or_corrupt_selected_original_is_an_error_and_remains_discoverable_after_repair() {
    let (mut w, send, original) = preparing();
    let receipt = w.retain_deferred_credited(&send, &original).unwrap();
    let digest =
        crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1(&original);
    let selected = w.custody.checkpoint.clone();
    w.archive.hide_object(digest);
    assert!(w.next_deferred_credited().is_err());
    assert!(w.verify_deferred_credited(&receipt).is_err());
    assert_eq!(w.custody.checkpoint, selected);
    w.archive.reveal_object(digest);
    let key = ArchiveKey::Object(digest);
    let held = w
        .archive
        .records
        .lock()
        .unwrap()
        .insert(key, b"corrupt selected object".to_vec())
        .unwrap();
    assert!(w.next_deferred_credited().is_err());
    assert_eq!(w.custody.checkpoint, selected);
    w.archive.records.lock().unwrap().insert(key, held);
    assert_eq!(w.next_deferred_credited().unwrap(), Some(receipt));
}

#[test]
fn newer_receipt_keeps_one_send_and_first_anchor_after_send_collection_and_archive_complete() {
    let (mut w, send, anchor) = payer_projection_wallet(true, Some(true));
    let first = w
        .retain_deferred_credited_with_clock(&send, &anchor, || None)
        .unwrap();
    w.verify_deferred_credited(&first).unwrap();
    let entry = w.deferred_archive_summary().unwrap().oldest.unwrap();
    assert_eq!(entry.original(), anchor);
    let newer = newer_receipt();
    let second = w
        .retain_deferred_credited_with_clock(&send, &newer, || Some(1))
        .unwrap();
    assert_eq!(w.verify_deferred_credited(&second).unwrap().bytes()[2], 2);
    let summary = w.deferred_archive_summary().unwrap();
    assert_eq!(summary.count, 1);
    assert_eq!(summary.oldest, Some(entry.clone()));
    assert_eq!(summary.oldest_age, DeferredArchiveAgeV1::Unavailable);
    // The fixture's Send is already tombstoned. Completion needs no new capacity gate.
    w.custody.fail_resources = true;
    assert!(matches!(
        w.reconcile_deferred_archive(&entry).unwrap(),
        RequestStatusV1::Outcome(Completion::Complete(_))
    ));
    assert_eq!(w.deferred_archive_summary().unwrap().count, 0);
    let selected = w.custody.checkpoint.clone();
    let mut w = reopen(w);
    assert!(matches!(
        w.reconcile_deferred_archive(&entry).unwrap(),
        RequestStatusV1::Outcome(Completion::Complete(_))
    ));
    w.verify_deferred_credited(&second).unwrap();
    assert_eq!(w.custody.checkpoint, selected);
    assert_eq!(w.custody.signatures, 0);
}

#[test]
fn preparing_and_missing_exact_archive_do_not_dequeue() {
    let (mut w, send, original) = preparing();
    let receipt = w.retain_deferred_credited(&send, &original).unwrap();
    w.verify_deferred_credited(&receipt).unwrap();
    let entry = w.deferred_archive_summary().unwrap().oldest.unwrap();
    let selected = w.custody.checkpoint.clone();
    assert!(matches!(
        w.reconcile_deferred_archive(&entry).unwrap(),
        RequestStatusV1::Preparing
    ));
    assert_eq!(w.custody.checkpoint, selected);
    assert_eq!(w.deferred_archive_summary().unwrap().count, 1);
    let wrong_wallet = payer_projection_wallet(true, Some(true)).0;
    let mut other = wrong_wallet;
    assert!(
        other.reconcile_deferred_archive(&entry).is_err(),
        "caller entry is no authority"
    );
    assert_eq!(other.deferred_archive_summary().unwrap().count, 0);
}

#[test]
fn completed_dequeue_uncertain_publication_retries_exact_anchor() {
    for after in [false, true] {
        let (mut w, send, original) = payer_projection_wallet(false, Some(false));
        let receipt = w.retain_deferred_credited(&send, &original).unwrap();
        w.verify_deferred_credited(&receipt).unwrap();
        let entry = w.deferred_archive_summary().unwrap().oldest.unwrap();
        w.custody.fail_publication = Some(after);
        assert!(w.reconcile_deferred_archive(&entry).is_err());
        let mut w = reopen(w);
        assert_eq!(
            w.deferred_archive_summary().unwrap().count,
            u64::from(!after)
        );
        assert!(matches!(
            w.reconcile_deferred_archive(&entry).unwrap(),
            RequestStatusV1::Outcome(Completion::Complete(_))
        ));
        assert_eq!(w.deferred_archive_summary().unwrap().count, 0);
    }
}

#[test]
fn pending_head_refuses_intake_before_any_queue_publication_or_capacity_query() {
    let mut w = wallet();
    let boot = bootstrap();
    w.commit(boot.clone()).unwrap();
    w.custody.pause = true;
    let next = frozen(
        Some(&boot),
        KagemushaWalletEffectV1::Load {
            receipt_digest: field(73),
            load_ordinal: 0,
            amount: 1,
            online_charge: 0,
        },
    );
    assert!(matches!(w.commit(next).unwrap(), Completion::Pending));
    let selected = w.custody.checkpoint.clone();
    let queries = w.custody.resource_queries;
    assert!(matches!(
        w.retain_deferred_credited(&[1; 32], b"held"),
        Err(Error::Pending)
    ));
    assert_eq!(w.custody.checkpoint, selected);
    assert_eq!(w.custody.resource_queries, queries);
}

#[test]
fn intake_metadata_survives_rebased_fold_publication_without_cancelling_work() {
    use std::ops::ControlFlow;
    // The projection-only fixture omits sequence zero and cannot start a fold. Use the
    // production commit/index path with its retained predecessor and contiguous Send tail.
    let (mut w, sent, request, original, _, _) = lifecycle::fresh_delivery_receipt();
    let send = request.request_id;
    let sequence = sent.capsule.body().statement.sequence;
    w.scheduler().set_activity(true, false);
    let ControlFlow::Continue(job) = w.fold_prepare().unwrap() else {
        panic!("fold job");
    };
    let proved = job.prove().unwrap();
    let receipt = w.retain_deferred_credited(&send, &original).unwrap();
    let before = archive::encode(&w.manifest().unwrap().1.deferred_archive).unwrap();
    assert_eq!(
        w.fold_publish(proved).unwrap().status,
        FoldStatus::Checkpoint {
            sequence,
            ordinal: 0
        }
    );
    let after = archive::encode(&w.manifest().unwrap().1.deferred_archive).unwrap();
    assert_eq!(before, after);
    assert_eq!(w.next_deferred_credited().unwrap(), Some(receipt));
}

#[test]
fn canonical_model_invalid_evidence_is_rejected_without_hiding_later_valid_evidence() {
    for foreign_scheme in [false, true] {
        let (mut w, send, anchor) = payer_projection_wallet(true, Some(true));
        let first = w.retain_deferred_credited(&send, &anchor).unwrap();
        w.verify_deferred_credited(&first).unwrap();
        let good_bytes = newer_receipt();
        let mut bad: KagemushaWalletCreditedV1 = archive::decode(&good_bytes).unwrap();
        if foreign_scheme {
            bad.scheme_id = field(99);
        } else {
            let KagemushaWalletCreditedEvidenceV1::Status { status } = &mut bad.evidence else {
                panic!("Status");
            };
            status.opening.credit_id = field(99);
        }
        let bad = w
            .retain_deferred_credited(&send, &archive::encode(&bad).unwrap())
            .unwrap();
        let good = w.retain_deferred_credited(&send, &good_bytes).unwrap();
        let mut w = reopen(w);
        assert_eq!(w.next_deferred_credited().unwrap(), Some(bad.clone()));
        assert!(matches!(
            w.verify_deferred_credited(&bad),
            Err(Error::ReceiptRejected(_))
        ));
        assert_eq!(w.next_deferred_credited().unwrap(), Some(good.clone()));
        w.verify_deferred_credited(&good).unwrap();
        assert_eq!(
            w.deferred_archive_summary()
                .unwrap()
                .oldest
                .unwrap()
                .original(),
            anchor
        );
    }
}

#[test]
fn open_request_identity_survives_exact_completed_anchor_and_collected_send_retries() {
    let (mut w, send, anchor) = payer_projection_wallet(true, Some(true));
    let open = vectors()["envelopes"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|row| {
            let envelope: KagemushaWalletEnvelopeV1 =
                archive::decode(&hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap())
                    .unwrap();
            match envelope.message {
                KagemushaWalletMessageV1::Request { request } => Some(request),
                _ => None,
            }
        })
        .unwrap();
    assert!(open.body.is_open(), "production receiver-first intent");
    let payment: KagemushaWalletPaymentV1 = fixture("KagemushaWalletPaymentV1");
    w.retain_delivery_test_mapping(
        &NativeIntentV1::user(OperationRequestV1 {
            request_id: send,
            action: OperationActionV1::Send {
                request: archive::encode(&open).unwrap(),
            },
        }),
        [84; 32],
        payment.send.receipt.capsule_commitment,
        payment.send.statement.operation_id(&w.wallet_id).unwrap(),
    )
    .unwrap();
    let retained = w.retain_deferred_credited(&send, &anchor).unwrap();
    w.verify_deferred_credited(&retained).unwrap();
    let entry = w.deferred_archive_summary().unwrap().oldest.unwrap();
    let mut w = reopen(w);
    assert_eq!(
        w.retain_deferred_credited(&send, &anchor).unwrap(),
        retained
    );
    assert!(matches!(
        w.reconcile_deferred_archive(&entry).unwrap(),
        RequestStatusV1::Outcome(Completion::Complete(_))
    ));
    let newer = newer_receipt();
    let latest = w.retain_deferred_credited(&send, &newer).unwrap();
    w.verify_deferred_credited(&latest).unwrap();
    assert_eq!(w.deferred_archive_summary().unwrap().count, 0);
    assert_eq!(w.custody.signatures, 0);
    // Mutating a receiver term cannot be disguised as reconstruction of payer completion.
    let mut foreign = open;
    foreign.body.amount += 1;
    w.retain_delivery_test_mapping(
        &NativeIntentV1::user(OperationRequestV1 {
            request_id: send,
            action: OperationActionV1::Send {
                request: archive::encode(&foreign).unwrap(),
            },
        }),
        [84; 32],
        payment.send.receipt.capsule_commitment,
        payment.send.statement.operation_id(&w.wallet_id).unwrap(),
    )
    .unwrap();
    assert!(w.verify_deferred_credited(&latest).is_err());
}

#[test]
fn archived_or_lost_exact_archive_output_never_counts_as_complete() {
    for archived in [false, true] {
        let (mut w, send, original) = payer_projection_wallet(false, Some(false));
        let receipt = w.retain_deferred_credited(&send, &original).unwrap();
        w.verify_deferred_credited(&receipt).unwrap();
        let entry = w.deferred_archive_summary().unwrap().oldest.unwrap();
        let operation = w.custody.status.marker().unwrap().head().unwrap().1;
        if archived {
            // Explicit hostile provider probe: production collection does not prune Archive.
            w.custody
                .prune_completion(operation, KagemushaWalletOperationKindV1::ArchiveSent as u8)
                .unwrap();
        } else {
            w.custody.retained.remove(&operation).unwrap();
        }
        let selected = w.custody.checkpoint.clone();
        let outcome = w.reconcile_deferred_archive(&entry);
        assert!(!matches!(
            outcome,
            Ok(RequestStatusV1::Outcome(Completion::Complete(_)))
        ));
        assert_eq!(w.custody.checkpoint, selected);
        assert_eq!(w.deferred_archive_summary().unwrap().count, 1);
    }
}

#[test]
fn fresh_open_send_retains_first_intent_before_verified_membership_and_recovers_interruption() {
    for (boundary, after) in [(1, false), (1, true), (2, false), (2, true)] {
        let (mut w, sent, request, original, _, _) = lifecycle::fresh_delivery_receipt();
        let OperationActionV1::Send { request: completed } = &request.action else {
            panic!("Send");
        };
        let mut received: KagemushaWalletRequestV1 = archive::decode(completed).unwrap();
        received.body = received.body.open();
        assert!(received.body.is_open());
        w.retain_delivery_test_mapping(
            &NativeIntentV1::user(OperationRequestV1 {
                request_id: request.request_id,
                action: OperationActionV1::Send {
                    request: archive::encode(&received).unwrap(),
                },
            }),
            sent.capsule.custody_digest(),
            sent.capsule.receipt_commitment(),
            sent.capsule.body().operation_id,
        )
        .unwrap();
        assert!(matches!(
            w.credited_status_for_send(&request.request_id, &original)
                .unwrap(),
            RequestStatusV1::Unknown
        ));
        let mut bad: KagemushaWalletCreditedV1 = archive::decode(&original).unwrap();
        let KagemushaWalletCreditedEvidenceV1::Receive { package } = &mut bad.evidence else {
            panic!("Receive");
        };
        let KagemushaWalletEffectV1::Receive { credit_id, .. } = &mut package.statement.effect
        else {
            panic!("Receive effect");
        };
        *credit_id = field(99);
        bad.validate().unwrap(); // reach source binding, not merely malformed model rejection
        let invalid = w
            .retain_deferred_credited(&request.request_id, &archive::encode(&bad).unwrap())
            .unwrap();
        let good = w
            .retain_deferred_credited(&request.request_id, &original)
            .unwrap();
        let mut w = reopen(w);
        assert_eq!(w.next_deferred_credited().unwrap(), Some(invalid.clone()));
        assert!(matches!(
            w.verify_deferred_credited(&invalid),
            Err(Error::ReceiptRejected("deferred Credited Send binding"))
        ));
        assert_eq!(w.next_deferred_credited().unwrap(), Some(good.clone()));
        let signatures = w.custody.signatures;
        let preparations = w.proofs.preparations.load(Ordering::SeqCst);
        // Inject before/after each of request-only intent and ready membership publication.
        // Reopen must adopt whichever exact checkpoint was selected, without a second anchor.
        w.custody.fail_publication_at = Some((w.custody.publication_calls + boundary, after));
        assert!(w.verify_deferred_credited(&good).is_err());
        let intent_selected = boundary == 2 || after;
        assert_eq!(
            w.credited_status_for_send(&request.request_id, &original)
                .unwrap(),
            if intent_selected {
                RequestStatusV1::Preparing
            } else {
                RequestStatusV1::Unknown
            }
        );
        let membership_selected = boundary == 2 && after;
        assert_eq!(
            w.deferred_archive_summary().unwrap().count,
            u64::from(membership_selected)
        );
        assert_eq!(
            w.next_deferred_credited().unwrap(),
            if membership_selected {
                None
            } else {
                Some(good.clone())
            }
        );
        assert_eq!(w.custody.signatures, signatures);
        assert_eq!(
            w.proofs.preparations.load(Ordering::SeqCst),
            preparations,
            "no monetary plan/proof"
        );
        let mut w = reopen(w);
        w.verify_deferred_credited(&good).unwrap();
        let entry = w.deferred_archive_summary().unwrap().oldest.unwrap();
        assert_eq!(entry.original(), original);
        assert_eq!(entry.ordinal(), 2, "rejected ordinal is not reused");
        assert!(matches!(
            w.reconcile_deferred_archive(&entry).unwrap(),
            RequestStatusV1::Preparing
        ));
        assert_eq!(w.custody.signatures, signatures);
    }
}

#[test]
fn only_explicit_completed_receipt_verdict_advances_the_unverified_fifo() {
    for rejected in [false, true] {
        let (mut w, send, original) = preparing();
        let receipt = w.retain_deferred_credited(&send, &original).unwrap();
        w.proofs.deferred_receipt_refusal = Some(rejected);
        let outcome = w.verify_deferred_credited(&receipt);
        if rejected {
            assert!(matches!(outcome, Err(Error::ReceiptRejected(_))));
            assert!(w.next_deferred_credited().unwrap().is_none());
        } else {
            assert!(matches!(outcome, Err(Error::NativeVerification(_))));
            assert_eq!(w.next_deferred_credited().unwrap(), Some(receipt.clone()));
            w.proofs.deferred_receipt_refusal = None;
            w.verify_deferred_credited(&receipt).unwrap();
            assert_eq!(w.deferred_archive_summary().unwrap().count, 1);
        }
    }
}

const FAULT_TEST_MALFORMED_CREDITED: &[u8] = b"fault-test malformed exact Credited";

fn unverified_fault_pair() -> (
    Wallet,
    [u8; 32],
    Vec<u8>,
    DeferredArchiveReceiptV1,
    DeferredArchiveReceiptV1,
) {
    let (mut w, send, original) = preparing();
    let bad = w
        .retain_deferred_credited_with_clock(&send, FAULT_TEST_MALFORMED_CREDITED, || None)
        .unwrap();
    let good = w
        .retain_deferred_credited_with_clock(&send, &original, || None)
        .unwrap();
    (w, send, original, bad, good)
}

#[test]
fn rejected_checkpoint_cuts_recover_selected_verdict_and_preserve_exact_original() {
    for after in [false, true] {
        let (mut w, send, original, bad, good) = unverified_fault_pair();
        let selected = w.custody.checkpoint.clone();
        w.custody.fail_publication = Some(after);
        assert!(matches!(
            w.verify_deferred_credited(&bad),
            Err(Error::Provider(ProviderError::Invalid { .. }))
        ));
        assert!(
            w.custody.fail_publication.is_none(),
            "checkpoint cut reached"
        );
        assert_eq!(w.custody.checkpoint == selected, !after);
        let mut w = reopen(w);
        assert_eq!(
            w.next_deferred_credited().unwrap(),
            Some(if after { good.clone() } else { bad.clone() })
        );
        let selected = w.custody.checkpoint.clone();
        assert_eq!(
            w.retain_deferred_credited_with_clock(&send, FAULT_TEST_MALFORMED_CREDITED, || {
                panic!("selected intake cannot resample age")
            })
            .unwrap(),
            bad
        );
        assert_eq!(w.custody.checkpoint, selected);
        assert!(matches!(
            w.verify_deferred_credited(&bad),
            Err(Error::ReceiptRejected(_))
        ));
        if after {
            assert_eq!(
                w.custody.checkpoint, selected,
                "selected verdict is terminal"
            );
        }
        let digest = crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1(
            FAULT_TEST_MALFORMED_CREDITED,
        );
        assert_eq!(
            w.archive
                .read_object(&digest, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)
                .unwrap(),
            FAULT_TEST_MALFORMED_CREDITED
        );
        assert_eq!(w.next_deferred_credited().unwrap(), Some(good.clone()));
        w.verify_deferred_credited(&good).unwrap();
        let summary = w.deferred_archive_summary().unwrap();
        assert_eq!(summary.count, 1);
        assert_eq!(summary.oldest_age, DeferredArchiveAgeV1::Unavailable);
        let entry = summary.oldest.unwrap();
        assert_eq!(entry.original(), original);
        assert_eq!(entry.ordinal(), 2, "rejected intake ordinal stays consumed");
        assert_eq!(w.custody.signatures, 0);
    }
}

#[test]
fn each_intake_object_write_cut_leaves_no_selected_receipt_and_retries_after_reopen() {
    let (mut pilot, send, original) = preparing();
    let before = pilot.archive.records.lock().unwrap().clone();
    pilot
        .retain_deferred_credited_with_clock(&send, &original, || None)
        .unwrap();
    // Discover the exact deterministic writes, including every index node, from a successful
    // pilot. This is an immediate-memory put fault, not a filesystem power-loss simulation.
    let keys: Vec<_> = pilot
        .archive
        .records
        .lock()
        .unwrap()
        .keys()
        .filter(|key| !before.contains_key(*key))
        .copied()
        .collect();
    let original_key = ArchiveKey::Object(
        crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1(&original),
    );
    assert!(
        keys.contains(&original_key),
        "exact original write is covered"
    );
    assert!(keys.len() >= 3, "original plus both intake index roots");
    assert!(keys.iter().all(|key| matches!(key, ArchiveKey::Object(_))));
    for key in keys {
        for after in [false, true] {
            let (mut w, send, original) = preparing();
            let selected = w.custody.checkpoint.clone();
            let publications = w.custody.publication_calls;
            *w.archive.fail_put.lock().unwrap() = Some((key, after));
            assert!(matches!(
                w.retain_deferred_credited_with_clock(&send, &original, || None),
                Err(Error::Storage(_))
            ));
            assert!(
                w.archive.fail_put.lock().unwrap().is_none(),
                "exact put cut reached"
            );
            assert_eq!(w.archive.records.lock().unwrap().contains_key(&key), after);
            assert_eq!(w.custody.checkpoint, selected);
            assert_eq!(w.custody.publication_calls, publications);
            let mut w = reopen(w);
            assert!(w.next_deferred_credited().unwrap().is_none());
            assert_eq!(w.deferred_archive_summary().unwrap().count, 0);
            let receipt = w
                .retain_deferred_credited_with_clock(&send, &original, || None)
                .unwrap();
            let mut w = reopen(w);
            assert_eq!(w.next_deferred_credited().unwrap(), Some(receipt.clone()));
            w.verify_deferred_credited(&receipt).unwrap();
            let summary = w.deferred_archive_summary().unwrap();
            assert_eq!(summary.count, 1);
            assert_eq!(summary.oldest_age, DeferredArchiveAgeV1::Unavailable);
            let entry = summary.oldest.unwrap();
            assert_eq!(entry.original(), original);
            assert_eq!(entry.ordinal(), 1, "unselected writes consume no ordinal");
            assert_eq!(w.custody.signatures, 0);
        }
    }
}

#[test]
fn each_rejection_index_write_cut_keeps_oldest_discoverable_until_selected_retry() {
    let (mut pilot, _, _, bad, _) = unverified_fault_pair();
    let before = pilot.archive.records.lock().unwrap().clone();
    assert!(matches!(
        pilot.verify_deferred_credited(&bad),
        Err(Error::ReceiptRejected(_))
    ));
    let keys: Vec<_> = pilot
        .archive
        .records
        .lock()
        .unwrap()
        .keys()
        .filter(|key| !before.contains_key(*key))
        .copied()
        .collect();
    assert!(
        !keys.is_empty(),
        "rejected receipt requires new index nodes"
    );
    assert!(keys.iter().all(|key| matches!(key, ArchiveKey::Object(_))));
    for key in keys {
        for after in [false, true] {
            let (mut w, send, original, bad, good) = unverified_fault_pair();
            let selected = w.custody.checkpoint.clone();
            let publications = w.custody.publication_calls;
            *w.archive.fail_put.lock().unwrap() = Some((key, after));
            assert!(matches!(
                w.verify_deferred_credited(&bad),
                Err(Error::Storage(_))
            ));
            assert!(
                w.archive.fail_put.lock().unwrap().is_none(),
                "exact put cut reached"
            );
            assert_eq!(w.archive.records.lock().unwrap().contains_key(&key), after);
            assert_eq!(w.custody.checkpoint, selected);
            assert_eq!(w.custody.publication_calls, publications);
            let mut w = reopen(w);
            assert_eq!(w.next_deferred_credited().unwrap(), Some(bad.clone()));
            assert_eq!(
                w.retain_deferred_credited_with_clock(&send, FAULT_TEST_MALFORMED_CREDITED, || {
                    panic!("failed rejection retains original age")
                })
                .unwrap(),
                bad
            );
            assert_eq!(w.custody.checkpoint, selected);
            assert!(matches!(
                w.verify_deferred_credited(&bad),
                Err(Error::ReceiptRejected(_))
            ));
            let mut w = reopen(w);
            assert_eq!(w.next_deferred_credited().unwrap(), Some(good.clone()));
            w.verify_deferred_credited(&good).unwrap();
            let summary = w.deferred_archive_summary().unwrap();
            assert_eq!(summary.count, 1);
            assert_eq!(summary.oldest_age, DeferredArchiveAgeV1::Unavailable);
            let entry = summary.oldest.unwrap();
            assert_eq!(entry.original(), original);
            assert_eq!(entry.ordinal(), 2);
            assert_eq!(w.custody.signatures, 0);
        }
    }
}

#[test]
fn each_verified_membership_write_cut_retains_unverified_intake_for_exact_retry() {
    let (mut pilot, send, original) = preparing();
    let receipt = pilot
        .retain_deferred_credited_with_clock(&send, &original, || None)
        .unwrap();
    let before = pilot.archive.records.lock().unwrap().clone();
    pilot.verify_deferred_credited(&receipt).unwrap();
    let keys: Vec<_> = pilot
        .archive
        .records
        .lock()
        .unwrap()
        .keys()
        .filter(|key| !before.contains_key(*key))
        .copied()
        .collect();
    assert!(
        keys.len() >= 3,
        "Send, ready and verified receipt index nodes"
    );
    assert!(keys.iter().all(|key| matches!(key, ArchiveKey::Object(_))));
    for key in keys {
        for after in [false, true] {
            let (mut w, send, original) = preparing();
            let receipt = w
                .retain_deferred_credited_with_clock(&send, &original, || None)
                .unwrap();
            let selected = w.custody.checkpoint.clone();
            let publications = w.custody.publication_calls;
            *w.archive.fail_put.lock().unwrap() = Some((key, after));
            assert!(matches!(
                w.verify_deferred_credited(&receipt),
                Err(Error::Storage(_))
            ));
            assert!(
                w.archive.fail_put.lock().unwrap().is_none(),
                "exact put cut reached"
            );
            assert_eq!(w.archive.records.lock().unwrap().contains_key(&key), after);
            assert_eq!(w.custody.checkpoint, selected);
            assert_eq!(w.custody.publication_calls, publications);
            let mut w = reopen(w);
            assert_eq!(w.next_deferred_credited().unwrap(), Some(receipt.clone()));
            assert_eq!(w.deferred_archive_summary().unwrap().count, 0);
            assert_eq!(
                w.retain_deferred_credited_with_clock(&send, &original, || {
                    panic!("failed membership keeps selected intake age")
                })
                .unwrap(),
                receipt
            );
            w.verify_deferred_credited(&receipt).unwrap();
            let summary = w.deferred_archive_summary().unwrap();
            assert_eq!(summary.count, 1);
            assert_eq!(summary.oldest_age, DeferredArchiveAgeV1::Unavailable);
            let entry = summary.oldest.unwrap();
            assert_eq!(entry.original(), original);
            assert_eq!(entry.ordinal(), 1);
            assert!(w.next_deferred_credited().unwrap().is_none());
            assert_eq!(w.custody.signatures, 0);
        }
    }
}

#[test]
fn each_completed_dequeue_write_cut_keeps_exact_anchor_until_capacity_free_retry() {
    let (mut pilot, send, original) = payer_projection_wallet(false, Some(false));
    let receipt = pilot
        .retain_deferred_credited_with_clock(&send, &original, || None)
        .unwrap();
    pilot.verify_deferred_credited(&receipt).unwrap();
    let entry = pilot.deferred_archive_summary().unwrap().oldest.unwrap();
    let before = pilot.archive.records.lock().unwrap().clone();
    assert!(matches!(
        pilot.reconcile_deferred_archive(&entry).unwrap(),
        RequestStatusV1::Outcome(Completion::Complete(_))
    ));
    let keys: Vec<_> = pilot
        .archive
        .records
        .lock()
        .unwrap()
        .keys()
        .filter(|key| !before.contains_key(*key))
        .copied()
        .collect();
    assert!(
        !keys.is_empty(),
        "exact completion updates retained Send index"
    );
    assert!(keys.iter().all(|key| matches!(key, ArchiveKey::Object(_))));
    for key in keys {
        for after in [false, true] {
            let (mut w, send, original) = payer_projection_wallet(false, Some(false));
            let receipt = w
                .retain_deferred_credited_with_clock(&send, &original, || None)
                .unwrap();
            w.verify_deferred_credited(&receipt).unwrap();
            let entry = w.deferred_archive_summary().unwrap().oldest.unwrap();
            let selected = w.custody.checkpoint.clone();
            let publications = w.custody.publication_calls;
            *w.archive.fail_put.lock().unwrap() = Some((key, after));
            assert!(matches!(
                w.reconcile_deferred_archive(&entry),
                Err(Error::Storage(_))
            ));
            assert!(
                w.archive.fail_put.lock().unwrap().is_none(),
                "exact put cut reached"
            );
            assert_eq!(w.archive.records.lock().unwrap().contains_key(&key), after);
            assert_eq!(w.custody.checkpoint, selected);
            assert_eq!(w.custody.publication_calls, publications);
            let mut w = reopen(w);
            let summary = w.deferred_archive_summary().unwrap();
            assert_eq!(summary.count, 1);
            assert_eq!(summary.oldest, Some(entry.clone()));
            w.custody.fail_resources = true;
            assert!(matches!(
                w.reconcile_deferred_archive(&entry).unwrap(),
                RequestStatusV1::Outcome(Completion::Complete(_))
            ));
            assert_eq!(w.deferred_archive_summary().unwrap().count, 0);
            let selected = w.custody.checkpoint.clone();
            let mut w = reopen(w);
            assert!(matches!(
                w.reconcile_deferred_archive(&entry).unwrap(),
                RequestStatusV1::Outcome(Completion::Complete(_))
            ));
            assert_eq!(
                w.custody.checkpoint, selected,
                "completed replay is read-only"
            );
            assert_eq!(w.custody.signatures, 0);
        }
    }
}
