//! Native Check consumption using actual typed execution and real three-of-four BLS/RS16 fixtures.
//! These local fixtures do not establish replicated consensus, application roots or deployed signer authority.

use super::*;
use crate::query::signer_check::bounded_entry;
use iroha_crypto::{Algorithm, Hash, SignatureOf};
use iroha_data_model::isi::InstructionBox;
use iroha_data_model::{
    isi::Revoke, sorafs::final_promotion_authority::FinalPromotionRevocationV1,
    transaction::TransactionSignature,
};
use iroha_executor_data_model::permission::sorafs::CanOperateSorafsFinalPromotion;
use sorafs_manifest::signer::{
    final_promotion::SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1,
    protocol::{SignerKeyAlgorithmV1, SignerRoleV1},
};

mod fixture;
mod recheck_interval;
mod time_interval;
use fixture::{DEPLOYMENT, Fixture, NOW, key};

fn interval(earliest_unix_ms: u64, latest_unix_ms: u64) -> FinalPromotionEligibilityTimeIntervalV1 {
    FinalPromotionEligibilityTimeIntervalV1 {
        earliest_unix_ms,
        latest_unix_ms,
    }
}

#[test]
fn exact_executed_check_joins_real_finality_and_current_native_authority() {
    let mut f = Fixture::new();
    let pending = f.pending();
    let signed = pending.signed_transaction().clone();
    let expected_hash = signed.hash_as_entrypoint();
    assert_eq!(f.commit(NOW, vec![signed], true, true), [true]);
    let verified = pending
        .verify_finalized(FinalPromotionCheckSourceV1::Current, || {
            Ok(interval(NOW, NOW))
        })
        .unwrap();
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
        verified.expected_operator(),
        &AccountId::new(key(2).public_key().clone())
    );
    assert_eq!(verified.entry_hash(), expected_hash);
    assert_eq!(verified.eligibility_time_interval(), interval(NOW, NOW));
    assert_eq!(verified.snapshot(), &f.snapshot());
    assert!(
        matches!(&verified.instruction().action, FinalPromotionAuthorityActionV1::Check(check) if check.challenge != [0; 32])
    );
    verified.ensure_live().unwrap();
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
        .verify_finalized(FinalPromotionCheckSourceV1::Current, || {
            Ok(interval(NOW + 1, NOW + 1))
        })
        .unwrap();
    assert_eq!(verified.check_height(), 3);
    assert_eq!(verified.applied_floor().height, 4);
    assert_eq!(verified.snapshot().custody_anchor.height, 4);
}

#[test]
fn successful_check_cannot_hide_later_same_block_custody_revocation() {
    let mut f = Fixture::new();
    let pending = f.pending();
    let revoke = f.instruction(FinalPromotionAuthorityActionV1::Revoke(
        FinalPromotionRevocationV1 {
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
        pending
            .verify_finalized(FinalPromotionCheckSourceV1::Current, || Ok(interval(
                NOW, NOW
            )))
            .err(),
        Some(Error::Authority)
    );
}

#[test]
fn successful_check_cannot_hide_same_block_or_descendant_permission_revocation() {
    for observer in [false, true] {
        for same_block in [true, false] {
            let mut f = Fixture::new();
            let pending = f.pending();
            let seed = if observer { 3 } else { 2 };
            let permission: iroha_data_model::permission::Permission = if observer {
                iroha_executor_data_model::permission::sorafs::CanCheckSorafsFinalPromotion {
                    deployment_id: DEPLOYMENT.into(),
                }
                .into()
            } else {
                CanOperateSorafsFinalPromotion {
                    deployment_id: DEPLOYMENT.into(),
                }
                .into()
            };
            let revoke = Revoke::account_permission(
                permission,
                AccountId::new(key(seed).public_key().clone()),
            )
            .into();
            let revoke = f.sign(revoke, seed, NOW);
            let mut transactions = vec![pending.signed_transaction().clone()];
            if same_block {
                transactions.push(revoke.clone());
            }
            assert!(
                f.commit(NOW, transactions, true, true)
                    .iter()
                    .all(|outcome| *outcome)
            );
            if !same_block {
                assert_eq!(f.commit(NOW + 1, vec![revoke], true, true), [true]);
            }
            assert_eq!(
                pending
                    .verify_finalized(FinalPromotionCheckSourceV1::Current, || Ok(interval(
                        NOW + 1,
                        NOW + 1
                    )))
                    .err(),
                Some(Error::Authority)
            );
        }
    }
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
            .verify_finalized(FinalPromotionCheckSourceV1::Current, || panic!(
                "rejected result must precede clock"
            ))
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
            .verify_finalized(FinalPromotionCheckSourceV1::Current, || panic!(
                "unapplied member must precede clock"
            ))
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
            .verify_finalized(FinalPromotionCheckSourceV1::Current, || panic!(
                "missing finality must precede clock"
            ))
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
        let prepared =
            begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
                .unwrap();
        let signed = f.sign(prepared_instruction(&prepared), 3, NOW);
        let pending = prepared.bind_signed_transaction(signed).unwrap();
        let results = f.commit(NOW, vec![pending.signed_transaction().clone()], true, true);
        assert_eq!(results, [!change_hash]);
        assert_eq!(
            pending
                .verify_finalized(FinalPromotionCheckSourceV1::Current, || panic!(
                    "foreign floor must precede clock"
                ))
                .err(),
            Some(Error::Finality)
        );
    }
}

fn prepared_instruction(prepared: &PreparedFinalPromotionCheckV1) -> InstructionBox {
    prepared.instruction().clone().into()
}

#[test]
fn each_round_has_fresh_entropy_and_signed_envelopes_cannot_be_replaced() {
    let f = Fixture::new();
    let first = f.prepared();
    let second = f.prepared();
    assert_ne!(first.instruction(), second.instruction());
    let signed_other = f.sign(prepared_instruction(&second), 3, NOW);
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
    let mut signed = f.sign(prepared_instruction(&prepared), 3, NOW);
    let original_hash = signed.hash_as_entrypoint();
    signed.set_signature(TransactionSignature(SignatureOf::from_signature(
        iroha_crypto::Signature::try_new(key(9).private_key(), b"wrong signed bytes").unwrap(),
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
fn canonical_entry_comparison_includes_authorization_bytes() {
    let f = Fixture::new();
    let prepared = f.prepared();
    let signed = f.sign(prepared_instruction(&prepared), 3, NOW);
    let original = bounded_entry(&TransactionEntrypoint::External(signed.clone())).unwrap();
    let mut altered = signed.clone();
    altered.set_signature(TransactionSignature(SignatureOf::from_signature(
        iroha_crypto::Signature::try_new(key(9).private_key(), b"different authorization").unwrap(),
    )));
    assert_eq!(altered.hash_as_entrypoint(), signed.hash_as_entrypoint());
    assert_ne!(
        bounded_entry(&TransactionEntrypoint::External(altered)).unwrap(),
        original
    );
}

#[test]
fn invalid_independent_coordinates_and_duration_fail_before_state_io() {
    let f = Fixture::new();
    for duration in [
        Duration::ZERO,
        Duration::from_millis(FINAL_PROMOTION_RESERVATION_MS_V1 + 1),
    ] {
        assert_eq!(
            begin_final_promotion_check_v1(Arc::clone(&f.state), f.expected(), duration).err(),
            Some(Error::Invalid)
        );
    }
    let mutations: &[fn(&mut FinalPromotionCheckExpectedV1)] = &[
        |expected: &mut FinalPromotionCheckExpectedV1| expected.floor.height = 0,
        |e| e.floor.block_hash = [0; 32],
        |e| e.control_revision = 0,
        |e| e.control_revision = FINAL_PROMOTION_CUSTODY_MAX_REVISIONS_V1 + 1,
        |e| e.control_digest = [0; 32],
        |e| e.request.operation_id = [0; 32],
        |e| e.request.statement_size = 0,
        |e| e.request.statement_size = SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1 as u64 + 1,
        |e| e.binding.network_id = [0; 32],
        |e| e.binding.role = SignerRoleV1::StreamToken,
        |e| {
            e.binding.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                deployment_id: "invalid deployment".into(),
            }
        },
    ];
    for mutate in mutations {
        let mut expected = f.expected();
        mutate(&mut expected);
        assert_eq!(
            begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
                .err(),
            Some(Error::Invalid)
        );
    }
}

#[test]
fn signing_and_terminal_verification_never_reset_the_one_use_interval() {
    let mut f = Fixture::new();
    let mut prepared = f.prepared();
    let signed = f.sign(prepared_instruction(&prepared), 3, NOW);
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
            .verify_finalized(FinalPromotionCheckSourceV1::Current, || panic!(
                "expiry must precede clock"
            ))
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
        assert_eq!(
            pending
                .verify_finalized(FinalPromotionCheckSourceV1::Current, || now)
                .err(),
            Some(Error::Clock)
        );
    }
}

#[test]
fn current_eligibility_time_rechecks_expired_custody_after_successful_execution() {
    let mut f = Fixture::new();
    let pending = f.pending();
    assert_eq!(
        f.commit(NOW, vec![pending.signed_transaction().clone()], true, true),
        [true]
    );
    assert_eq!(
        pending
            .verify_finalized(FinalPromotionCheckSourceV1::Current, || Ok(interval(
                100_000, 100_000
            )))
            .err(),
        Some(Error::Authority)
    );
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
            .verify_finalized(FinalPromotionCheckSourceV1::Current, || panic!(
                "new challenge has no applied entry"
            ))
            .err(),
        Some(Error::NotApplied)
    );
}

#[test]
fn malformed_current_audit_and_software_bindings_fail_before_preparation() {
    use sorafs_manifest::signer::protocol::SignerOperationAuditHeadV1;
    let f = Fixture::new();
    for audit in [
        SignerOperationAuditHeadV1 {
            sequence: 0,
            digest: [1; 32],
        },
        SignerOperationAuditHeadV1 {
            sequence: 1,
            digest: [0; 32],
        },
        SignerOperationAuditHeadV1 {
            sequence: FINAL_PROMOTION_MAX_OPERATIONS_V1 + 1,
            digest: [1; 32],
        },
    ] {
        let mut expected = f.expected();
        expected.subject = FinalPromotionCheckSubjectV1::Current(audit);
        assert_eq!(
            begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
                .err(),
            Some(Error::Invalid)
        );
    }
    let mutations: &[fn(&mut SignerCustodyBindingV1)] = &[
        |binding: &mut SignerCustodyBindingV1| {
            binding.runtime_handle = "software://promotion/primary".into()
        },
        |binding| binding.key_handle = "pkcs11:promotion/invalid key".into(),
        |binding| binding.service_id = binding.administrator_id.clone(),
        |binding| binding.administrator_id = "invalid identity".into(),
    ];
    for mutate in mutations {
        let mut expected = f.expected();
        mutate(&mut expected.binding);
        assert_eq!(
            begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
                .err(),
            Some(Error::Invalid)
        );
    }
}

fn native_operation_row(
    f: &Fixture,
) -> iroha_data_model::sorafs::final_promotion_authority::FinalPromotionOperationRecordV1 {
    crate::query::final_promotion_authority::read_final_promotion_authority_at_v1(
        &f.state.view(),
        &f.policy.binding,
        f.state.view().height() as u64,
        Some([31; 32]),
    )
    .unwrap()
    .unwrap()
    .operation
    .unwrap()
}

fn reserve_reviewed_request(
    f: &mut Fixture,
) -> iroha_data_model::sorafs::final_promotion_authority::FinalPromotionOperationRecordV1 {
    reserve_reviewed_request_with_availability(f, true, true)
}

fn reserve_reviewed_request_with_availability(
    f: &mut Fixture,
    membership: bool,
    finality: bool,
) -> iroha_data_model::sorafs::final_promotion_authority::FinalPromotionOperationRecordV1 {
    use iroha_data_model::sorafs::final_promotion_authority::FinalPromotionReserveV1;
    use sorafs_manifest::signer::protocol::SignerOperationIntentV1;
    let expected = f.expected();
    let pre_reserve_floor = expected.floor;
    let reserve = f.instruction(FinalPromotionAuthorityActionV1::Reserve(
        FinalPromotionReserveV1 {
            intent: SignerOperationIntentV1 {
                action: SignerOperationActionV1::Sign,
                operation_id: expected.request.operation_id,
                request_digest: expected.request.digest().unwrap(),
                previous_audit: f.snapshot().operations.audit,
            },
            custody: expected.request.original_custody,
        },
    ));
    let signed = f.sign(reserve.into(), 2, NOW);
    assert_eq!(
        f.commit(NOW, vec![signed.clone()], membership, finality),
        [true]
    );
    f.reserve_signed = Some(signed);
    f.reserve_floor = Some(pre_reserve_floor);
    native_operation_row(f)
}

#[test]
fn same_block_reserve_retry_cannot_replace_the_allocating_signed_entry() {
    use iroha_data_model::sorafs::final_promotion_authority::FinalPromotionReserveV1;
    use sorafs_manifest::signer::protocol::SignerOperationIntentV1;

    let mut f = Fixture::new();
    let expected = f.expected();
    let reserve = f.instruction(FinalPromotionAuthorityActionV1::Reserve(
        FinalPromotionReserveV1 {
            intent: SignerOperationIntentV1 {
                action: SignerOperationActionV1::Sign,
                operation_id: expected.request.operation_id,
                request_digest: expected.request.digest().unwrap(),
                previous_audit: f.snapshot().operations.audit,
            },
            custody: expected.request.original_custody,
        },
    ));
    let allocating = f.sign(reserve.clone().into(), 2, NOW - 1);
    let retry = f.sign(reserve.into(), 2, NOW);
    assert_ne!(allocating.hash_as_entrypoint(), retry.hash_as_entrypoint());
    let allocating_hash = *allocating.hash_as_entrypoint().as_ref();
    let retry_hash = *retry.hash_as_entrypoint().as_ref();
    assert_eq!(
        f.commit(NOW, vec![allocating.clone(), retry.clone()], true, true),
        [true, false]
    );

    let row = native_operation_row(&f);
    assert_eq!(row.revision, 1);
    assert_eq!(row.execution_origin, Some(row.reserved_origin));
    assert_eq!(row.reserved_origin.entry_hash, allocating_hash);
    assert_eq!(row.reserved_origin.entry_index, 0);
    assert_ne!(row.reserved_origin.entry_hash, retry_hash);

    // The second signed envelope cannot claim idempotent success for the first row.
    let mut valid = f.expected();
    valid.floor = expected.floor;
    valid.subject = FinalPromotionCheckSubjectV1::BeforeProvider(row.clone());
    let valid =
        begin_final_promotion_check_v1(Arc::clone(&f.state), valid, Duration::from_secs(60))
            .unwrap();
    let mut substituted_source = f.expected();
    substituted_source.floor = expected.floor;
    substituted_source.subject = FinalPromotionCheckSubjectV1::BeforeProvider(row.clone());
    let substituted_source = begin_final_promotion_check_v1(
        Arc::clone(&f.state),
        substituted_source,
        Duration::from_secs(60),
    )
    .unwrap();
    let mut substituted = row;
    substituted.reserved_origin.entry_hash = retry_hash;
    substituted.reserved_origin.entry_index = 1;
    substituted.execution_origin = Some(substituted.reserved_origin);
    let mut wrong = f.expected();
    wrong.floor = expected.floor;
    wrong.subject = FinalPromotionCheckSubjectV1::BeforeProvider(substituted);
    let wrong =
        begin_final_promotion_check_v1(Arc::clone(&f.state), wrong, Duration::from_secs(60))
            .unwrap();
    let valid_signed = f.sign(valid.instruction().clone().into(), 3, NOW + 1);
    let valid = valid.bind_signed_transaction(valid_signed).unwrap();
    let substituted_signed = f.sign(substituted_source.instruction().clone().into(), 3, NOW + 1);
    let substituted_source = substituted_source
        .bind_signed_transaction(substituted_signed)
        .unwrap();
    assert_eq!(
        f.commit(
            NOW + 1,
            vec![
                valid.signed_transaction().clone(),
                substituted_source.signed_transaction().clone(),
                f.sign(wrong.instruction().clone().into(), 3, NOW + 1),
            ],
            true,
            true,
        ),
        [true, true, false]
    );
    assert!(
        valid
            .verify_finalized(FinalPromotionCheckSourceV1::Reserved(&allocating), || {
                Ok(interval(NOW + 1, NOW + 1))
            })
            .is_ok()
    );
    assert_eq!(
        substituted_source
            .verify_finalized(FinalPromotionCheckSourceV1::Reserved(&retry), || {
                panic!("substituted source must precede clock")
            })
            .err(),
        Some(Error::Execution)
    );
}

#[test]
fn reserved_check_rejects_floor_selected_after_its_reserve() {
    let mut f = Fixture::new();
    let row = reserve_reviewed_request(&mut f);
    let mut expected = f.expected();
    assert_eq!(expected.floor.height, row.reserved.height);
    expected.subject = FinalPromotionCheckSubjectV1::BeforeProvider(row);
    let prepared =
        begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
            .unwrap();
    let signed = f.sign(prepared_instruction(&prepared), 3, NOW + 1);
    let pending = prepared.bind_signed_transaction(signed).unwrap();
    assert_eq!(
        f.commit(
            NOW + 1,
            vec![pending.signed_transaction().clone()],
            true,
            true,
        ),
        [true]
    );
    assert_eq!(
        pending
            .verify_finalized(
                FinalPromotionCheckSourceV1::Reserved(f.reserve_signed.as_ref().unwrap()),
                || panic!("post-Reserve floor must precede clock"),
            )
            .err(),
        Some(Error::Execution)
    );
}

#[test]
fn reserved_check_requires_original_source_membership_and_durable_reserve_finality() {
    for (membership, finality, expected_error) in [
        (false, true, Error::Execution),
        (true, false, Error::Finality),
    ] {
        let mut f = Fixture::new();
        let row = reserve_reviewed_request_with_availability(&mut f, membership, finality);
        let mut expected = f.expected();
        expected.floor = f.reserve_floor.expect("pre-Reserve independent floor");
        expected.subject = FinalPromotionCheckSubjectV1::BeforeProvider(row);
        let prepared =
            begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
                .unwrap();
        let signed = f.sign(prepared_instruction(&prepared), 3, NOW + 1);
        let pending = prepared.bind_signed_transaction(signed).unwrap();
        assert_eq!(
            f.commit(
                NOW + 1,
                vec![pending.signed_transaction().clone()],
                true,
                true,
            ),
            [true]
        );
        assert_eq!(
            pending
                .verify_finalized(
                    FinalPromotionCheckSourceV1::Reserved(f.reserve_signed.as_ref().unwrap()),
                    || panic!("missing Reserve evidence must precede clock"),
                )
                .err(),
            Some(expected_error),
            "Reserve membership={membership}, finality={finality}"
        );
    }
}

#[test]
fn reserved_phase_preflight_accepts_exact_native_rows_and_rejects_substitutions() {
    use iroha_data_model::sorafs::final_promotion_authority::FinalPromotionOperationRecordV1;
    let mut f = Fixture::new();
    let row = reserve_reviewed_request(&mut f);
    for subject in [
        FinalPromotionCheckSubjectV1::BeforeProvider(row.clone()),
        FinalPromotionCheckSubjectV1::AfterProvider(row.clone()),
        FinalPromotionCheckSubjectV1::BeforeCommit(row.clone()),
    ] {
        let mut expected = f.expected();
        expected.subject = subject;
        assert!(
            begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
                .is_ok()
        );
    }
    let mutations: &[fn(&mut FinalPromotionOperationRecordV1)] = &[
        |r: &mut FinalPromotionOperationRecordV1| {
            r.outcome = FinalPromotionOperationOutcomeV1::Expired
        },
        |r| r.deployment_id = "different-deployment".into(),
        |r| r.deployment_id = "x".repeat(FINAL_PROMOTION_MAX_RECORD_BYTES_V1 + 1),
        |r| r.intent.operation_id = [0x91; 32],
        |r| r.intent.request_digest = [0x92; 32],
        |r| r.custody.record_digest = [0x93; 32],
        |r| r.reserved.authority = AccountId::new(key(1).public_key().clone()),
        |r| r.revision = 0,
        |r| r.execution.height = 0,
        |r| r.request_digest = [0; 32],
        |r| r.reservation.reservation_id = [0; 32],
        |r| r.reservation.fence = 0,
        |r| r.reservation.expires_at_unix_ms = r.reserved.recorded_at_unix_ms,
        |r| {
            r.reservation.expires_at_unix_ms =
                r.reserved.recorded_at_unix_ms + FINAL_PROMOTION_RESERVATION_MS_V1 + 1
        },
        |r| r.execution.ordinal += 1,
        |r| r.reserved_origin.entry_hash = [0; 32],
        |r| r.execution_origin = None,
        |r| {
            r.execution.ordinal = u32::MAX;
            r.reserved.ordinal = u32::MAX;
        },
    ];
    for mutate in mutations {
        let mut wrong = row.clone();
        mutate(&mut wrong);
        let mut expected = f.expected();
        expected.subject = FinalPromotionCheckSubjectV1::BeforeProvider(wrong);
        assert_eq!(
            begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
                .err(),
            Some(Error::Invalid)
        );
    }
    let mut expected = f.expected();
    expected.subject = FinalPromotionCheckSubjectV1::BeforeRelease(row);
    assert_eq!(
        begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
            .err(),
        Some(Error::Invalid)
    );
}

#[test]
fn completed_phase_preflight_preserves_original_native_time_and_commitments() {
    use iroha_data_model::sorafs::final_promotion_authority::{
        FinalPromotionCompleteV1, FinalPromotionOperationRecordV1,
    };
    use sorafs_manifest::signer::protocol::{
        SignerOperationAuditHeadV1, SignerOperationCommitmentV1,
    };
    let mut f = Fixture::new();
    let reserved = reserve_reviewed_request(&mut f);
    let complete = f.instruction(FinalPromotionAuthorityActionV1::Complete(
        FinalPromotionCompleteV1 {
            intent: reserved.intent,
            custody: reserved.custody,
            reservation: reserved.reservation,
            commitment: SignerOperationCommitmentV1 {
                audit: SignerOperationAuditHeadV1 {
                    sequence: 1,
                    digest: [21; 32],
                },
                response_digest: [22; 32],
            },
            signatures_digest: [23; 32],
        },
    ));
    assert_eq!(
        f.commit(
            NOW + 1,
            vec![f.sign(complete.into(), 2, NOW + 1)],
            true,
            true
        ),
        [true]
    );
    let row = native_operation_row(&f);
    assert_eq!(row.reserved_origin, reserved.reserved_origin);
    assert_ne!(row.execution_origin, Some(row.reserved_origin));
    for subject in [
        FinalPromotionCheckSubjectV1::AfterCommit(row.clone()),
        FinalPromotionCheckSubjectV1::BeforeRelease(row.clone()),
    ] {
        let mut expected = f.expected();
        expected.subject = subject;
        assert!(
            begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
                .is_ok()
        );
    }
    let mutations: &[fn(&mut FinalPromotionOperationRecordV1)] = &[
        |r: &mut FinalPromotionOperationRecordV1| {
            r.outcome = FinalPromotionOperationOutcomeV1::Reserved
        },
        |r| r.execution = r.reserved.clone(),
        |r| r.execution.recorded_at_unix_ms = r.reservation.expires_at_unix_ms,
        |r| r.execution.authority = AccountId::new(key(1).public_key().clone()),
        |r| r.execution_origin = None,
        |r| r.reserved_origin.entry_hash = [0; 32],
        |r| {
            if let FinalPromotionOperationOutcomeV1::Completed(ref mut c) = r.outcome {
                c.commitment.audit.sequence += 1;
            }
        },
        |r| {
            if let FinalPromotionOperationOutcomeV1::Completed(ref mut c) = r.outcome {
                c.commitment.audit.digest = [0; 32];
            }
        },
        |r| {
            if let FinalPromotionOperationOutcomeV1::Completed(ref mut c) = r.outcome {
                c.commitment.response_digest = [0; 32];
            }
        },
        |r| {
            if let FinalPromotionOperationOutcomeV1::Completed(ref mut c) = r.outcome {
                c.signatures_digest = [0; 32];
            }
        },
    ];
    for mutate in mutations {
        let mut wrong = row.clone();
        mutate(&mut wrong);
        let mut expected = f.expected();
        expected.subject = FinalPromotionCheckSubjectV1::BeforeRelease(wrong);
        assert_eq!(
            begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
                .err(),
            Some(Error::Invalid)
        );
    }
}

mod prepared_access;

mod observer;

#[test]
fn receipt_check_preparation_rejects_non_ed25519_observer_before_issuing_a_round() {
    let fixture = Fixture::new();
    let mut expected = fixture.expected();
    let other =
        iroha_crypto::KeyPair::try_from_seed(vec![91; 32], iroha_crypto::Algorithm::Secp256k1)
            .unwrap();
    expected.observer = AccountId::new(other.public_key().clone());
    let height = fixture.state.view().height();
    assert_eq!(
        begin_final_promotion_check_v1(
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
fn receipt_verified_check_retains_original_full_floor_after_applied_descendants() {
    let mut f = Fixture::new();
    let original = f.expected().floor;
    let pending = f.pending();
    assert_eq!(
        f.commit(NOW, vec![pending.signed_transaction().clone()], true, true),
        [true]
    );
    f.commit(NOW + 1, Vec::new(), true, true);
    let verified = pending
        .verify_finalized(FinalPromotionCheckSourceV1::Current, || {
            Ok(interval(NOW + 1, NOW + 1))
        })
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
fn receipt_verified_check_retains_exact_external_and_check_block_after_descendants() {
    let mut f = Fixture::new();
    let pending = f.pending();
    let signed = pending.signed_transaction().clone();
    let exact = norito::encode_canonical(&TransactionEntrypoint::External(signed.clone())).unwrap();
    assert_eq!(f.commit(NOW, vec![signed], true, true), [true]);
    let check_hash = *f.state.view().latest_block_hash().unwrap().as_ref();
    f.commit(NOW + 1, Vec::new(), true, true);
    let verified = pending
        .verify_finalized(FinalPromotionCheckSourceV1::Current, || {
            Ok(interval(NOW + 1, NOW + 1))
        })
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

fn complete_reviewed_request_with_availability(
    f: &mut Fixture,
    membership: bool,
    finality: bool,
) -> (
    SignedTransaction,
    iroha_data_model::sorafs::final_promotion_authority::FinalPromotionOperationRecordV1,
    MutateSorafsFinalPromotionAuthority,
) {
    use iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCompleteV1;
    use sorafs_manifest::signer::protocol::{
        SignerOperationAuditHeadV1, SignerOperationCommitmentV1,
    };
    let reserved = reserve_reviewed_request(f);
    let instruction = f.instruction(FinalPromotionAuthorityActionV1::Complete(
        FinalPromotionCompleteV1 {
            intent: reserved.intent,
            custody: reserved.custody,
            reservation: reserved.reservation,
            commitment: SignerOperationCommitmentV1 {
                audit: SignerOperationAuditHeadV1 {
                    sequence: 1,
                    digest: [21; 32],
                },
                response_digest: [22; 32],
            },
            signatures_digest: [23; 32],
        },
    ));
    let signed = f.sign(instruction.clone().into(), 2, NOW + 1);
    assert_eq!(
        f.commit(NOW + 1, vec![signed.clone()], membership, finality),
        [true]
    );
    (signed, native_operation_row(f), instruction)
}

fn pending_completed_check(
    f: &Fixture,
    subject: FinalPromotionCheckSubjectV1,
) -> PendingFinalPromotionCheckV1 {
    let mut expected = f.expected();
    expected.floor = f
        .reserve_floor
        .expect("independently pinned pre-Reserve floor");
    expected.subject = subject;
    let prepared =
        begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
            .unwrap();
    let signed = f.sign(prepared.instruction().clone().into(), 3, NOW + 2);
    prepared.bind_signed_transaction(signed).unwrap()
}

#[test]
fn completed_checks_require_both_original_signed_sources_and_successful_finality() {
    let subjects: [fn(
        iroha_data_model::sorafs::final_promotion_authority::FinalPromotionOperationRecordV1,
    ) -> FinalPromotionCheckSubjectV1; 2] = [
        FinalPromotionCheckSubjectV1::AfterCommit,
        FinalPromotionCheckSubjectV1::BeforeRelease,
    ];
    for subject in subjects {
        let mut f = Fixture::new();
        let (complete, row, _) = complete_reviewed_request_with_availability(&mut f, true, true);
        let pending = pending_completed_check(&f, subject(row.clone()));
        assert_eq!(
            f.commit(
                NOW + 2,
                vec![pending.signed_transaction().clone()],
                true,
                true,
            ),
            [true]
        );
        let verified = pending
            .verify_finalized(
                FinalPromotionCheckSourceV1::Completed {
                    reserve: f.reserve_signed.as_ref().unwrap(),
                    complete: &complete,
                },
                || Ok(interval(NOW + 2, NOW + 2)),
            )
            .unwrap();
        assert_eq!(verified.snapshot().operation.as_ref(), Some(&row));
    }
}

#[test]
fn completed_checks_reject_wrong_role_substituted_and_replayed_sources_before_clock() {
    let mut f = Fixture::new();
    let (complete, row, instruction) =
        complete_reviewed_request_with_availability(&mut f, true, true);
    let pending =
        pending_completed_check(&f, FinalPromotionCheckSubjectV1::AfterCommit(row.clone()));
    let wrong_role = pending.signed_transaction().clone();
    let mut altered_instruction = instruction;
    let FinalPromotionAuthorityActionV1::Complete(ref mut altered) = altered_instruction.action
    else {
        unreachable!("reviewed helper builds Complete");
    };
    altered.signatures_digest[0] ^= 1;
    let substituted_complete = f.sign(altered_instruction.into(), 2, NOW + 2);
    assert_ne!(
        substituted_complete.hash_as_entrypoint(),
        complete.hash_as_entrypoint()
    );
    assert_eq!(
        f.commit(
            NOW + 2,
            vec![
                pending.signed_transaction().clone(),
                substituted_complete.clone()
            ],
            true,
            true,
        ),
        [true, false]
    );
    assert_eq!(
        pending
            .verify_finalized(
                FinalPromotionCheckSourceV1::Completed {
                    reserve: f.reserve_signed.as_ref().unwrap(),
                    complete: &wrong_role,
                },
                || panic!("wrong-role source must precede clock"),
            )
            .err(),
        Some(Error::Execution)
    );

    let pending =
        pending_completed_check(&f, FinalPromotionCheckSubjectV1::BeforeRelease(row.clone()));
    assert_eq!(
        f.commit(
            NOW + 3,
            vec![pending.signed_transaction().clone()],
            true,
            true,
        ),
        [true]
    );
    assert_eq!(
        pending
            .verify_finalized(
                FinalPromotionCheckSourceV1::Completed {
                    reserve: f.reserve_signed.as_ref().unwrap(),
                    complete: &substituted_complete,
                },
                || panic!("rejected Complete retry must precede clock"),
            )
            .err(),
        Some(Error::Execution)
    );

    let pending = pending_completed_check(&f, FinalPromotionCheckSubjectV1::BeforeRelease(row));
    assert_eq!(
        f.commit(
            NOW + 4,
            vec![pending.signed_transaction().clone()],
            true,
            true,
        ),
        [true]
    );
    assert_eq!(
        pending
            .verify_finalized(
                FinalPromotionCheckSourceV1::Completed {
                    reserve: &complete,
                    complete: &complete,
                },
                || panic!("replayed Complete as Reserve must precede clock"),
            )
            .err(),
        Some(Error::Execution)
    );
}

#[test]
fn completed_check_rejects_forked_floor_and_missing_complete_evidence() {
    let mut f = Fixture::new();
    let (complete, row, _) = complete_reviewed_request_with_availability(&mut f, true, true);
    let mut expected = f.expected();
    expected.floor = f.reserve_floor.unwrap();
    expected.floor.block_hash[0] ^= 1;
    expected.subject = FinalPromotionCheckSubjectV1::AfterCommit(row.clone());
    let prepared =
        begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
            .unwrap();
    let signed = f.sign(prepared.instruction().clone().into(), 3, NOW + 2);
    let pending = prepared.bind_signed_transaction(signed).unwrap();
    assert_eq!(
        f.commit(
            NOW + 2,
            vec![pending.signed_transaction().clone()],
            true,
            true
        ),
        [false]
    );
    assert!(
        pending
            .verify_finalized(
                FinalPromotionCheckSourceV1::Completed {
                    reserve: f.reserve_signed.as_ref().unwrap(),
                    complete: &complete,
                },
                || panic!("forked floor must precede clock"),
            )
            .is_err()
    );

    for (membership, finality, expected_error) in [
        (false, true, Error::Execution),
        (true, false, Error::Finality),
    ] {
        let mut f = Fixture::new();
        let (complete, row, _) =
            complete_reviewed_request_with_availability(&mut f, membership, finality);
        let pending = pending_completed_check(&f, FinalPromotionCheckSubjectV1::AfterCommit(row));
        assert_eq!(
            f.commit(
                NOW + 2,
                vec![pending.signed_transaction().clone()],
                true,
                true
            ),
            [true]
        );
        assert_eq!(
            pending
                .verify_finalized(
                    FinalPromotionCheckSourceV1::Completed {
                        reserve: f.reserve_signed.as_ref().unwrap(),
                        complete: &complete,
                    },
                    || panic!("missing Complete evidence must precede clock"),
                )
                .err(),
            Some(expected_error),
            "Complete membership={membership}, finality={finality}"
        );
    }
}
