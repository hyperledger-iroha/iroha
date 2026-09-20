//! Account-custody observations with exact native execution and real fixed-roster BLS/RS16 proofs.
//! These simulated device attestations and local finality fixtures do not qualify production custody.
use super::*;
use iroha_crypto::{Hash, SignatureOf};
use iroha_data_model::{
    isi::{InstructionBox, Revoke},
    sorafs::final_promotion_account_custody::FinalPromotionAccountCustodyRevocationV1,
    transaction::TransactionSignature,
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsFinalPromotionAccountCustody, CanOperateSorafsFinalPromotion,
};
mod fixture;
mod recheck_interval;
use fixture::{DEPLOYMENT, Fixture, NOW, key};
fn interval(
    earliest_unix_ms: u64,
    latest_unix_ms: u64,
) -> FinalPromotionAccountEligibilityTimeIntervalV1 {
    FinalPromotionAccountEligibilityTimeIntervalV1 {
        earliest_unix_ms,
        latest_unix_ms,
    }
}
fn prepared_instruction(prepared: &PreparedFinalPromotionAccountCheckV1) -> InstructionBox {
    prepared.instruction().clone().into()
}

#[test]
fn exact_account_check_joins_real_finality_and_distinct_current_accounts() {
    let mut f = Fixture::new();
    assert!(f.policy.binding.runtime_handle.starts_with("software://"));
    assert!(f.policy.binding.key_handle.starts_with("software://"));
    let pending = f.pending();
    let signed = pending.signed_transaction().clone();
    let expected_hash = signed.hash_as_entrypoint();
    assert_eq!(f.commit(NOW, vec![signed], true, true), [true]);
    let verified = pending.verify_finalized(|| Ok(interval(NOW, NOW))).unwrap();
    assert_eq!(verified.check_height(), 3);
    assert_eq!(verified.applied_floor().height, 3);
    assert_eq!(
        verified.applied_floor().context_id,
        f.finalized[2].proof().finality_artifact.context_id()
    );
    assert_eq!(
        verified.applied_floor().block_hash,
        *f.finalized[2].block().hash().as_ref()
    );
    assert_eq!(
        verified.observer(),
        &AccountId::new(key(2).public_key().clone())
    );
    assert_eq!(verified.entry_hash(), expected_hash);
    assert_eq!(verified.eligibility_time_interval(), interval(NOW, NOW));
    assert_eq!(verified.snapshot(), &f.snapshot());
    assert!(
        matches!(&verified.instruction().action, FinalPromotionAccountCustodyActionV1::Check(check) if check.challenge != [0; 32])
    );
    verified.ensure_live().unwrap();
    let FinalPromotionAccountCustodyActionV1::Check(check) = &verified.instruction().action else {
        panic!("exact account Check");
    };
    assert_eq!(
        check.expected_account,
        AccountId::new(key(4).public_key().clone())
    );
    assert_ne!(&check.expected_account, verified.observer());
    assert_eq!(check.transaction_payload_digest, [31; 32]);
}

#[test]
fn coherent_authenticated_descendant_cut_rechecks_current_authority() {
    let mut f = Fixture::new();
    let pending = f.pending();
    assert_eq!(
        f.commit(NOW, vec![pending.signed_transaction().clone()], true, true),
        [true]
    );
    f.commit(NOW + 1, Vec::new(), true, true);
    let verified = pending
        .verify_finalized(|| Ok(interval(NOW + 1, NOW + 1)))
        .unwrap();
    assert_eq!(verified.check_height(), 3);
    assert_eq!(verified.applied_floor().height, 4);
    assert_eq!(verified.snapshot().custody_anchor.height, 4);
}

#[test]
fn successful_check_cannot_hide_later_same_block_custody_revocation() {
    let mut f = Fixture::new();
    let pending = f.pending();
    let revoke = f.instruction(FinalPromotionAccountCustodyActionV1::Revoke(
        FinalPromotionAccountCustodyRevocationV1 {
            signer: true,
            attester: false,
        },
    ));
    assert_eq!(
        f.commit(
            NOW,
            vec![
                pending.signed_transaction().clone(),
                f.sign(revoke.into(), 1, NOW)
            ],
            true,
            true
        ),
        [true, true]
    );
    assert_eq!(
        pending.verify_finalized(|| Ok(interval(NOW, NOW))).err(),
        Some(Error::Authority)
    );
}

#[test]
fn rejection_result_is_not_a_successful_check_even_with_real_finality() {
    let mut f = Fixture::new();
    let pending = f.pending();
    // Native enrollment expires exactly at 100000. Preserve its actual rejection in the wire;
    // a clock callback returning an earlier time must never hide that aligned failed result.
    assert_eq!(
        f.commit(
            100_000,
            vec![pending.signed_transaction().clone()],
            true,
            true
        ),
        [false]
    );
    assert_eq!(
        pending
            .verify_finalized(|| panic!("rejected result must precede clock"))
            .err(),
        Some(Error::Execution)
    );
}

#[test]
fn durable_check_without_actual_state_membership_is_not_observation() {
    let mut f = Fixture::new();
    let pending = f.pending();
    assert_eq!(
        f.commit(NOW, vec![pending.signed_transaction().clone()], false, true),
        [true]
    );
    assert_eq!(
        pending
            .verify_finalized(|| panic!("unapplied member must precede clock"))
            .err(),
        Some(Error::NotApplied)
    );
}

#[test]
fn applied_check_without_durable_finality_is_rejected() {
    let mut f = Fixture::new();
    let pending = f.pending();
    assert_eq!(
        f.commit(NOW, vec![pending.signed_transaction().clone()], true, false),
        [true]
    );
    assert_eq!(
        pending
            .verify_finalized(|| panic!("missing finality must precede clock"))
            .err(),
        Some(Error::Finality)
    );
}

#[test]
fn independent_floor_hash_and_committee_context_cannot_come_from_candidate() {
    for change_hash in [true, false] {
        let mut f = Fixture::new();
        let mut expected = f.expected();
        if change_hash {
            expected.floor.block_hash = [0x91; 32];
        } else {
            expected.floor.context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"independent foreign committee",
            )));
        }
        let prepared = begin_final_promotion_account_check_v1(
            Arc::clone(&f.state),
            expected,
            Duration::from_secs(60),
        )
        .unwrap();
        let signed = f.sign(prepared_instruction(&prepared), 2, NOW);
        let pending = prepared.bind_signed_transaction(signed).unwrap();
        let results = f.commit(NOW, vec![pending.signed_transaction().clone()], true, true);
        assert_eq!(results, [!change_hash]);
        assert_eq!(
            pending
                .verify_finalized(|| panic!("foreign floor must precede clock"))
                .err(),
            Some(Error::Finality)
        );
    }
}

#[test]
fn each_round_has_fresh_entropy_and_signed_envelopes_cannot_be_replaced() {
    let f = Fixture::new();
    let first = f.prepared();
    let second = f.prepared();
    assert_ne!(first.instruction(), second.instruction());
    let signed_other = f.sign(prepared_instruction(&second), 2, NOW);
    assert_eq!(
        first.bind_signed_transaction(signed_other).err(),
        Some(Error::Transaction)
    );
    let signed_wrong_account = f.sign(prepared_instruction(&second), 1, NOW);
    assert_eq!(
        second.bind_signed_transaction(signed_wrong_account).err(),
        Some(Error::Transaction)
    );
}

#[test]
fn signature_verification_is_independent_of_signed_intent_identity() {
    let f = Fixture::new();
    let prepared = f.prepared();
    let mut signed = f.sign(prepared_instruction(&prepared), 2, NOW);
    let original_hash = signed.hash_as_entrypoint();
    signed.set_signature(TransactionSignature(SignatureOf::from_signature(
        iroha_crypto::Signature::try_new(key(3).private_key(), b"wrong signed bytes").unwrap(),
    )));
    assert_eq!(
        signed.hash_as_entrypoint(),
        original_hash,
        "intent identity excludes authorization"
    );
    assert_eq!(
        prepared.bind_signed_transaction(signed).err(),
        Some(Error::Transaction)
    );
}

#[test]
fn signing_and_terminal_verification_never_reset_the_one_use_interval() {
    let mut f = Fixture::new();
    let mut prepared = f.prepared();
    let signed = f.sign(prepared_instruction(&prepared), 2, NOW);
    prepared.round.expire_for_test();
    assert_eq!(
        prepared.bind_signed_transaction(signed).err(),
        Some(Error::Expired)
    );
    let mut pending = f.pending();
    f.commit(NOW, vec![pending.signed_transaction().clone()], true, true);
    pending.prepared.round.expire_for_test();
    assert_eq!(pending.ensure_live(), Err(Error::Expired));
    assert_eq!(
        pending
            .verify_finalized(|| panic!("expiry must precede clock"))
            .err(),
        Some(Error::Expired)
    );
}

#[test]
fn unavailable_or_invalid_eligibility_clock_fails_closed_after_proof() {
    for now in [
        Err(Error::Clock),
        Ok(interval(0, 0)),
        Ok(interval(u64::MAX, u64::MAX)),
        Ok(interval(0, NOW)),
        Ok(interval(NOW, 0)),
        Ok(interval(NOW, u64::MAX)),
        Ok(interval(NOW + 1, NOW)),
    ] {
        let mut f = Fixture::new();
        let pending = f.pending();
        f.commit(NOW, vec![pending.signed_transaction().clone()], true, true);
        assert_eq!(pending.verify_finalized(|| now).err(), Some(Error::Clock));
    }
}

#[test]
fn historical_future_dated_qc_cannot_stand_in_for_a_new_round() {
    let mut f = Fixture::new();
    let old = f.pending();
    assert_eq!(
        f.commit(90_000, vec![old.signed_transaction().clone()], true, true),
        [true]
    );
    drop(old);
    let fresh = f.pending();
    assert_eq!(
        fresh
            .verify_finalized(|| panic!("new challenge has no applied entry"))
            .err(),
        Some(Error::NotApplied)
    );
}

#[test]
fn independent_target_digest_binding_and_observer_are_checked_before_preparation() {
    let f = Fixture::new();
    let mutations: &[fn(&mut FinalPromotionAccountCheckExpectedV1)] = &[
        |e| e.expected_account = e.observer.clone(),
        |e| e.observer = e.expected_account.clone(),
        |e| e.transaction_payload_digest = [0; 32],
        |e| e.control_revision = 0,
        |e| e.control_revision = FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_REVISIONS_V1 + 1,
        |e| e.control_digest = [0; 32],
        |e| e.floor.height = 0,
        |e| e.floor.block_hash = [0; 32],
        |e| e.binding.runtime_handle = "unknown://promotion/primary".into(),
        |e| e.binding.key_handle = "pkcs11:invalid key".into(),
        |e| e.binding.service_id = e.binding.administrator_id.clone(),
        |e| e.binding.role = SignerRoleV1::FinalPromotionProvenance,
        |e| {
            e.binding.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                deployment_id: DEPLOYMENT.into(),
            }
        },
        |e| e.binding.public_key = key(3).public_key().clone(),
    ];
    for mutate in mutations {
        let mut expected = f.expected();
        mutate(&mut expected);
        assert_eq!(
            begin_final_promotion_account_check_v1(
                Arc::clone(&f.state),
                expected,
                Duration::from_secs(60)
            )
            .err(),
            Some(Error::Invalid)
        );
    }
    for duration in [Duration::ZERO, Duration::from_millis(60_001)] {
        assert_eq!(
            begin_final_promotion_account_check_v1(Arc::clone(&f.state), f.expected(), duration)
                .err(),
            Some(Error::Invalid)
        );
    }
}

#[test]
fn exact_reviewed_target_and_payload_commitment_cannot_be_replaced_in_signed_check() {
    let f = Fixture::new();
    for change_target in [true, false] {
        let prepared = f.prepared();
        let mut changed = prepared.instruction().clone();
        let FinalPromotionAccountCustodyActionV1::Check(check) = &mut changed.action else {
            panic!("Check");
        };
        if change_target {
            check.expected_account = AccountId::new(key(3).public_key().clone());
        } else {
            check.transaction_payload_digest = [32; 32];
        }
        let signed = f.sign(changed.into(), 2, NOW);
        assert_eq!(
            prepared.bind_signed_transaction(signed).err(),
            Some(Error::Transaction)
        );
    }
}

#[test]
fn successful_check_rechecks_both_accounts_permissions_at_same_or_descendant_cut() {
    for same_block in [true, false] {
        for target_permission in [true, false] {
            let mut f = Fixture::new();
            let pending = f.pending();
            let (revoke, seed): (InstructionBox, u8) = if target_permission {
                (
                    Revoke::account_permission(
                        CanOperateSorafsFinalPromotion {
                            deployment_id: DEPLOYMENT.into(),
                        },
                        AccountId::new(key(4).public_key().clone()),
                    )
                    .into(),
                    4,
                )
            } else {
                (
                    Revoke::account_permission(
                        CanCheckSorafsFinalPromotionAccountCustody {
                            deployment_id: DEPLOYMENT.into(),
                        },
                        AccountId::new(key(2).public_key().clone()),
                    )
                    .into(),
                    2,
                )
            };
            let revoke = f.sign(revoke, seed, NOW);
            let mut transactions = vec![pending.signed_transaction().clone()];
            if same_block {
                transactions.push(revoke.clone());
            }
            assert!(
                f.commit(NOW, transactions, true, true)
                    .iter()
                    .all(|result| *result)
            );
            if !same_block {
                assert_eq!(f.commit(NOW + 1, vec![revoke], true, true), [true]);
            }
            assert_eq!(
                pending
                    .verify_finalized(|| Ok(interval(NOW + 1, NOW + 1)))
                    .err(),
                Some(Error::Authority)
            );
        }
    }
}

#[test]
fn account_interval_requires_both_endpoints_after_native_enrollment_execution() {
    for (time, should_pass) in [
        (interval(2_000, 62_000), true),
        (interval(1_999, NOW), false),
        (interval(1_600, 1_600), false),
        (interval(NOW, 100_000), false),
    ] {
        // Attestation is signed at1500 but only enrolled at2000: signed issuance does not
        // authorize observation before the actual native custody transition.
        let mut f = Fixture::with_enrollment_time(2_000);
        let pending = f.pending();
        assert_eq!(
            f.commit(NOW, vec![pending.signed_transaction().clone()], true, true),
            [true]
        );
        let samples = std::cell::Cell::new(0);
        let result = pending.verify_finalized(|| {
            samples.set(samples.get() + 1);
            Ok(time)
        });
        assert_eq!(samples.get(), 1);
        if should_pass {
            let mut verified = result.unwrap();
            assert_eq!(verified.eligibility_time_interval(), time);
            assert_eq!(verified.snapshot(), &f.snapshot());
            verified.ensure_live().unwrap();
            verified.round.expire_for_test();
            assert_eq!(verified.ensure_live(), Err(Error::Expired));
        } else {
            assert_eq!(result.err(), Some(Error::Authority));
        }
    }
}

#[test]
fn prepared_account_liveness_keeps_original_challenge_and_gates_expired_runtime_work() {
    let f = Fixture::new();
    let mut prepared = f.prepared();
    let instruction = prepared.instruction().clone();
    let signed = f.sign(prepared_instruction(&prepared), 2, NOW);
    let callbacks = std::cell::Cell::new(0_u32);
    let invoke = |prepared: &PreparedFinalPromotionAccountCheckV1| -> Result<(), Error> {
        prepared.ensure_live()?;
        callbacks.set(callbacks.get() + 1);
        Ok(())
    };
    prepared.ensure_live().unwrap();
    invoke(&prepared).unwrap();
    assert_eq!(callbacks.get(), 1);
    assert_eq!(prepared.instruction(), &instruction);
    prepared.round.expire_for_test();
    for _ in 0..2 {
        assert_eq!(prepared.ensure_live(), Err(Error::Expired));
        assert_eq!(invoke(&prepared), Err(Error::Expired));
    }
    assert_eq!(callbacks.get(), 1);
    assert_eq!(prepared.instruction(), &instruction);
    assert_eq!(
        prepared.bind_signed_transaction(signed).err(),
        Some(Error::Expired)
    );
}

#[test]
fn prepared_account_observer_is_pinned_before_signing_and_preserved_through_finality() {
    let mut f = Fixture::new();
    let expected_observer = f.expected().observer;
    let mut foreign_expected = f.expected();
    foreign_expected.observer = AccountId::new(key(9).public_key().clone());
    let foreign = begin_final_promotion_account_check_v1(
        Arc::clone(&f.state),
        foreign_expected,
        Duration::from_secs(60),
    )
    .unwrap();
    let prepared = f.prepared();
    let instruction = prepared.instruction().clone();
    let callbacks = std::cell::Cell::new(0_u32);
    let sign = |prepared: &PreparedFinalPromotionAccountCheckV1| {
        prepared.ensure_live()?;
        if prepared.observer() != &expected_observer {
            return Err(Error::Authority);
        }
        callbacks.set(callbacks.get() + 1);
        Ok(f.sign(prepared_instruction(prepared), 2, NOW))
    };
    assert_ne!(foreign.observer(), prepared.observer());
    assert_eq!(sign(&foreign).err(), Some(Error::Authority));
    assert_eq!(
        callbacks.get(),
        0,
        "foreign observer reaches no signing callback"
    );
    assert_eq!(prepared.observer(), &expected_observer);
    let signed = sign(&prepared).unwrap();
    assert_eq!(callbacks.get(), 1);
    assert_eq!(prepared.instruction(), &instruction);
    assert_eq!(signed.authority(), &expected_observer);
    let pending = prepared.bind_signed_transaction(signed.clone()).unwrap();
    assert_eq!(f.commit(NOW, vec![signed], true, true), [true]);
    let verified = pending.verify_finalized(|| Ok(interval(NOW, NOW))).unwrap();
    assert_eq!(verified.observer(), &expected_observer);
    assert_eq!(verified.instruction(), &instruction);
    assert_ne!(verified.observer(), foreign.observer());
}

#[test]
fn account_check_preparation_rejects_non_ed25519_observer_before_issuing_a_round() {
    let fixture = Fixture::new();
    let mut expected = fixture.expected();
    let other =
        iroha_crypto::KeyPair::try_from_seed(vec![91; 32], iroha_crypto::Algorithm::Secp256k1)
            .unwrap();
    expected.observer = AccountId::new(other.public_key().clone());
    let height = fixture.state.view().height();
    assert_eq!(
        begin_final_promotion_account_check_v1(
            Arc::clone(&fixture.state),
            expected,
            Duration::from_secs(60)
        )
        .err(),
        Some(Error::Invalid)
    );
    assert_eq!(fixture.state.view().height(), height);
    fixture.prepared().ensure_live().unwrap();
}

#[test]
fn account_verified_check_retains_original_full_floor_after_applied_descendants() {
    let mut f = Fixture::new();
    let original = f.expected().floor;
    let pending = f.pending();
    assert_eq!(
        f.commit(NOW, vec![pending.signed_transaction().clone()], true, true),
        [true]
    );
    f.commit(NOW + 1, Vec::new(), true, true);
    let verified = pending
        .verify_finalized(|| Ok(interval(NOW + 1, NOW + 1)))
        .unwrap();
    assert_eq!(verified.original_floor(), original);
    assert_eq!(verified.original_floor().height, 2);
    assert_eq!(verified.check_height(), 3);
    assert_eq!(verified.applied_floor().height, 4);
    assert_ne!(verified.original_floor(), verified.applied_floor());
    verified
        .recheck_use_interval(interval(NOW + 2, NOW + 2))
        .unwrap();
    assert_eq!(verified.original_floor(), original);
    verified.ensure_live().unwrap();
}

#[test]
fn account_verified_check_retains_exact_external_and_check_block_after_descendants() {
    let mut f = Fixture::new();
    let pending = f.pending();
    let signed = pending.signed_transaction().clone();
    let exact = norito::encode_canonical(&TransactionEntrypoint::External(signed.clone())).unwrap();
    assert_eq!(f.commit(NOW, vec![signed], true, true), [true]);
    let check_hash = *f.state.view().latest_block_hash().unwrap().as_ref();
    f.commit(NOW + 1, Vec::new(), true, true);
    let verified = pending
        .verify_finalized(|| Ok(interval(NOW + 1, NOW + 1)))
        .unwrap();
    assert_eq!(verified.canonical_external(), exact);
    assert_eq!(verified.check_height(), 3);
    assert_eq!(verified.check_block_hash(), check_hash);
    assert_ne!(
        verified.check_block_hash(),
        verified.applied_floor().block_hash
    );
    verified
        .recheck_use_interval(interval(NOW + 2, NOW + 2))
        .unwrap();
    assert_eq!(verified.canonical_external(), exact);
    assert_eq!(verified.check_block_hash(), check_hash);
    verified.ensure_live().unwrap();
}
