//! Actual Coordinator projection over explicit host custody/proof fixtures, not installed
//! financial or hardware qualification. Original envelopes retain canonical signatures.

use super::*;

fn receive_projection_wallet(burn: bool) -> (Wallet, FrozenTransition, [u8; 32]) {
    let payment: KagemushaWalletPaymentV1 = fixture("KagemushaWalletPaymentV1");
    let mut wallet = wallet();
    let boot = bootstrap();
    wallet.commit(boot.clone()).unwrap();
    snapshot_test_fold(&mut wallet);
    wallet.proofs.burn = burn;
    let mut receive = frozen(
        Some(&boot),
        KagemushaWalletEffectV1::Receive {
            credit_id: payment.request.body.credit_id(),
            payer_wallet_id: payment.request.body.payer_wallet_id,
            amount: payment.request.body.amount,
        },
    );
    receive.capsule.successor_state.core.balance = payment.request.body.amount + 20;
    rebind_frozen_successor(&mut receive);
    let mut payer = None;
    let mut request = None;
    for row in vectors()["envelopes"].as_array().unwrap() {
        let envelope: KagemushaWalletEnvelopeV1 =
            archive::decode(&hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap()).unwrap();
        match envelope.message {
            KagemushaWalletMessageV1::Offer { offer } => payer = Some(offer),
            KagemushaWalletMessageV1::Request { request: original } => request = Some(original),
            _ => {}
        }
    }
    let payer = payer.unwrap();
    let request = request.unwrap();
    for input in &mut receive.capsule.retained_inputs {
        input.bytes = match input.role {
            KagemushaWalletRetainedInputRoleV1::Request => archive::encode(&request).unwrap(),
            KagemushaWalletRetainedInputRoleV1::Payment => payment.to_canonical_bytes().unwrap(),
            KagemushaWalletRetainedInputRoleV1::Credential => {
                payer.payer_credential.to_canonical_bytes().unwrap()
            }
            KagemushaWalletRetainedInputRoleV1::CertificateSet => {
                archive::encode(&payer.certificates).unwrap()
            }
            _ => input.bytes.clone(),
        };
    }
    wallet.commit(receive.clone()).unwrap();
    let id = [0x97; 32];
    wallet
        .retain_collected_receive_test_request(
            OperationRequestV1 {
                request_id: id,
                action: OperationActionV1::Receive {
                    payment: payment.to_canonical_bytes().unwrap(),
                    payer_credential: payer.payer_credential.to_canonical_bytes().unwrap(),
                    certificates: archive::encode(&payer.certificates).unwrap(),
                },
            },
            &receive,
        )
        .unwrap();
    (wallet, receive, id)
}

#[test]
fn receiver_projection_updates_at_covering_fold_before_collection_and_new_unfolded_head() {
    for burn in [false, true] {
        let (mut wallet, receive, id) = receive_projection_wallet(burn);
        let first = wallet.receive_credit_projection(&id).unwrap().bytes();
        assert_eq!(&first[..8], &[1, 0, 1, 0, 0, 0, 0, 0]);
        assert!(!first[92..].is_empty());
        snapshot_test_fold(&mut wallet);
        let folded = wallet.receive_credit_projection(&id).unwrap().bytes();
        assert_eq!(folded[2], if burn { 3 } else { 2 });
        assert_ne!(&first[92..], &folded[92..]);
        assert!(
            matches!(
                wallet.retry_request(&id).unwrap(),
                RequestStatusV1::Outcome(Completion::Complete(_))
            ),
            "Projection changes before historical Receive collection"
        );
        let mut next = frozen(
            Some(&receive),
            KagemushaWalletEffectV1::Load {
                receipt_digest: field(98),
                load_ordinal: 0,
                amount: 1,
                online_charge: 0,
            },
        );
        next.capsule.successor_state.core.balance = receive.capsule.successor_state.core.balance;
        rebind_frozen_successor(&mut next);
        wallet.commit(next).unwrap();
        assert!(wallet.snapshot().unwrap().folded_balance.is_none());
        assert_eq!(
            wallet.receive_credit_projection(&id).unwrap().bytes()[2],
            folded[2]
        );
    }
}

#[test]
fn receiver_projection_rejects_foreign_mapping_and_unavailable_custody() {
    let (mut wallet, _, id) = receive_projection_wallet(false);
    assert!(wallet.receive_credit_projection(&[0x98; 32]).is_err());
    wallet.custody.unavailable = true;
    assert!(wallet.receive_credit_projection(&id).is_err());
}

fn originals() -> (
    KagemushaWalletOfferV1,
    KagemushaWalletRequestV1,
    KagemushaWalletCreditedV1,
) {
    let mut offer = None;
    let mut request = None;
    for row in vectors()["envelopes"].as_array().unwrap() {
        let envelope: KagemushaWalletEnvelopeV1 =
            archive::decode(&hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap()).unwrap();
        match envelope.message {
            KagemushaWalletMessageV1::Offer { offer: value } => offer = Some(value),
            KagemushaWalletMessageV1::Request { request: value } => request = Some(value),
            _ => {}
        }
    }
    let complete: KagemushaWalletCompletionRecordV1 = fixture("KagemushaWalletCompletionRecordV1");
    (
        offer.unwrap(),
        request.unwrap(),
        KagemushaWalletCreditedV1::from_receive(archive::decode(&complete.output).unwrap())
            .unwrap(),
    )
}

// Explicit selected-head/verified-fold probes allow independent core and lineage pending roots.
// The prior Send is a permanent tombstone; no historical Send capsule/source exists. Receipt
// assembly still uses the real payer key and exact retained originals. No financial proof is
// claimed: TestProofs alone admits this seeded history and the [1,2,3] fold proof.
fn payer_projection_wallet(
    core_pending: bool,
    folded_pending: Option<bool>,
) -> (Wallet, [u8; 32], Vec<u8>) {
    use crate::kagemusha_wallet_state_v1::preparation_custody::SourceCustodyV1;
    let (offer, request, credited) = originals();
    let payment: KagemushaWalletPaymentV1 = fixture("KagemushaWalletPaymentV1");
    let credited_digest = credited
        .verify_for(&fixture("KagemushaWalletSchemeV1"), &request, &payment)
        .unwrap()
        .0;
    let anchor = archive::encode(&credited).unwrap();
    let descriptor = KagemushaWalletPendingOutgoingLeafV1 {
        credit_id: payment.request.body.credit_id(),
        receiver_wallet_id: request.body.receiver_wallet_id,
        send_ordinal: request.body.send_ordinal,
        amount: request.body.amount,
        fee: request.body.fee,
        request_digest: request.request_digest(),
    };
    let mut w = wallet();
    let (_, mut manifest) = w.manifest().unwrap();
    w.wallet_id = offer.payer_credential.body.wallet_id;
    w.archive.wallet = w.wallet_id;
    manifest.wallet_id = w.wallet_id;
    let mut pending = map_tree::PersistentMapV1::default();
    pending
        .insert(
            &mut w.archive,
            descriptor.credit_id,
            descriptor.leaf_value().unwrap(),
        )
        .unwrap();
    let empty = map_tree::PersistentMapV1::default();
    let mut current = frozen(
        Some(&bootstrap()),
        KagemushaWalletEffectV1::Load {
            receipt_digest: field(98),
            load_ordinal: 0,
            amount: 1,
            online_charge: 0,
        },
    );
    current.credential = offer.payer_credential.clone();
    let c = &mut current.capsule;
    c.wallet_id = w.wallet_id;
    c.kind = KagemushaWalletOperationKindV1::ArchiveSent;
    c.statement.effect = KagemushaWalletEffectV1::ArchiveSent {
        credit_id: descriptor.credit_id,
        credited: credited_digest,
    };
    c.statement.next_load = 0;
    c.successor_state = KagemushaWalletStateV1::bootstrap(&current.credential, field(92)).unwrap();
    c.successor_state.core.sequence = c.statement.sequence;
    c.successor_state.core.balance = 20;
    let mut source = SourceCustodyV1::bootstrap(
        &mut w.archive,
        &c.successor_state,
        &current.credential.to_canonical_bytes().unwrap(),
        &archive::encode(&offer.certificates).unwrap(),
    )
    .unwrap();
    let selected_pending = if core_pending {
        pending.clone()
    } else {
        empty.clone()
    };
    source.maps.replace_pending(selected_pending.clone());
    c.successor_state.core.pending_outgoing_root = selected_pending.root();
    c.retained_inputs = vec![
        (
            KagemushaWalletRetainedInputRoleV1::Request,
            archive::encode(&request).unwrap(),
        ),
        (
            KagemushaWalletRetainedInputRoleV1::Payment,
            payment.to_canonical_bytes().unwrap(),
        ),
        (KagemushaWalletRetainedInputRoleV1::Credited, anchor.clone()),
        (
            KagemushaWalletRetainedInputRoleV1::Credential,
            current.credential.to_canonical_bytes().unwrap(),
        ),
        (
            KagemushaWalletRetainedInputRoleV1::CertificateSet,
            archive::encode(&offer.certificates).unwrap(),
        ),
    ]
    .into_iter()
    .map(|(role, bytes)| KagemushaWalletRetainedInputV1 { role, bytes })
    .collect();
    rebind_frozen_successor(&mut current);
    current.validate().unwrap();
    let c = &current.capsule;
    let capsule = c.capsule_digest().unwrap();
    let owner = TransitionOwner::new(current.credential.clone());
    let signature: Signature =
        signer(&current.credential).sign(&kagemusha_wallet_signing_message_v1(
            KagemushaWalletSigningDomainV1::Receipt,
            &owner.receipt_body(c, &capsule).unwrap(),
        ));
    let signature = signature.normalize_s().unwrap_or(signature);
    let signature = KagemushaDeviceSignatureV1::from_raw_bytes(&signature.to_bytes()).unwrap();
    let record = owner.assemble(c, &capsule, &signature).unwrap();
    let retained = Retained {
        operation_id: c.operation_id,
        capsule_digest: capsule,
        selected_generation: 3,
        completion_digest: record.completion_digest().unwrap(),
        frame: record.to_canonical_bytes().unwrap(),
        record,
    };
    let mut marker = enrollment();
    marker.wallet_id = w.wallet_id;
    marker.payment_key = current.credential.body.payment_key;
    marker.generation = 4;
    marker.state = c.head_marker_state().unwrap();
    w.custody.status = SlotStatus::Released(marker_record(marker, retained.completion_digest));
    w.custody.retained.insert(c.operation_id, retained.clone());
    w.archive
        .put(
            ArchiveKey::Capsule(capsule),
            &archive::encode(&current).unwrap(),
        )
        .unwrap();
    manifest.indexed = Some(c.statement.sequence);
    manifest.capsule = capsule;
    manifest.steps = manifest
        .steps
        .set(
            &mut w.archive,
            manifest::sequence_key(c.statement.sequence),
            &archive::encode(&manifest::StepEntry {
                capsule,
                operation: c.operation_id,
                selected_generation: 3,
                completion: retained.completion_digest,
                kind: c.kind,
                checkpoints: 0,
                collected: false,
            })
            .unwrap(),
        )
        .unwrap();
    let address = w
        .archive
        .write_object(&archive::encode(&source).unwrap(), 32 * 1024)
        .unwrap();
    manifest.capsule_sources = manifest
        .capsule_sources
        .set(&mut w.archive, capsule, &address)
        .unwrap();
    if let Some(present) = folded_pending {
        let selected = if present { &pending } else { &empty };
        let mut fold: KagemushaWalletFoldRecordV1 = fixture("KagemushaWalletFoldRecordV1");
        fold.scheme_id = w.scheme_id;
        fold.wallet_id = w.wallet_id;
        fold.first_sequence = c.statement.sequence;
        fold.sequence = c.statement.sequence;
        fold.head = c.statement.successor;
        fold.capsule_digest = capsule;
        fold.lineage.public = KagemushaWalletLineagePublicV1 {
            version: 1,
            scheme_id: w.scheme_id,
            relation_id: c.statement.relation_id,
            head: c.statement.successor,
            wallet_id: w.wallet_id,
            credential_digest: c.statement.credential_digest,
            payment_key: current.credential.body.payment_key,
            lifecycle: c.statement.lifecycle,
            policy_epoch: c.successor_state.core.policy_epoch,
            enabled_controls: c.successor_state.core.enabled_controls,
            burned_total: 0,
            pending_outgoing_root: selected.root(),
            credit_digest_root: manifest.credit_tree.root(),
        };
        fold.lineage.proof = vec![1, 2, 3];
        let bytes = archive::encode(&RecordedFold {
            record: fold,
            burned: false,
        })
        .unwrap();
        w.archive
            .put(ArchiveKey::Fold(c.statement.sequence), &bytes)
            .unwrap();
        let digest = crate::kagemusha_wallet_advance_v1::kagemusha_wallet_provider_digest_v1(
            "wallet-recorded-fold",
            &bytes,
        );
        manifest.folds = manifest
            .folds
            .set(
                &mut w.archive,
                manifest::sequence_key(c.statement.sequence),
                &digest,
            )
            .unwrap();
        let address = w
            .archive
            .write_object(&archive::encode(selected).unwrap(), 2048)
            .unwrap();
        manifest.fold_pending = manifest
            .fold_pending
            .set(
                &mut w.archive,
                manifest::sequence_key(c.statement.sequence),
                &address,
            )
            .unwrap();
        manifest.folded = Some(c.statement.sequence);
    }
    w.publish_manifest([0; 32], &manifest).unwrap();
    let send = [0x99; 32];
    let send_operation = payment.send.statement.operation_id(&w.wallet_id).unwrap();
    let send_capsule = payment.send.receipt.capsule_digest;
    w.retain_delivery_test_mapping(
        NativeIntentV1::user(OperationRequestV1 {
            request_id: send,
            action: OperationActionV1::Send {
                request: archive::encode(&request).unwrap(),
            },
        }),
        send_capsule,
        send_operation,
    )
    .unwrap();
    w.custody.tombstones.insert(
        send_operation,
        crate::kagemusha_wallet_advance_v1::KagemushaWalletTombstoneV1 {
            version: 1,
            operation_id: send_operation,
            kind: KagemushaWalletOperationKindV1::Send as u8,
            selected_generation: 1,
            capsule_digest: send_capsule,
            completion_digest: [8; 32],
        },
    );
    let intent = NativeIntentV1::retained_delivery_test_originals(
        &payment,
        archive::encode(&request).unwrap(),
        anchor.clone(),
        current.credential.to_canonical_bytes().unwrap(),
        archive::encode(&offer.certificates).unwrap(),
    );
    w.retain_delivery_test_mapping(intent, capsule, c.operation_id)
        .unwrap();
    (w, send, anchor)
}

#[test]
fn payer_projection_observes_later_credit_status_after_send_collection_without_advance() {
    for burn in [false, true] {
        let (mut receiver, _, receive) = receive_projection_wallet(burn);
        snapshot_test_fold(&mut receiver);
        let newer = receiver
            .receive_credit_projection(&receive)
            .unwrap()
            .bytes()[92..]
            .to_vec();
        let (mut payer, send, anchor) = payer_projection_wallet(false, Some(false));
        assert_eq!(
            payer
                .delivery_credit_projection(&send, &anchor, &[])
                .unwrap()
                .bytes()[2..5],
            [1, 2, 0]
        );
        let result = payer
            .delivery_credit_projection(&send, &anchor, &newer)
            .unwrap()
            .bytes();
        assert_eq!(result[2..5], [if burn { 3 } else { 2 }, 2, 0]);
        assert_eq!(result.len(), 92);
        assert_eq!(payer.custody.signatures, 0);
        assert_eq!(payer.proofs.preparations.load(Ordering::SeqCst), 0);
        assert!(
            payer.receive_credit_projection(&send).is_err(),
            "wrong role must fail"
        );
        assert!(
            payer
                .delivery_credit_projection(&[0x98; 32], &anchor, &newer)
                .is_err()
        );
        let mut foreign: KagemushaWalletCreditedV1 = archive::decode(&newer).unwrap();
        let KagemushaWalletCreditedEvidenceV1::Status { status } = &mut foreign.evidence else {
            panic!("status")
        };
        status.opening.credit_id = field(99);
        assert!(
            payer
                .delivery_credit_projection(&send, &anchor, &archive::encode(&foreign).unwrap())
                .is_err()
        );
        payer.proofs.reject = true;
        assert!(
            payer
                .delivery_credit_projection(&send, &anchor, &newer)
                .is_err(),
            "proof refusal is not a verdict"
        );
    }
}

#[test]
fn payer_projection_distinguishes_awaiting_removed_and_noop_from_current_core_membership() {
    for (folded, expected) in [(None, 1), (Some(false), 2), (Some(true), 3)] {
        for pending in [false, true] {
            let (mut payer, send, anchor) = payer_projection_wallet(pending, folded);
            assert_eq!(
                payer
                    .delivery_credit_projection(&send, &anchor, &[])
                    .unwrap()
                    .bytes()[2..5],
                [1, expected, u8::from(pending)]
            );
            assert_eq!(payer.custody.signatures, 0);
            if folded.is_some() {
                let (selected, mut manifest) = payer.manifest().unwrap();
                let wrong = map_tree::PersistentMapV1::default();
                let address = payer
                    .archive
                    .write_object(&archive::encode(&wrong).unwrap(), 2048)
                    .unwrap();
                if folded == Some(true) {
                    manifest.fold_pending = manifest
                        .fold_pending
                        .set(&mut payer.archive, manifest::sequence_key(1), &address)
                        .unwrap();
                    payer.publish_manifest(selected, &manifest).unwrap();
                    assert!(
                        payer
                            .delivery_credit_projection(&send, &anchor, &[])
                            .is_err(),
                        "map must match verified fold root"
                    );
                }
            }
        }
    }
}

#[test]
fn noop_later_candidate_uses_ordinary_native_preparation_and_never_reissues_send() {
    let (mut receiver, _, receive) = receive_projection_wallet(false);
    snapshot_test_fold(&mut receiver);
    let newer = receiver
        .receive_credit_projection(&receive)
        .unwrap()
        .bytes()[92..]
        .to_vec();
    let (mut payer, send, anchor) = payer_projection_wallet(true, Some(true));
    assert_eq!(
        payer
            .delivery_credit_projection(&send, &anchor, &newer)
            .unwrap()
            .bytes()[3..5],
        [3, 1]
    );
    let (offer, request, _) = originals();
    let payment = fixture("KagemushaWalletPaymentV1");
    // This fixture represents an already retained new private intent. Its only proof provider
    // refuses Archive planning, so the test establishes genuine dispatcher re-entry, not success.
    let intent = NativeIntentV1::retained_delivery_test_originals(
        &payment,
        archive::encode(&request).unwrap(),
        newer.clone(),
        offer.payer_credential.to_canonical_bytes().unwrap(),
        archive::encode(&offer.certificates).unwrap(),
    );
    payer.retain_delivery_test_preparing(intent).unwrap();
    assert!(matches!(
        payer.credited_status_for_send(&send, &newer).unwrap(),
        RequestStatusV1::Preparing
    ));
    assert!(matches!(
        payer.accept_credited_for_send(&send, &newer),
        Err(Error::Invalid("test action"))
    ));
    assert_eq!(payer.proofs.preparations.load(Ordering::SeqCst), 1);
    assert_eq!(payer.custody.signatures, 0);
    assert!(matches!(
        payer.credited_status_for_send(&send, &anchor).unwrap(),
        RequestStatusV1::Outcome(Completion::Complete(_))
    ));
}
