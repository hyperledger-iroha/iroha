//! Observer transactions retain actual native challenges, scoped fees and exact signature bytes.
use super::*;

#[test]
fn observer_factory_from_account_workflow_retains_exact_payload_and_native_floor() {
    let mut f = Fixture::new();
    let receipt = f.receipt_check(None);
    let prepared = f.prepare(receipt);
    let observer = prepared
        .observer_transactions(FeePaymentIntent::authority(Vec::new(), None))
        .unwrap();
    let check = f.begin_account_check(&prepared);
    let instruction = check.instruction().clone();
    let FinalPromotionAccountCustodyActionV1::Check(expected) = &instruction.action else {
        panic!("account Check");
    };
    let payload = f.observer_payload(instruction.clone().into());
    let exact = payload.clone();
    let mut calls = 0;
    let pending = observer
        .sign_account_with(check, payload, |request| {
            calls += 1;
            assert_eq!(request.payload(), &exact);
            assert_eq!(
                request.signing_message(),
                TransactionBuilder::from_payload(exact.clone())
                    .unwrap()
                    .payload_hash_bytes()
            );
            Signature::try_new(key(3).private_key(), request.signing_message())
                .map_err(|_| ObserverError::Provider)
        })
        .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(pending.signed_transaction().payload(), &exact);
    assert_eq!(
        f.native
            .commit(NOW, vec![pending.signed_transaction().clone()]),
        [true]
    );
    let verified = pending.verify_finalized(|| Ok(times().1)).unwrap();
    assert_eq!(verified.instruction(), &instruction);
    assert_eq!(verified.original_floor().height, expected.minimum_height);
    assert_eq!(
        verified.original_floor().block_hash,
        expected.minimum_block_hash
    );
    prepared.authorize(verified, times().0, times().1).unwrap();
}

#[test]
fn observer_rejects_instruction_purpose_authority_and_fee_substitution_before_key_io() {
    let mut f = Fixture::new();
    let receipt = f.receipt_check(None);
    let account_prepared = f.prepare(receipt);
    let foreign_purpose = f
        .begin_account_check(&account_prepared)
        .instruction()
        .clone();
    let observer = f.observer_transactions();
    let floor = f.native.finalized_floor();
    for mutation in 0..7 {
        let prepared = f.prepare_receipt_check(None, Duration::from_secs(60));
        let original = prepared.instruction().clone();
        let mut payload = f.observer_payload(original.clone().into());
        match mutation {
            0 => payload.authority = account(2),
            1 => payload.domain = TransactionDomain::Genesis,
            2 => {
                let mut substituted = original.clone();
                let FinalPromotionAuthorityActionV1::Check(check) = &mut substituted.action else {
                    panic!("Check");
                };
                check.challenge = [0xFE; 32];
                payload.instructions = Executable::Instructions(vec![substituted.into()].into());
            }
            3 => {
                payload.instructions =
                    Executable::Instructions(vec![foreign_purpose.clone().into()].into())
            }
            4 => {
                payload.instructions = Executable::Instructions(
                    vec![original.clone().into(), original.clone().into()].into(),
                )
            }
            5 => payload.instructions = account_prepared.payload.instructions.clone(),
            6 => {
                payload = TransactionBuilder::new(
                    *f.native.state().network_id_ref(),
                    account(3),
                    FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(1)),
                )
                .with_instructions([original])
                .into_payload()
                .unwrap()
            }
            _ => unreachable!(),
        }
        let rejected = observer.sign_receipt_with(prepared, payload, |_| {
            panic!("payload rejection must precede observer key I/O")
        });
        assert!(
            matches!(rejected, Err(ObserverError::Payload)),
            "mutation {mutation}"
        );
        assert_eq!(f.native.finalized_floor(), floor);
    }
    let prepared = f.begin_account_check(&account_prepared);
    let receipt = f.prepare_receipt_check(None, Duration::from_secs(60));
    let payload = f.observer_payload(receipt.instruction().clone().into());
    assert!(matches!(
        observer.sign_account_with(prepared, payload, |_| panic!(
            "cross-purpose Check cannot reach signer"
        )),
        Err(ObserverError::Payload)
    ));
}

#[test]
fn observer_rejects_protected_keys_provider_substitution_and_wrong_message() {
    let f = Fixture::new();
    for observer in [account(2), account(4)] {
        assert!(matches!(
            FinalPromotionObserverTransactionsV1::new(
                f.receipt_policy.binding.clone(),
                f.account_policy.binding.clone(),
                observer,
                FeePaymentIntent::authority(Vec::new(), None),
            ),
            Err(ObserverError::Binding)
        ));
    }
    let mut wrong_deployment = f.account_policy.binding.clone();
    wrong_deployment.purpose = SignerPurposeBindingV1::FinalPromotionAccountTransaction {
        deployment_id: "promotion-secondary".into(),
    };
    assert!(matches!(
        FinalPromotionObserverTransactionsV1::new(
            f.receipt_policy.binding.clone(),
            wrong_deployment,
            account(3),
            FeePaymentIntent::authority(Vec::new(), None),
        ),
        Err(ObserverError::Binding)
    ));
    let observer = f.observer_transactions();
    for (signer, wrong_message) in [(2, false), (4, false), (9, false), (3, true)] {
        let prepared = f.prepare_receipt_check(None, Duration::from_secs(60));
        let payload = f.observer_payload(prepared.instruction().clone().into());
        let mut calls = 0;
        let result = observer.sign_receipt_with(prepared, payload, |request| {
            calls += 1;
            let message = if wrong_message {
                b"another observer transaction"
            } else {
                request.signing_message()
            };
            Signature::try_new(key(signer).private_key(), message)
                .map_err(|_| ObserverError::Provider)
        });
        assert_eq!(calls, 1);
        assert!(matches!(result, Err(ObserverError::Provider)));
    }
}

#[test]
fn observer_requires_complete_account_binding_before_key_io_with_unchanged_public_target() {
    let mut f = Fixture::new();
    let receipt = f.receipt_check(None);
    let account_prepared = f.prepare(receipt);
    let observer = f.observer_transactions();
    let original = f.begin_account_check(&account_prepared);
    assert_eq!(original.binding(), &f.account_policy.binding);
    let instruction = original.instruction().clone();
    let (height, block_hash, context_id) = f.native.finalized_floor().unwrap();
    for mutation in 0..8 {
        let mut binding = f.account_policy.binding.clone();
        match mutation {
            0 => binding.runtime_handle.push_str("-alternate"),
            1 => binding.key_handle.push_str("-alternate"),
            2 => binding.key_revision += 1,
            3 => binding.policy_revision += 1,
            4 => binding.policy_digest = [0xAD; 32],
            5 => binding.chain_id = "alternate-production-chain".into(),
            6 => binding.service_id.push_str("-alternate"),
            7 => binding.administrator_id.push_str("-alternate"),
            _ => unreachable!(),
        }
        binding.validate().unwrap();
        assert_eq!(binding.network_id, f.account_policy.binding.network_id);
        assert_eq!(binding.purpose, f.account_policy.binding.purpose);
        assert_eq!(binding.public_key, f.account_policy.binding.public_key);
        let prepared = begin_final_promotion_account_check_v1(
            Arc::clone(f.native.state()),
            FinalPromotionAccountCheckExpectedV1 {
                binding: binding.clone(),
                observer: account(3),
                expected_account: account(2),
                transaction_payload_digest: account_prepared.payload_digest(),
                control_revision: instruction.expected_control_revision,
                control_digest: instruction.expected_control_digest,
                floor: FinalPromotionAccountCheckFloorV1 {
                    height,
                    block_hash,
                    context_id,
                },
            },
            Duration::from_secs(60),
        )
        .unwrap();
        assert_eq!(prepared.binding(), &binding);
        let payload = f.observer_payload(prepared.instruction().clone().into());
        assert!(
            matches!(
                observer.sign_account_with(prepared, payload, |_| {
                    panic!("complete binding mismatch must precede observer key I/O")
                }),
                Err(ObserverError::Binding)
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn observer_expiry_during_key_io_does_not_renew_original_challenge() {
    let f = Fixture::new();
    let observer = f.observer_transactions();
    let prepared = f.prepare_receipt_check(None, Duration::from_secs(1));
    let payload = f.observer_payload(prepared.instruction().clone().into());
    let floor = f.native.finalized_floor();
    let mut calls = 0;
    let result = observer.sign_receipt_with(prepared, payload, |request| {
        calls += 1;
        std::thread::sleep(Duration::from_millis(1_100));
        Signature::try_new(key(3).private_key(), request.signing_message())
            .map_err(|_| ObserverError::Provider)
    });
    assert_eq!(calls, 1);
    assert!(matches!(result, Err(ObserverError::Check)));
    assert_eq!(f.native.finalized_floor(), floor);
}
