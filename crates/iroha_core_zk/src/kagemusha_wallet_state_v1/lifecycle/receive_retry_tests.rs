//! Receive recovery uses exact originals after definitive nonselection. Proofs in this module
//! are explicit mocks; these tests establish coordinator crash behavior, not cryptographic or
//! physical-device throughput claims.

use super::*;

fn receive_request() -> OperationRequestV1 {
    let payment: KagemushaWalletPaymentV1 = fixture("KagemushaWalletPaymentV1");
    let companion: KagemushaWalletPayerCompanionV1 = fixture("KagemushaWalletPayerCompanionV1");
    OperationRequestV1 {
        request_id: [83; 32],
        action: OperationActionV1::Receive {
            payment: payment.to_canonical_bytes().unwrap(),
            payer_credential: companion.payer_credential.to_canonical_bytes().unwrap(),
            certificates: archive::encode(&companion.certificates).unwrap(),
        },
    }
}

pub(super) fn receive_step(
    source: &PreparationSourceV1<'_>,
    action: &OperationActionV1,
) -> Result<FrozenTransition, Error> {
    use KagemushaWalletRetainedInputRoleV1 as R;
    let OperationActionV1::Receive {
        payment,
        payer_credential,
        certificates,
    } = action
    else {
        return Err(Error::Invalid("test Receive action"));
    };
    let received: KagemushaWalletPaymentV1 = archive::decode(payment)?;
    let request = valid(received.signed_request())?;
    let identity = valid(received.digests())?;
    let previous = &source.released().frozen;
    let (credential, mut body) = frozen_body_for_credential(
        &previous.credential,
        Some(previous),
        KagemushaWalletEffectV1::Receive {
            credit_id: identity.credit_id,
            payer_wallet_id: request.body.payer_wallet_id,
            amount: request.body.amount,
        },
    );
    body.successor_state.core.balance = previous
        .capsule
        .body()
        .successor_state
        .core
        .balance
        .checked_add(request.body.amount)
        .unwrap();
    body.payment_digest = identity.payment;
    body.retained_inputs = [
        (R::Request, archive::encode(&request)?),
        (R::Payment, payment.clone()),
        (R::Credential, payer_credential.clone()),
        (R::CertificateSet, certificates.clone()),
    ]
    .into_iter()
    .map(|(role, bytes)| KagemushaWalletRetainedInputV1 { role, bytes })
    .collect();
    body.statement.successor = valid(body.successor_state.commitment())?;
    body.operation_id = valid(body.statement.operation_id(&body.wallet_id))?;
    body.output = valid(KagemushaWalletOutputDescriptorV1::for_transition(
        &body.statement,
        &valid(body.proof_digest())?,
        &body.payment_digest,
    ))?;
    let next = FrozenTransition {
        credential,
        capsule: seal_body(body),
    };
    next.validate()?;
    Ok(next)
}

fn receiver_unfolded() -> Wallet {
    let original = fixture::<KagemushaWalletRecoveryCapsuleV1>("KagemushaWalletRecoveryCapsuleV1")
        .body()
        .retained_inputs
        .iter()
        .find(|input| input.role == KagemushaWalletRetainedInputRoleV1::Request)
        .unwrap()
        .bytes
        .clone();
    let request: KagemushaWalletRequestV1 = archive::decode(&original).unwrap();
    let credential = request.receiver_credential;
    let all = vectors();
    let binding = all["digests"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row["role"].as_str() == Some("enrollment-key-binding")
                && row["object"].as_str()
                    == Some("receiver App Attest enrollment assertion client data")
        })
        .unwrap();
    let transcript = hex::decode(binding["body_hex"].as_str().unwrap()).unwrap();
    let challenge: [u8; 32] = transcript[..32].try_into().unwrap();
    let marker = KagemushaWalletMarkerV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id: credential.body.scheme_id,
        asset_digest: credential.body.asset_digest,
        wallet_id: credential.body.wallet_id,
        payment_key: credential.body.payment_key,
        generation: 0,
        state: KagemushaWalletMarkerStateV1::Enrollment {
            challenge_digest: challenge,
            enrollment_id: credential.body.enrollment_id,
        },
    };
    marker.validate().unwrap();
    let (_, body) = frozen_body_for_credential(
        &credential,
        None,
        KagemushaWalletEffectV1::Bootstrap {
            enrollment_id: credential.body.enrollment_id,
            enrollment_marker: marker.marker_digest().unwrap(),
        },
    );
    let boot = FrozenTransition {
        credential,
        capsule: seal_body(body),
    };
    let mut archive = MemoryArchive::new();
    archive.scheme = credential.body.scheme_id;
    archive.wallet = credential.body.wallet_id;
    let mut wallet = Coordinator::new(
        TestCustody::for_enrollment(&credential, &marker),
        archive,
        TestProofs {
            receive_plans: true,
            ..TestProofs::default()
        },
        credential.body.scheme_id,
        credential.body.wallet_id,
    )
    .unwrap();
    wallet.commit(boot).unwrap();
    wallet
}

fn receiver() -> Wallet {
    let mut wallet = receiver_unfolded();
    snapshot_test_fold(&mut wallet);
    wallet
}

// Another Receive runs through production typed dispatch, so its source-custody
// snapshot is published together with its permanent operation and credit mappings.
fn advance(wallet: &mut Wallet) {
    let fail_first_proof = wallet.proofs.fail_preparation_proof;
    wallet.proofs.fail_preparation_proof = false;
    let result = wallet.execute(numbered_receive(200));
    wallet.proofs.fail_preparation_proof = fail_first_proof;
    assert!(matches!(result.unwrap(), Completion::Complete(_)));
}

fn assert_complete_once(wallet: &mut Wallet, signatures: usize) {
    let request = receive_request();
    let result = wallet.execute(request.clone()).unwrap();
    assert!(matches!(result, Completion::Complete(_)));
    assert_eq!(wallet.custody.signatures, signatures + 1);
    let proofs = wallet.proofs.preparation_proofs.load(Ordering::SeqCst);
    assert_eq!(wallet.execute(request.clone()).unwrap(), result);
    assert_eq!(wallet.custody.signatures, signatures + 1);
    assert_eq!(
        wallet.proofs.preparation_proofs.load(Ordering::SeqCst),
        proofs
    );
    let step = wallet.released_steps().unwrap().pop().unwrap();
    let OperationActionV1::Receive { payment, .. } = request.action else {
        unreachable!()
    };
    assert!(
        step.frozen
            .capsule
            .body()
            .retained_inputs
            .iter()
            .any(|input| {
                input.role == KagemushaWalletRetainedInputRoleV1::Payment && input.bytes == payment
            })
    );
}

#[test]
fn stale_receive_plan_restarts_with_same_identity_and_exact_payment() {
    let mut wallet = receiver();
    wallet.proofs.fail_preparation_proof = true;
    assert!(wallet.execute(receive_request()).is_err());
    advance(&mut wallet);
    let signatures = wallet.custody.signatures;
    wallet.proofs.fail_preparation_proof = false;
    let mut wallet = restart(wallet);
    assert_complete_once(&mut wallet, signatures);
    let mut wallet = restart(wallet);
    let proofs = wallet.proofs.preparation_proofs.load(Ordering::SeqCst);
    assert!(matches!(
        wallet.execute(receive_request()).unwrap(),
        Completion::Complete(_)
    ));
    assert_eq!(wallet.custody.signatures, signatures + 1);
    assert_eq!(
        wallet.proofs.preparation_proofs.load(Ordering::SeqCst),
        proofs
    );
}

#[test]
fn stale_receive_capsule_survives_every_retirement_boundary() {
    for boundary in 0..9 {
        let mut wallet = receiver();
        let before = capsules(&wallet);
        wallet.proofs.fail_commit_derivation = true;
        assert!(wallet.execute(receive_request()).is_err());
        let stale: Vec<_> = capsules(&wallet).difference(&before).copied().collect();
        assert_eq!(stale.len(), 1);
        let stale = stale[0];
        wallet.proofs.fail_commit_derivation = false;
        advance(&mut wallet);
        let signatures = wallet.custody.signatures;
        let calls = wallet.custody.publication_calls;
        let draft = ArchiveKey::PreparationDraft(receive_request().request_id);
        match boundary {
            0 | 1 => wallet.archive.fail_remove_at = Some((draft, boundary == 1)),
            2 | 3 => {
                wallet.archive.fail_remove_at = Some((ArchiveKey::Capsule(stale), boundary == 3))
            }
            4 | 5 => wallet.custody.fail_publication_at = Some((calls + 1, boundary == 5)),
            6 => wallet.proofs.fail_preparation_plan = true,
            7 => wallet.proofs.fail_preparation_proof = true,
            _ => *wallet.archive.fail_put.lock().unwrap() = Some((draft, false)),
        }
        assert!(
            wallet.execute(receive_request()).is_err(),
            "boundary {boundary}"
        );
        assert_eq!(wallet.custody.signatures, signatures, "boundary {boundary}");
        wallet.proofs.fail_preparation_plan = false;
        wallet.proofs.fail_preparation_proof = false;
        let mut wallet = restart(wallet);
        assert_complete_once(&mut wallet, signatures);
        assert!(!capsules(&wallet).contains(&stale), "boundary {boundary}");
    }
}

#[test]
fn stale_receive_batch_keeps_original_across_every_retirement_boundary() {
    for boundary in 0..9 {
        let mut wallet = receiver();
        let request = receive_request();
        wallet.proofs.preparation_payload = vec![42; 97];
        let object = object_digest(&wallet.proofs.preparation_payload);
        *wallet.archive.fail_put.lock().unwrap() = Some((ArchiveKey::Object(object), false));
        assert!(matches!(
            wallet.execute(request.clone()),
            Err(Error::Storage(_))
        ));
        // A fresh Receive already has only its binding and durable batch here; unlike
        // the Archive fixture, no delivery helper pre-published its request original.
        wallet
            .require_intake_batch_only_test_original(&request)
            .unwrap();
        assert_eq!(
            wallet.retry_request(&request.request_id).unwrap(),
            RequestStatusV1::Preparing
        );
        advance(&mut wallet);
        let signatures = wallet.custody.signatures;
        let calls = wallet.custody.publication_calls;
        let draft = ArchiveKey::PreparationDraft(request.request_id);
        match boundary {
            0 | 1 => wallet.custody.fail_publication_at = Some((calls + 1, boundary == 1)),
            2 | 3 => wallet.archive.fail_remove_at = Some((draft, boundary == 3)),
            4 | 5 => *wallet.archive.fail_put.lock().unwrap() = Some((draft, boundary == 5)),
            6 | 7 => wallet.custody.fail_publication_at = Some((calls + 2, boundary == 7)),
            _ => wallet.proofs.fail_preparation_proof = true,
        }
        assert!(wallet.execute(request).is_err(), "boundary {boundary}");
        assert_eq!(wallet.custody.signatures, signatures, "boundary {boundary}");
        wallet.proofs.fail_preparation_proof = false;
        let mut wallet = restart(wallet);
        assert_complete_once(&mut wallet, signatures);
        assert!(!wallet.archive.records.lock().unwrap().contains_key(&draft));
    }
}

#[test]
fn stale_receive_retirement_never_converts_unavailable_or_lost_custody_to_absence() {
    for boundary in 0..3 {
        let mut wallet = receiver();
        let before = capsules(&wallet);
        wallet.proofs.fail_commit_derivation = true;
        assert!(wallet.execute(receive_request()).is_err());
        let stale: Vec<_> = capsules(&wallet).difference(&before).copied().collect();
        assert_eq!(stale.len(), 1);
        let operation = wallet.frozen(stale[0]).unwrap().capsule.body().operation_id;
        wallet.proofs.fail_commit_derivation = false;
        advance(&mut wallet);
        let retained = capsules(&wallet);
        let signatures = wallet.custody.signatures;
        let plans = wallet.proofs.preparations.load(Ordering::SeqCst);
        match boundary {
            0 => wallet.custody.unavailable = true,
            1 => wallet.custody.delivery_lost = true,
            _ => {
                let (selected, mut manifest) = wallet.manifest().unwrap();
                manifest.operations = manifest
                    .operations
                    .set(&mut wallet.archive, operation, &[0; 32])
                    .unwrap();
                wallet.publish_manifest(selected, &manifest).unwrap();
            }
        }
        let result = wallet.execute(receive_request());
        if boundary == 1 {
            assert_eq!(result.unwrap(), Completion::DeliveryDataLoss);
        } else {
            assert!(result.is_err());
        }
        assert_eq!(capsules(&wallet), retained);
        assert_eq!(wallet.custody.signatures, signatures);
        assert_eq!(wallet.proofs.preparations.load(Ordering::SeqCst), plans);
    }
}

#[test]
fn selected_receive_pending_and_missing_output_are_never_replanned() {
    let mut wallet = receiver();
    let request = receive_request();
    wallet.custody.pause = true;
    assert_eq!(
        wallet.execute(request.clone()).unwrap(),
        Completion::Pending
    );
    let plans = wallet.proofs.preparations.load(Ordering::SeqCst);
    let proofs = wallet.proofs.preparation_proofs.load(Ordering::SeqCst);
    let mut wallet = restart(wallet);
    assert_eq!(
        wallet.execute(request.clone()).unwrap(),
        Completion::Pending
    );
    assert_eq!(wallet.proofs.preparations.load(Ordering::SeqCst), plans);
    assert_eq!(
        wallet.proofs.preparation_proofs.load(Ordering::SeqCst),
        proofs
    );
    wallet.custody.pause = false;
    assert!(matches!(
        wallet.resume().unwrap(),
        Some(Completion::Complete(_))
    ));
    let step = wallet.released_steps().unwrap().pop().unwrap();
    let operation = step.frozen.capsule.body().operation_id;
    wallet.custody.retained.remove(&operation).unwrap();
    assert_eq!(
        wallet.execute(request).unwrap(),
        Completion::DeliveryDataLoss
    );
    assert_eq!(wallet.proofs.preparations.load(Ordering::SeqCst), plans);
    assert_eq!(
        wallet.proofs.preparation_proofs.load(Ordering::SeqCst),
        proofs
    );
}

// Unique fixture payments retain the receiver's open signed terms. Only payer completion
// and its real P-256 receipt are changed; sigma/omega remain explicit test stand-ins.
fn numbered_receive(number: u8) -> OperationRequestV1 {
    let mut input = receive_request();
    input.request_id = field(number);
    let OperationActionV1::Receive {
        payment,
        payer_credential,
        ..
    } = &mut input.action
    else {
        unreachable!()
    };
    let credential: KagemushaWalletCredentialV1 = archive::decode(payer_credential).unwrap();
    let mut value: KagemushaWalletPaymentV1 = archive::decode(payment).unwrap();
    let KagemushaWalletEffectV1::Send { send_ordinal, .. } = &mut value.send.statement.effect
    else {
        unreachable!()
    };
    *send_ordinal = u128::from(number);
    let signed = value.signed_request().unwrap();
    let KagemushaWalletEffectV1::Send {
        credit_id, request, ..
    } = &mut value.send.statement.effect
    else {
        unreachable!()
    };
    *credit_id = signed.credit_id();
    *request = signed.request_digest();
    value.send.receipt.operation_id = value
        .send
        .statement
        .operation_id(&credential.body.wallet_id)
        .unwrap();
    let signer_context = KagemushaWalletReceiptSignerV1::from_credential(&credential).unwrap();
    let body = value
        .send
        .receipt
        .body(
            &signer_context,
            &value.send.statement,
            &value.send.proof_digest().unwrap(),
        )
        .unwrap();
    let signature: Signature = signer(&credential).sign(&body.signing_message());
    let signature = signature.normalize_s();
    value.send.receipt.signature =
        KagemushaDeviceSignatureV1::from_raw_bytes(&signature.to_bytes()).unwrap();
    value.digests().unwrap();
    *payment = value.to_canonical_bytes().unwrap();
    input
}

#[test]
fn hundred_receives_survive_fold_publication_restart_and_exact_replay() {
    use std::ops::ControlFlow;
    let mut wallet = receiver_unfolded();
    wallet.scheduler().set_activity(true, false);
    // Prove the bootstrap checkpoint, but hold publication while every receive advances
    // custody. The proof must rebase only its fold fields, retaining all 100 credit entries.
    let ControlFlow::Continue(job) = wallet.fold_prepare().unwrap() else {
        panic!("bootstrap proof")
    };
    let publication = job.prove().unwrap();
    let initial = wallet.snapshot().unwrap().owned_balance;
    let requests: Vec<_> = (1..=100).map(numbered_receive).collect();
    let amount = match &requests[0].action {
        OperationActionV1::Receive { payment, .. } => {
            archive::decode::<KagemushaWalletPaymentV1>(payment)
                .unwrap()
                .signed_request()
                .unwrap()
                .body
                .amount
        }
        _ => unreachable!(),
    };
    let mut exact = Vec::new();
    let mut credits = std::collections::BTreeSet::new();
    for request in &requests {
        let OperationActionV1::Receive { payment, .. } = &request.action else {
            unreachable!()
        };
        let payment: KagemushaWalletPaymentV1 = archive::decode(payment).unwrap();
        assert!(credits.insert(payment.digests().unwrap().credit_id));
        exact.push(wallet.execute(request.clone()).unwrap());
        assert!(matches!(exact.last(), Some(Completion::Complete(_))));
    }
    assert_eq!(wallet.custody.signatures, 101);
    assert_eq!(wallet.fold_publish(publication).unwrap().backlog, Some(101));
    assert_eq!(wallet.fold_once().unwrap(), FoldStatus::Folded(0));
    let mut wallet = restart(wallet);
    let snapshot = wallet.snapshot().unwrap();
    assert_eq!(snapshot.owned_balance, initial + 100 * amount);
    assert_eq!(snapshot.fold_backlog, 100);
    assert_eq!(
        wallet
            .released_steps()
            .unwrap()
            .last()
            .unwrap()
            .frozen
            .capsule
            .body()
            .successor_state
            .core
            .burned_total,
        0
    );
    let proofs = wallet.proofs.preparation_proofs.load(Ordering::SeqCst);
    for (request, exact) in requests.iter().zip(&exact) {
        let OperationActionV1::Receive { payment, .. } = &request.action else {
            unreachable!()
        };
        let identity = archive::decode::<KagemushaWalletPaymentV1>(payment)
            .unwrap()
            .digests()
            .unwrap();
        assert!(
            wallet
                .consumed_credit(&identity.credit_id, &identity.payment)
                .unwrap()
                .is_some()
        );
        assert_eq!(&wallet.execute(request.clone()).unwrap(), exact);
    }
    assert_eq!(
        wallet.snapshot().unwrap().owned_balance,
        initial + 100 * amount
    );
    assert_eq!(wallet.custody.signatures, 101);
    assert_eq!(
        wallet.proofs.preparation_proofs.load(Ordering::SeqCst),
        proofs
    );
    wallet.scheduler().set_activity(true, false);
    for sequence in 1..=100 {
        assert!(matches!(
            wallet.fold_once().unwrap(),
            FoldStatus::Checkpoint { .. }
        ));
        assert_eq!(wallet.fold_once().unwrap(), FoldStatus::Folded(sequence));
        let (_, manifest) = wallet.manifest().unwrap();
        let step = wallet.indexed_step(&manifest, sequence).unwrap();
        let fold = wallet.read_fold(&step).unwrap().unwrap();
        assert_eq!(fold.record.lineage.public.burned_total, 0);
    }
    let mut wallet = restart(wallet);
    let final_state = wallet.snapshot().unwrap();
    assert_eq!(final_state.fold_backlog, 0);
    assert_eq!(final_state.owned_balance, initial + 100 * amount);
    assert_eq!(final_state.folded_balance, Some(initial + 100 * amount));
    for (request, exact) in requests.iter().zip(&exact) {
        assert_eq!(&wallet.execute(request.clone()).unwrap(), exact);
    }
    assert_eq!(wallet.custody.signatures, 101);
}

#[test]
fn stale_receive_replanning_cannot_replace_the_bound_payment_or_companion() {
    for component in 0..3 {
        let mut wallet = receiver();
        wallet.proofs.fail_commit_derivation = true;
        assert!(wallet.execute(receive_request()).is_err());
        wallet.proofs.fail_commit_derivation = false;
        advance(&mut wallet);
        let retained = capsules(&wallet);
        let signatures = wallet.custody.signatures;
        let plans = wallet.proofs.preparations.load(Ordering::SeqCst);
        let mut changed = receive_request();
        let OperationActionV1::Receive {
            payment,
            payer_credential,
            certificates,
        } = &mut changed.action
        else {
            unreachable!()
        };
        let bytes = match component {
            0 => payment,
            1 => payer_credential,
            _ => certificates,
        };
        bytes[0] ^= 1;
        assert!(wallet.execute(changed).is_err());
        assert_eq!(capsules(&wallet), retained);
        assert_eq!(wallet.custody.signatures, signatures);
        assert_eq!(wallet.proofs.preparations.load(Ordering::SeqCst), plans);
        assert_complete_once(&mut wallet, signatures);
    }
}

/// Receive admission (design §3.2) runs inside native planning; this mock refuses there exactly
/// as the native owner does when the fold would burn the Payment.
#[test]
fn receive_admission_refusal_unbinds_the_fresh_request_and_moves_no_value() {
    let mut wallet = receiver();
    let request = receive_request();
    let refused = |result: Result<Completion, Error>| {
        matches!(result, Err(Error::Invalid(field))
            if field == crate::kagemusha_wallet_state_v1::native_worker::RECEIVE_ADMISSION_REFUSAL)
    };
    let before = wallet.snapshot().unwrap();
    let signatures = wallet.custody.signatures;
    let retained = capsules(&wallet);
    wallet.proofs.refuse_receive_admission = true;
    assert!(refused(wallet.execute(request.clone())));
    // Nothing references the refused Payment: the request reads Unknown, so both apps
    // quarantine it, and no signature, capsule, head or balance changed.
    assert_eq!(
        wallet.retry_request(&request.request_id).unwrap(),
        RequestStatusV1::Unknown
    );
    let after = wallet.snapshot().unwrap();
    assert_eq!(
        (
            after.head,
            after.sequence,
            after.balance,
            after.owned_balance
        ),
        (
            before.head,
            before.sequence,
            before.balance,
            before.owned_balance
        )
    );
    assert_eq!(wallet.custody.signatures, signatures);
    assert_eq!(capsules(&wallet), retained);
    // The same exact bytes meet the same refusal, also after a restart.
    assert!(refused(wallet.execute(request.clone())));
    let mut wallet = restart(wallet);
    assert!(refused(wallet.execute(request.clone())));
    assert_eq!(
        wallet.retry_request(&request.request_id).unwrap(),
        RequestStatusV1::Unknown
    );
    assert_eq!(wallet.custody.signatures, signatures);
    // The refusal left nothing behind: once admitted, the same identity completes exactly once.
    wallet.proofs.refuse_receive_admission = false;
    assert_complete_once(&mut wallet, signatures);
}
