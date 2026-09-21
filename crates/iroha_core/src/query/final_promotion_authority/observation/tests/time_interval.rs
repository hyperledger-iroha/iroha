//! Finite caller-supplied UTC uncertainty checks reuse the native custody and phase owners.
//! These software-custody and real test-QC fixtures do not qualify a deployment clock.

use super::*;
use iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCompleteV1;
use sorafs_manifest::signer::protocol::{SignerOperationAuditHeadV1, SignerOperationCommitmentV1};

fn pending_for_subject(
    f: &Fixture,
    subject: FinalPromotionCheckSubjectV1,
) -> PendingFinalPromotionCheckV1 {
    let mut expected = f.expected();
    expected.subject = subject;
    let prepared =
        begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
            .unwrap();
    let signed = f.sign(prepared_instruction(&prepared), 3, NOW + 2);
    prepared.bind_signed_transaction(signed).unwrap()
}

#[test]
fn finite_utc_interval_requires_both_custody_endpoints_at_one_applied_cut() {
    for (time, expected) in [
        (interval(1_500, 61_500), None),
        (interval(1_499, NOW), Some(Error::Authority)),
        (interval(NOW, 100_000), Some(Error::Authority)),
    ] {
        let mut f = Fixture::new();
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
        assert_eq!(
            samples.get(),
            1,
            "both endpoints come from one clock sample"
        );
        match (result, expected) {
            (Ok(verified), None) => {
                assert_eq!(verified.eligibility_time_interval(), time);
                assert_eq!(verified.snapshot(), &f.snapshot());
                assert_eq!(verified.check_height(), 3);
                assert_eq!(verified.applied_floor().height, 3);
                verified.ensure_live().unwrap();
            }
            (Err(actual), Some(expected)) => assert_eq!(actual, expected),
            _ => panic!("both finite UTC endpoints must satisfy custody eligibility"),
        }
    }
}

#[test]
fn malformed_interval_is_rejected_before_native_eligibility() {
    for time in [
        interval(0, 0),
        interval(0, NOW),
        interval(NOW, 0),
        interval(NOW, u64::MAX),
        interval(u64::MAX, u64::MAX),
        interval(NOW + 1, NOW),
    ] {
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
                    f.sign(revoke.into(), 1, NOW),
                ],
                true,
                true,
            ),
            [true, true]
        );
        // The exact Check succeeded, but same-cut custody is revoked. A malformed interval
        // must return Clock before the native eligibility owner would return Authority.
        assert_eq!(
            pending.verify_finalized(|| Ok(time)).err(),
            Some(Error::Clock)
        );
    }
}

#[test]
fn reserved_interval_checks_execution_lower_bound_and_exclusive_expiry_upper_bound() {
    for phase in 0..3 {
        for boundary in 0..3 {
            let mut f = Fixture::new();
            let row = reserve_reviewed_request(&mut f);
            let expiry = row.reservation.expires_at_unix_ms;
            let time = match boundary {
                0 => interval(NOW, expiry - 1),
                1 => interval(NOW - 1, NOW + 2),
                _ => interval(NOW + 2, expiry),
            };
            let subject = match phase {
                0 => FinalPromotionCheckSubjectV1::BeforeProvider(row.clone()),
                1 => FinalPromotionCheckSubjectV1::AfterProvider(row.clone()),
                _ => FinalPromotionCheckSubjectV1::BeforeCommit(row.clone()),
            };
            let pending = pending_for_subject(&f, subject);
            assert_eq!(
                f.commit(
                    NOW + 2,
                    vec![pending.signed_transaction().clone()],
                    true,
                    true
                ),
                [true]
            );
            let result = pending.verify_finalized(|| Ok(time));
            if boundary == 0 {
                let verified = result.unwrap();
                assert_eq!(verified.eligibility_time_interval(), time);
                assert_eq!(verified.snapshot().operation.as_ref(), Some(&row));
                assert_eq!(verified.applied_floor().height, 4);
            } else {
                assert_eq!(result.err(), Some(Error::Authority));
            }
        }
    }
}

#[test]
fn completed_interval_checks_execution_time_without_renewing_reservation_expiry() {
    for before_release in [false, true] {
        for lower_before_execution in [false, true] {
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
            let time = if lower_before_execution {
                interval(NOW, NOW + 2)
            } else {
                interval(NOW + 1, row.reservation.expires_at_unix_ms + 1)
            };
            let subject = if before_release {
                FinalPromotionCheckSubjectV1::BeforeRelease(row.clone())
            } else {
                FinalPromotionCheckSubjectV1::AfterCommit(row.clone())
            };
            let pending = pending_for_subject(&f, subject);
            assert_eq!(
                f.commit(
                    NOW + 2,
                    vec![pending.signed_transaction().clone()],
                    true,
                    true
                ),
                [true]
            );
            let result = pending.verify_finalized(|| Ok(time));
            if lower_before_execution {
                assert_eq!(result.err(), Some(Error::Authority));
            } else {
                let verified = result.unwrap();
                assert_eq!(verified.eligibility_time_interval(), time);
                assert_eq!(verified.snapshot().operation.as_ref(), Some(&row));
                assert_eq!(verified.applied_floor().height, 5);
            }
        }
    }
}
