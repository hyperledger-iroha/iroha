//! Native no-write Check predicates over real custody and operation fixtures.
//! These local execution tests do not authenticate consensus, a pending challenge, or hardware.
use super::*;
use crate::query::final_promotion_authority::{check as eligibility, operation::operation_digest};
use iroha_data_model::{
    isi::{Register, Revoke, Unregister},
    role::{Role, RoleId},
    sorafs::final_promotion_authority::{
        FINAL_PROMOTION_MAX_RECORD_BYTES_V1, FinalPromotionCheckSubjectV1 as Subject,
        FinalPromotionCheckV1,
    },
};
use sorafs_manifest::signer::final_promotion::{
    SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1, SignerFinalPromotionExpectedV1,
    SignerFinalPromotionRequestV1, signer_final_promotion_digest_v1,
    statement::prepare_final_promotion_statement_v1,
};

mod statement_fixture {
    use sorafs_manifest as manifest;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../sorafs_manifest/src/signer/final_promotion/tests/statement_fixture_support.rs"
    ));
}

fn enrolled() -> Fixture {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    f
}

fn verified(f: &Fixture, now: u64) -> VerifiedSignerCustodyV1 {
    let current = snapshot(f, None);
    verify_signer_custody_use_v1(
        current.control_record.enrollment.as_deref().unwrap(),
        &f.policy.binding,
        &f.policy.custody_trust(),
        &SignerCustodyUseContextV1 {
            now_unix_ms: now,
            anchor_observed_at_unix_ms: now,
            current_anchor: current.custody_anchor,
            active_head: current.control.active_head.unwrap(),
            signer_revoked: current.control.signer_revoked,
            attester_revoked: current.control.attester_revoked,
        },
    )
    .expect("independently signed native enrollment is currently eligible")
}

fn request(f: &Fixture, id: u8) -> SignerFinalPromotionRequestV1 {
    let message = statement_fixture::statement_message(&f.policy.binding);
    let prepared = prepare_final_promotion_statement_v1(&message, &f.policy.binding).unwrap();
    SignerFinalPromotionRequestV1::new(
        &verified(f, 1_600),
        &SignerFinalPromotionExpectedV1 {
            operation_id: [id; 32],
            statement_digest: signer_final_promotion_digest_v1(&message),
            statement_size: message.len().try_into().unwrap(),
        },
        &prepared,
    )
    .expect("real prepared statement and verified native custody")
}

fn check_instruction(
    f: &Fixture,
    request: SignerFinalPromotionRequestV1,
    subject: Subject,
) -> MutateSorafsFinalPromotionAuthority {
    let current = snapshot(f, None);
    MutateSorafsFinalPromotionAuthority {
        deployment_id: DEPLOYMENT.into(),
        expected_control_revision: current.control_record.revision,
        expected_control_digest: current.custody_anchor.state_digest,
        action: Action::Check(FinalPromotionCheckV1 {
            challenge: [90; 32],
            network_id: f.policy.binding.network_id,
            expected_operator: f.operator.clone(),
            minimum_height: current.custody_anchor.height,
            minimum_block_hash: current.custody_anchor.block_hash,
            request,
            subject,
        }),
    }
}

fn payload(instruction: &MutateSorafsFinalPromotionAuthority) -> &FinalPromotionCheckV1 {
    let Action::Check(check) = &instruction.action else {
        panic!("Check fixture")
    };
    check
}

fn payload_mut(
    instruction: &mut MutateSorafsFinalPromotionAuthority,
) -> &mut FinalPromotionCheckV1 {
    let Action::Check(check) = &mut instruction.action else {
        panic!("Check fixture")
    };
    check
}

fn reserve_exact(
    f: &mut Fixture,
    request: SignerFinalPromotionRequestV1,
    now: u64,
) -> FinalPromotionOperationRecordV1 {
    let reserve = FinalPromotionReserveV1 {
        intent: SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: request.operation_id,
            request_digest: request.digest().unwrap(),
            previous_audit: snapshot(f, None).operations.audit,
        },
        custody: request.original_custody,
    };
    transact(&mut f.state, now, |tx| {
        instruction(tx, Action::Reserve(reserve))
            .execute(&f.operator, tx)
            .expect("reserve exact prepared request")
    });
    snapshot(f, Some(request.operation_id)).operation.unwrap()
}

fn assert_no_writes(
    tx: &mut StateTransaction<'_, '_>,
    check: &MutateSorafsFinalPromotionAuthority,
    authority: &AccountId,
    allowed: bool,
) {
    let before = retained(tx);
    assert_eq!(check.clone().execute(authority, tx).is_ok(), allowed);
    assert_eq!(retained(tx), before, "Check must not publish native state");
}

fn applied(
    f: &Fixture,
    check: &MutateSorafsFinalPromotionAuthority,
    now: u64,
) -> Result<FinalPromotionAuthoritySnapshotV1, Error> {
    eligibility::check_applied_snapshot_v1(
        &f.state.view(),
        check,
        &f.policy.binding,
        &f.observer,
        now,
    )
}

#[test]
fn current_check_repeats_without_consuming_ids_fences_audit_or_history() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let before = snapshot(&f, None);
    let check = check_instruction(&f, request, Subject::Current(before.operations.audit));
    assert!(applied(&f, &check, 1_700).is_err(), "cut must follow floor");
    transact(&mut f.state, 1_700, |tx| {
        for _ in 0..3 {
            assert_no_writes(tx, &check, &f.observer, true);
        }
        assert_no_writes(tx, &check, &f.manager, false);
        assert_no_writes(tx, &check, &f.other, false);
        let mut fresh_challenge = check.clone();
        payload_mut(&mut fresh_challenge).challenge = [91; 32];
        assert_no_writes(tx, &fresh_challenge, &f.observer, true);
    });
    let after = applied(&f, &check, 1_700).unwrap();
    assert_eq!(after.control_record, before.control_record);
    assert_eq!(after.operations, before.operations);
    assert_eq!(after.operation, None);
    let reserved = reserve_exact(&mut f, request, 2_000);
    assert_eq!(reserved.reservation.fence, 1);
    assert_eq!(snapshot(&f, None).operations.total_admissions, 1);
    transact(&mut f.state, 2_100, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
    });
    assert_eq!(
        applied(&f, &check, 2_100).unwrap().operation,
        Some(reserved)
    );
}

#[test]
fn check_rejects_floor_network_challenge_control_and_request_substitutions() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let row = reserve_exact(&mut f, request, 2_000);
    let base = check_instruction(&f, request, Subject::BeforeProvider(row));
    transact(&mut f.state, 2_100, |tx| {
        assert_no_writes(tx, &base, &f.observer, true);
        for field in 0..20 {
            let mut changed = base.clone();
            match field {
                0 => payload_mut(&mut changed).challenge = [0; 32],
                1 => payload_mut(&mut changed).network_id = [0; 32],
                2 => payload_mut(&mut changed).network_id[0] ^= 1,
                3 => payload_mut(&mut changed).minimum_height = 0,
                4 => payload_mut(&mut changed).minimum_height += 1,
                5 => payload_mut(&mut changed).minimum_height = u64::MAX,
                6 => payload_mut(&mut changed).minimum_block_hash = [0; 32],
                7 => payload_mut(&mut changed).minimum_block_hash[0] ^= 1,
                8 => changed.expected_control_revision += 1,
                9 => changed.expected_control_digest[0] ^= 1,
                10 => payload_mut(&mut changed).request.operation_id = [0; 32],
                11 => payload_mut(&mut changed).request.operation_id = [32; 32],
                12 => payload_mut(&mut changed).request.binding_digest[0] ^= 1,
                13 => {
                    payload_mut(&mut changed)
                        .request
                        .original_custody
                        .record_digest[0] ^= 1
                }
                14 => {
                    payload_mut(&mut changed)
                        .request
                        .original_custody
                        .control_state_digest[0] ^= 1
                }
                15 => payload_mut(&mut changed).request.statement_digest = [0; 32],
                16 => payload_mut(&mut changed).request.statement_digest[0] ^= 1,
                17 => payload_mut(&mut changed).request.statement_size = 0,
                18 => payload_mut(&mut changed).request.statement_size += 1,
                19 => {
                    payload_mut(&mut changed).request.statement_size =
                        SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1 as u64 + 1
                }
                _ => unreachable!(),
            }
            assert_ne!(changed, base, "field {field}");
            assert_no_writes(tx, &changed, &f.observer, false);
        }
        let mut wrong_deployment = base.clone();
        wrong_deployment.deployment_id = "promotion-secondary".into();
        assert_no_writes(tx, &wrong_deployment, &f.other, false);
    });
    assert!(applied(&f, &base, 2_100).is_ok());
}

#[test]
fn all_phases_require_their_exact_current_or_reserved_or_completed_subject() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let reserved = reserve_exact(&mut f, request, 2_000);
    let mut base = check_instruction(&f, request, Subject::BeforeProvider(reserved.clone()));
    transact(&mut f.state, 2_100, |tx| {
        for subject in [
            Subject::BeforeProvider(reserved.clone()),
            Subject::AfterProvider(reserved.clone()),
            Subject::BeforeCommit(reserved.clone()),
        ] {
            payload_mut(&mut base).subject = subject;
            assert_no_writes(tx, &base, &f.observer, true);
        }
        for subject in [
            Subject::AfterCommit(reserved.clone()),
            Subject::BeforeRelease(reserved.clone()),
        ] {
            payload_mut(&mut base).subject = subject;
            assert_no_writes(tx, &base, &f.observer, false);
        }
        payload_mut(&mut base).subject = Subject::Current(SignerOperationAuditHeadV1 {
            sequence: 1,
            digest: [92; 32],
        });
        assert_no_writes(tx, &base, &f.observer, false);
        instruction(tx, Action::Complete(completion(&reserved)))
            .execute(&f.operator, tx)
            .unwrap();
    });
    let completed = snapshot(&f, Some(request.operation_id)).operation.unwrap();
    transact(&mut f.state, 2_200, |tx| {
        for subject in [
            Subject::AfterCommit(completed.clone()),
            Subject::BeforeRelease(completed.clone()),
        ] {
            payload_mut(&mut base).subject = subject;
            assert_no_writes(tx, &base, &f.observer, true);
        }
        for subject in [
            Subject::BeforeProvider(completed.clone()),
            Subject::AfterProvider(completed.clone()),
            Subject::BeforeCommit(completed.clone()),
            Subject::AfterCommit(reserved.clone()),
        ] {
            payload_mut(&mut base).subject = subject;
            assert_no_writes(tx, &base, &f.observer, false);
        }
    });
}

fn changed_rows(row: &FinalPromotionOperationRecordV1) -> Vec<FinalPromotionOperationRecordV1> {
    let mutations: &[fn(&mut FinalPromotionOperationRecordV1)] = &[
        |r| r.deployment_id.push('x'),
        |r| r.revision += 1,
        |r| r.predecessor_digest[0] ^= 1,
        |r| r.request_digest[0] ^= 1,
        |r| r.execution.height += 1,
        |r| r.execution.ordinal += 1,
        |r| r.execution.recorded_at_unix_ms += 1,
        |r| r.execution.authority = AccountId::new(key(99).public_key().clone()),
        |r| r.intent.action = SignerOperationActionV1::Status,
        |r| r.intent.operation_id[0] ^= 1,
        |r| r.intent.request_digest[0] ^= 1,
        |r| r.intent.previous_audit.sequence += 1,
        |r| r.intent.previous_audit.digest[0] ^= 1,
        |r| r.custody.record_digest[0] ^= 1,
        |r| r.custody.control_state_digest[0] ^= 1,
        |r| r.reservation.reservation_id[0] ^= 1,
        |r| r.reservation.fence += 1,
        |r| r.reservation.expires_at_unix_ms += 1,
        |r| r.reserved.height += 1,
        |r| r.reserved.ordinal += 1,
        |r| r.reserved.recorded_at_unix_ms += 1,
        |r| r.reserved.authority = AccountId::new(key(99).public_key().clone()),
        |r| r.outcome = FinalPromotionOperationOutcomeV1::Invalidated,
    ];
    mutations
        .iter()
        .map(|mutate| {
            let mut changed = row.clone();
            mutate(&mut changed);
            assert_ne!(&changed, row);
            changed
        })
        .collect()
}

#[test]
fn reserved_and_completed_checks_bind_every_original_row_coordinate_and_owner() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let reserved = reserve_exact(&mut f, request, 2_000);
    let mut base = check_instruction(&f, request, Subject::BeforeProvider(reserved.clone()));
    transact(&mut f.state, 2_100, |tx| {
        assert_no_writes(tx, &base, &f.observer, true);
        for row in changed_rows(&reserved) {
            payload_mut(&mut base).subject = Subject::BeforeProvider(row);
            assert_no_writes(tx, &base, &f.observer, false);
        }
        payload_mut(&mut base).subject = Subject::BeforeProvider(reserved.clone());
        tx.world.account_permissions.insert(
            f.other.clone(),
            Permissions::from_iter([CanOperateSorafsFinalPromotion {
                deployment_id: DEPLOYMENT.into(),
            }
            .into()]),
        );
        assert_no_writes(tx, &base, &f.other, false);
        instruction(tx, Action::Complete(completion(&reserved)))
            .execute(&f.operator, tx)
            .unwrap();
    });
    let completed = snapshot(&f, Some(request.operation_id)).operation.unwrap();
    transact(&mut f.state, 2_200, |tx| {
        payload_mut(&mut base).subject = Subject::AfterCommit(completed.clone());
        assert_no_writes(tx, &base, &f.observer, true);
        assert_no_writes(tx, &base, &f.other, false);
        for row in changed_rows(&completed) {
            payload_mut(&mut base).subject = Subject::AfterCommit(row);
            assert_no_writes(tx, &base, &f.observer, false);
        }
        for field in 0..4 {
            let mut changed = completed.clone();
            let FinalPromotionOperationOutcomeV1::Completed(ref mut result) = changed.outcome
            else {
                panic!("completed fixture")
            };
            match field {
                0 => result.commitment.audit.sequence += 1,
                1 => result.commitment.audit.digest[0] ^= 1,
                2 => result.commitment.response_digest[0] ^= 1,
                3 => result.signatures_digest[0] ^= 1,
                _ => unreachable!(),
            }
            payload_mut(&mut base).subject = Subject::BeforeRelease(changed);
            assert_no_writes(tx, &base, &f.observer, false);
        }
    });
}

#[test]
fn reserved_checks_reject_exact_expiry_but_current_does_not_renew_the_slot() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let row = reserve_exact(&mut f, request, 2_000);
    let check = check_instruction(&f, request, Subject::BeforeCommit(row.clone()));
    let current = check_instruction(&f, request, Subject::Current(row.intent.previous_audit));
    transact(&mut f.state, row.reservation.expires_at_unix_ms - 1, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
    });
    assert!(applied(&f, &check, row.reservation.expires_at_unix_ms - 1).is_ok());
    assert_eq!(
        applied(&f, &check, row.reservation.expires_at_unix_ms),
        Err(Error::ReservationTime)
    );
    transact(&mut f.state, row.reservation.expires_at_unix_ms, |tx| {
        assert_no_writes(tx, &check, &f.observer, false);
        assert_no_writes(tx, &current, &f.observer, true);
    });
    assert_eq!(
        snapshot(&f, Some(request.operation_id)).operation,
        Some(row)
    );
}

#[test]
fn old_completed_checks_survive_new_audit_and_original_expiry_with_fresh_custody() {
    let mut f = enrolled();
    let first_request = request(&f, 31);
    let first = reserve_exact(&mut f, first_request, 2_000);
    transact(&mut f.state, 3_000, |tx| {
        instruction(tx, Action::Complete(completion(&first)))
            .execute(&f.operator, tx)
            .unwrap();
    });
    let completed = snapshot(&f, Some(first_request.operation_id))
        .operation
        .unwrap();
    let second_request = request(&f, 32);
    let second = reserve_exact(&mut f, second_request, 4_000);
    transact(&mut f.state, 5_000, |tx| {
        instruction(tx, Action::Complete(completion(&second)))
            .execute(&f.operator, tx)
            .unwrap();
    });
    let latest = snapshot(&f, None).operations;
    assert_eq!(latest.audit.sequence, 2);
    let check = check_instruction(&f, first_request, Subject::BeforeRelease(completed.clone()));
    transact(&mut f.state, 70_000, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
        let mut after = check.clone();
        payload_mut(&mut after).subject = Subject::AfterCommit(completed.clone());
        assert_no_writes(tx, &after, &f.observer, true);
    });
    let observed = applied(&f, &check, 70_000).unwrap();
    assert_eq!(observed.operation, Some(completed));
    assert_eq!(observed.operations, latest);
    assert!(
        applied(&f, &check, 100_000).is_err(),
        "enrollment expires independently"
    );
    transact(&mut f.state, 100_000, |tx| {
        let before = retained(tx);
        assert!(
            instruction(tx, Action::Complete(completion(&first)))
                .execute(&f.operator, tx)
                .is_err(),
            "a different signed Complete envelope cannot claim historical success"
        );
        assert_eq!(retained(tx), before);
        assert_no_writes(tx, &check, &f.observer, false);
    });
}

#[test]
fn check_then_same_block_signer_or_attester_revoke_fences_exact_reexecution_and_applied_cut() {
    for signer in [false, true] {
        let mut f = enrolled();
        let request = request(&f, 31);
        let reserved = reserve_exact(&mut f, request, 2_000);
        let check = check_instruction(&f, request, Subject::BeforeProvider(reserved));
        transact(&mut f.state, 3_000, |tx| {
            assert_no_writes(tx, &check, &f.observer, true);
            instruction(
                tx,
                Action::Revoke(FinalPromotionRevocationV1 {
                    signer,
                    attester: !signer,
                }),
            )
            .execute(&f.manager, tx)
            .unwrap();
            assert_no_writes(tx, &check, &f.observer, false);
            let fresh_cas = instruction(tx, check.action.clone());
            assert_no_writes(tx, &fresh_cas, &f.observer, false);
        });
        assert!(applied(&f, &check, 3_000).is_err());
        assert_eq!(
            snapshot(&f, Some(request.operation_id))
                .operation
                .unwrap()
                .outcome,
            FinalPromotionOperationOutcomeV1::Invalidated
        );
    }
}

#[test]
fn same_block_completion_invalidates_reserved_check_and_allows_exact_completed_check() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let reserved = reserve_exact(&mut f, request, 2_000);
    let check = check_instruction(&f, request, Subject::AfterProvider(reserved.clone()));
    let mut completed_check = check.clone();
    transact(&mut f.state, 3_000, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
        instruction(tx, Action::Complete(completion(&reserved)))
            .execute(&f.operator, tx)
            .unwrap();
        assert_no_writes(tx, &check, &f.observer, false);
        let completed = read_operation_slot(tx.world(), DEPLOYMENT, request.operation_id)
            .unwrap()
            .unwrap()
            .record;
        payload_mut(&mut completed_check).subject = Subject::AfterCommit(completed);
        assert_no_writes(tx, &completed_check, &f.observer, true);
    });
    assert!(applied(&f, &check, 3_000).is_err());
    assert!(applied(&f, &completed_check, 3_000).is_ok());
}

#[test]
fn current_check_rejects_trusted_time_before_native_enrollment_even_after_valid_issuance() {
    let mut f = fixture();
    configure(&mut f);
    let enrollment = attest(&f, 1_500, 100_000);
    transact(&mut f.state, 2_000, |tx| {
        instruction(tx, Action::Enroll(enrollment))
            .execute(&f.manager, tx)
            .unwrap();
    });
    let request = request(&f, 31);
    let before = snapshot(&f, None);
    assert_eq!(before.control_record.execution.recorded_at_unix_ms, 2_000);
    let check = check_instruction(&f, request, Subject::Current(before.operations.audit));
    transact(&mut f.state, 3_000, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
    });
    // The signed enrollment itself accepts both times; its later native admission is decisive.
    for now in [1_600, 1_999] {
        verified(&f, now);
        assert_eq!(applied(&f, &check, now), Err(Error::Custody));
    }
    for now in [2_000, 3_000] {
        let after = applied(&f, &check, now).unwrap();
        assert_eq!(after.control_record, before.control_record);
        assert_eq!(after.operations, before.operations);
        assert_eq!(after.operation, before.operation);
    }
}

#[test]
fn applied_operation_checks_reject_trusted_time_before_original_execution() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let row = reserve_exact(&mut f, request, 2_000);
    let mut check = check_instruction(&f, request, Subject::BeforeProvider(row.clone()));
    transact(&mut f.state, 3_000, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
    });
    // Custody is already eligible: these failures concern the operation execution timestamp.
    verified(&f, 1_999);
    for subject in [
        Subject::BeforeProvider(row.clone()),
        Subject::AfterProvider(row.clone()),
        Subject::BeforeCommit(row.clone()),
    ] {
        payload_mut(&mut check).subject = subject;
        assert_eq!(applied(&f, &check, 1_999), Err(Error::ReservationTime));
        assert!(applied(&f, &check, 2_000).is_ok());
    }
    transact(&mut f.state, 4_000, |tx| {
        instruction(tx, Action::Complete(completion(&row)))
            .execute(&f.operator, tx)
            .unwrap();
    });
    let completed = snapshot(&f, Some(request.operation_id)).operation.unwrap();
    verified(&f, 3_999);
    for subject in [
        Subject::AfterCommit(completed.clone()),
        Subject::BeforeRelease(completed),
    ] {
        payload_mut(&mut check).subject = subject;
        assert_eq!(applied(&f, &check, 3_999), Err(Error::ReservationTime));
        assert!(applied(&f, &check, 4_000).is_ok());
        assert!(applied(&f, &check, 70_000).is_ok());
    }
}

#[test]
fn applied_check_rejects_invalid_scope_before_encoding_and_requires_exact_binding() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let check = check_instruction(
        &f,
        request,
        Subject::Current(snapshot(&f, None).operations.audit),
    );
    transact(&mut f.state, 2_000, |tx| {
        assert_no_writes(tx, &check, &f.observer, true)
    });
    assert!(applied(&f, &check, 2_000).is_ok());
    for deployment in [
        String::new(),
        " ".into(),
        "x".repeat(FINAL_PROMOTION_MAX_RECORD_BYTES_V1),
    ] {
        let mut invalid = check.clone();
        invalid.deployment_id = deployment;
        assert_eq!(applied(&f, &invalid, 2_000), Err(Error::BindingMismatch));
    }
    for field in 0..5 {
        let mut binding = f.policy.binding.clone();
        match field {
            0 => binding.chain_id.push('x'),
            1 => binding.network_id[0] ^= 1,
            2 => binding.role = SignerRoleV1::ReleaseManifest,
            3 => {
                binding.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                    deployment_id: "promotion-secondary".into(),
                }
            }
            4 => binding.public_key = key(99).public_key().clone(),
            _ => unreachable!(),
        }
        assert!(
            eligibility::check_applied_snapshot_v1(
                &f.state.view(),
                &check,
                &binding,
                &f.observer,
                2_000
            )
            .is_err()
        );
    }
    let mut noncheck = check.clone();
    noncheck.action = Action::Revoke(FinalPromotionRevocationV1 {
        signer: true,
        attester: false,
    });
    assert_eq!(applied(&f, &noncheck, 2_000), Err(Error::Invalid));
}

#[test]
fn applied_cut_rechecks_account_and_direct_or_role_permissions_after_native_changes() {
    for observer_account in [false, true] {
        for change in ["direct", "account", "role_permission", "assigned_role"] {
            let mut f = enrolled();
            let request = request(&f, 31);
            let account = if observer_account {
                f.observer.clone()
            } else {
                f.operator.clone()
            };
            let permission: Permission = if observer_account {
                CanCheckSorafsFinalPromotion {
                    deployment_id: DEPLOYMENT.into(),
                }
                .into()
            } else {
                CanOperateSorafsFinalPromotion {
                    deployment_id: DEPLOYMENT.into(),
                }
                .into()
            };
            let role: RoleId = "check_operator".parse().unwrap();
            if change.starts_with("role_") || change == "assigned_role" {
                transact(&mut f.state, 1_700, |tx| {
                    Register::role(
                        Role::new(role.clone(), account.clone()).add_permission(permission.clone()),
                    )
                    .execute(&account, tx)
                    .unwrap();
                    Revoke::account_permission(permission.clone(), account.clone())
                        .execute(&account, tx)
                        .unwrap();
                });
            }
            let check = check_instruction(
                &f,
                request,
                Subject::Current(snapshot(&f, None).operations.audit),
            );
            transact(&mut f.state, 2_000, |tx| {
                assert_no_writes(tx, &check, &f.observer, true)
            });
            assert!(
                applied(&f, &check, 2_000).is_ok(),
                "positive {change} control"
            );
            transact(&mut f.state, 3_000, |tx| {
                assert_no_writes(tx, &check, &f.observer, true);
                match change {
                    "direct" => Revoke::account_permission(permission.clone(), account.clone())
                        .execute(&account, tx)
                        .unwrap(),
                    "account" => Unregister::account(account.clone())
                        .execute(&account, tx)
                        .unwrap(),
                    "role_permission" => Revoke::role_permission(permission.clone(), role.clone())
                        .execute(&account, tx)
                        .unwrap(),
                    "assigned_role" => Revoke::account_role(role.clone(), account.clone())
                        .execute(&account, tx)
                        .unwrap(),
                    _ => unreachable!(),
                }
                assert_no_writes(tx, &check, &f.observer, false);
            });
            assert_eq!(
                applied(&f, &check, 3_000),
                Err(Error::BindingMismatch),
                "{change}"
            );
        }
    }
}

#[test]
fn whole_check_instruction_bound_precedes_oversized_nested_row_comparison() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let mut row = reserve_exact(&mut f, request, 2_000);
    row.deployment_id = "x".repeat(FINAL_PROMOTION_MAX_RECORD_BYTES_V1);
    let check = check_instruction(&f, request, Subject::BeforeProvider(row));
    assert!(norito::canonical_frame_len(&check).unwrap() > FINAL_PROMOTION_MAX_RECORD_BYTES_V1);
    transact(&mut f.state, 3_000, |tx| {
        let before = retained(tx);
        assert_eq!(apply(check.clone(), &f.observer, tx), Err(Error::Invalid));
        assert_no_writes(tx, &check, &f.observer, false);
        assert_eq!(retained(tx), before);
    });
    assert_eq!(applied(&f, &check, 3_000), Err(Error::Invalid));
}

#[test]
fn corrupted_admission_latest_and_height_indexes_reject_check_without_writes() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let row = reserve_exact(&mut f, request, 2_000);
    let check = check_instruction(&f, request, Subject::BeforeProvider(row.clone()));
    transact(&mut f.state, 2_100, |tx| {
        assert_no_writes(tx, &check, &f.observer, true)
    });
    for path in [
        operation_admission_key(DEPLOYMENT, request.operation_id).unwrap(),
        operation_slot_key(DEPLOYMENT, request.operation_id).unwrap(),
        operation_height_key(DEPLOYMENT, row.execution.height, row.execution.ordinal).unwrap(),
    ] {
        transact(&mut f.state, 3_000, |tx| {
            let original = tx.world.smart_contract_state.get(&path).unwrap().clone();
            tx.world.smart_contract_state.insert(path.clone(), vec![0]);
            assert_no_writes(tx, &check, &f.observer, false);
            assert!(
                eligibility::check_applied_snapshot_v1(
                    tx,
                    &check,
                    &f.policy.binding,
                    &f.observer,
                    3_000
                )
                .is_err()
            );
            let mut current = check.clone();
            payload_mut(&mut current).request.operation_id = [32; 32];
            payload_mut(&mut current).subject = Subject::Current(row.intent.previous_audit);
            assert_no_writes(tx, &current, &f.observer, false);
            tx.world.smart_contract_state.insert(path, original);
            assert_no_writes(tx, &check, &f.observer, true);
        });
    }
    assert!(applied(&f, &check, 3_000).is_ok());
}

#[test]
fn reserved_subject_requires_the_selected_row_active_head_and_original_audit() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let row = reserve_exact(&mut f, request, 2_000);
    let check = check_instruction(&f, request, Subject::BeforeProvider(row.clone()));
    let custody = verified(&f, 3_000);
    let head = snapshot(&f, Some(request.operation_id)).operations;
    assert_eq!(
        eligibility::check_subject(
            payload(&check),
            &f.observer,
            &custody,
            &head,
            Some(&row),
            3_000
        ),
        Ok(())
    );
    assert_eq!(
        eligibility::check_subject(payload(&check), &f.observer, &custody, &head, None, 3_000),
        Err(Error::Conflict)
    );
    // Exercise the comparison helper directly; malformed heads never enter native State.
    let mutations: &[fn(&mut FinalPromotionOperationHeadV1)] = &[
        |head| head.active_operation = None,
        |head| head.active_operation = Some([32; 32]),
        |head| head.revision += 1,
        |head| head.digest[0] ^= 1,
        |head| head.audit.sequence += 1,
        |head| head.audit.digest[0] ^= 1,
    ];
    for mutate in mutations {
        let mut changed = head;
        mutate(&mut changed);
        assert_ne!(changed, head);
        assert_eq!(
            eligibility::check_subject(
                payload(&check),
                &f.observer,
                &custody,
                &changed,
                Some(&row),
                3_000
            ),
            Err(Error::Conflict)
        );
    }
    assert_eq!(snapshot(&f, Some(request.operation_id)).operations, head);
}

#[test]
fn check_subject_capacity_counter_boundary_does_not_allocate_an_operation() {
    use crate::query::final_promotion_authority::operation::successor_head;
    let mut f = enrolled();
    let request = request(&f, 31);
    let actual = reserve_exact(&mut f, request, 2_000);
    let retained_before = snapshot(&f, Some(request.operation_id));
    // A coherent adjacent suffix exercises the pure counter boundary. The omitted full history
    // is never inserted into State or represented as authenticated native authority.
    let mut old = actual.clone();
    old.revision = 2 * (FINAL_PROMOTION_MAX_OPERATIONS_V1 - 1);
    old.predecessor_digest = [81; 32];
    old.execution.height = 40;
    old.execution.recorded_at_unix_ms = 1_000;
    old.reserved.height = 39;
    old.reserved.recorded_at_unix_ms = 500;
    old.intent.operation_id = [30; 32];
    old.intent.request_digest = [82; 32];
    old.reservation.reservation_id = [83; 32];
    old.reservation.fence = FINAL_PROMOTION_MAX_OPERATIONS_V1 - 1;
    old.reservation.expires_at_unix_ms = 1_000;
    old.outcome = FinalPromotionOperationOutcomeV1::Expired;
    let head = FinalPromotionOperationHeadV1 {
        revision: old.revision,
        digest: operation_digest(&old).unwrap(),
        fence: old.reservation.fence,
        audit: old.intent.previous_audit,
        active_operation: None,
        total_admissions: FINAL_PROMOTION_MAX_OPERATIONS_V1 - 1,
    };
    let mut last = actual;
    last.revision = head.revision + 1;
    last.predecessor_digest = head.digest;
    last.execution.height = 41;
    last.reserved = last.execution.clone();
    last.reservation.fence = head.fence + 1;
    let at_capacity = successor_head(head, Some(&old), &last).unwrap();
    assert_eq!(
        at_capacity.total_admissions,
        FINAL_PROMOTION_MAX_OPERATIONS_V1
    );
    let check = check_instruction(&f, request, Subject::BeforeProvider(last.clone()));
    let custody = verified(&f, 3_000);
    for _ in 0..2 {
        assert_eq!(
            eligibility::check_subject(
                payload(&check),
                &f.observer,
                &custody,
                &at_capacity,
                Some(&last),
                3_000
            ),
            Ok(())
        );
    }
    for now in [0, u64::MAX, 1_999] {
        assert_eq!(
            eligibility::check_subject(
                payload(&check),
                &f.observer,
                &custody,
                &at_capacity,
                Some(&last),
                now
            ),
            Err(Error::ReservationTime)
        );
    }
    let completion = completion(&last);
    let mut terminal = last.clone();
    terminal.revision += 1;
    terminal.predecessor_digest = at_capacity.digest;
    terminal.request_digest = [84; 32];
    terminal.execution.height += 1;
    terminal.execution.recorded_at_unix_ms = 3_000;
    terminal.outcome = FinalPromotionOperationOutcomeV1::Completed(
        iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCompletedV1 {
            commitment: completion.commitment,
            signatures_digest: completion.signatures_digest,
        },
    );
    let exhausted = successor_head(at_capacity, Some(&last), &terminal).unwrap();
    assert_eq!(exhausted.revision, 2 * FINAL_PROMOTION_MAX_OPERATIONS_V1);
    let completed_check = check_instruction(&f, request, Subject::BeforeRelease(terminal.clone()));
    assert_eq!(
        eligibility::check_subject(
            payload(&completed_check),
            &f.observer,
            &verified(&f, 70_000),
            &exhausted,
            Some(&terminal),
            70_000
        ),
        Ok(())
    );
    assert_eq!(snapshot(&f, Some(request.operation_id)), retained_before);
}

mod observer;
