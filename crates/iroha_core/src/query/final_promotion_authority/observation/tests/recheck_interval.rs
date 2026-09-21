//! Retained-cut time checks use real native execution and fixed-roster finality.
//! These fixtures do not qualify UTC or turn a past Check into a new current-state observation.
use super::*;

fn verified_current(f: &mut Fixture) -> VerifiedFinalPromotionCheckV1 {
    let pending = f.pending();
    assert_eq!(
        f.commit(NOW, vec![pending.signed_transaction().clone()], true, true),
        [true]
    );
    pending
        .verify_finalized(|| Ok(interval(NOW, NOW + 10)))
        .unwrap()
}

#[test]
fn initial_interval_enforces_earliest_observation_age_at_exact_boundary() {
    for (upper, expected) in [
        (1500 + 60_000, None),
        (1500 + 60_001, Some(Error::Authority)),
        (99_999, Some(Error::Authority)),
    ] {
        let mut f = Fixture::new();
        assert_eq!(f.policy.max_anchor_age_ms, 60_000);
        let pending = f.pending();
        assert_eq!(
            f.commit(NOW, vec![pending.signed_transaction().clone()], true, true),
            [true]
        );
        let result = pending.verify_finalized(|| Ok(interval(1500, upper)));
        if let Some(error) = expected {
            assert_eq!(result.err(), Some(error));
        } else {
            let verified = result.unwrap();
            assert_eq!(verified.eligibility_time_interval(), interval(1500, upper));
            assert_eq!(
                verified.recheck_use_interval(interval(upper, upper)),
                Ok(())
            );
        }
    }
}

#[test]
fn retained_interval_never_renews_original_age_or_deadline() {
    let mut f = Fixture::new();
    let mut verified = verified_current(&mut f);
    let snapshot = verified.snapshot().clone();
    let instruction = verified.instruction().clone();
    let floor = verified.applied_floor();
    let entry_hash = verified.entry_hash();
    let before = f.snapshot();
    let age_limit = NOW + f.policy.max_anchor_age_ms;
    assert_eq!(
        verified.recheck_use_interval(interval(NOW + 100, NOW + 200)),
        Ok(())
    );
    assert_eq!(
        verified.recheck_use_interval(interval(age_limit, age_limit)),
        Ok(())
    );
    assert_eq!(
        verified.recheck_use_interval(interval(NOW + 200, age_limit + 1)),
        Err(Error::Authority),
        "the later sample cannot reset the original earliest observation"
    );
    for invalid in [
        interval(0, NOW),
        interval(NOW - 1, NOW + 10),
        interval(NOW + 1, NOW),
        interval(NOW, u64::MAX),
        interval(u64::MAX, u64::MAX),
    ] {
        assert_eq!(verified.recheck_use_interval(invalid), Err(Error::Clock));
    }
    assert_eq!(verified.snapshot(), &snapshot);
    assert_eq!(verified.instruction(), &instruction);
    assert_eq!(verified.applied_floor(), floor);
    assert_eq!(verified.entry_hash(), entry_hash);
    assert_eq!(
        verified.eligibility_time_interval(),
        interval(NOW, NOW + 10)
    );
    assert_eq!(f.snapshot(), before);
    assert_eq!(f.state.view().height(), 3);
    verified.round.expire_for_test();
    for time in [interval(NOW, NOW), interval(0, u64::MAX)] {
        assert_eq!(verified.recheck_use_interval(time), Err(Error::Expired));
    }
    assert_eq!(
        verified.eligibility_time_interval(),
        interval(NOW, NOW + 10)
    );
    assert_eq!(f.snapshot(), before);
}

#[test]
fn retained_interval_does_not_claim_newer_native_authority() {
    let mut f = Fixture::new();
    let verified = verified_current(&mut f);
    let original = verified.snapshot().clone();
    let pending = f.pending();
    let revoke = f.instruction(FinalPromotionAuthorityActionV1::Revoke(
        FinalPromotionRevocationV1 {
            signer: true,
            attester: false,
        },
    ));
    assert_eq!(
        f.commit(
            NOW + 1,
            vec![
                pending.signed_transaction().clone(),
                f.sign(revoke.into(), 1, NOW + 1)
            ],
            true,
            true,
        ),
        [true, true]
    );
    let changed = f.snapshot();
    assert!(changed.control.signer_revoked);
    assert_eq!(
        verified.recheck_use_interval(interval(NOW + 1, NOW + 2)),
        Ok(())
    );
    assert_eq!(verified.snapshot(), &original);
    assert_eq!(verified.applied_floor().height, 3);
    assert_eq!(f.snapshot(), changed);
    assert_eq!(
        pending
            .verify_finalized(|| Ok(interval(NOW + 1, NOW + 2)))
            .err(),
        Some(Error::Authority),
        "only a fresh Check can observe the subsequent revocation"
    );
}

fn verified_subject(
    f: &mut Fixture,
    subject: FinalPromotionCheckSubjectV1,
) -> VerifiedFinalPromotionCheckV1 {
    let mut expected = f.expected();
    expected.subject = subject;
    let prepared =
        begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
            .unwrap();
    let signed = f.sign(prepared_instruction(&prepared), 3, NOW + 2);
    let pending = prepared.bind_signed_transaction(signed).unwrap();
    assert_eq!(
        f.commit(
            NOW + 2,
            vec![pending.signed_transaction().clone()],
            true,
            true
        ),
        [true]
    );
    pending
        .verify_finalized(|| Ok(interval(NOW + 2, NOW + 2)))
        .unwrap()
}

#[test]
fn retained_reserved_interval_keeps_all_three_exclusive_phase_expiries() {
    for phase in 0..3 {
        let mut f = Fixture::new();
        let row = reserve_reviewed_request(&mut f);
        let expiry = row.reservation.expires_at_unix_ms;
        let subject = match phase {
            0 => FinalPromotionCheckSubjectV1::BeforeProvider(row.clone()),
            1 => FinalPromotionCheckSubjectV1::AfterProvider(row.clone()),
            _ => FinalPromotionCheckSubjectV1::BeforeCommit(row.clone()),
        };
        let verified = verified_subject(&mut f, subject);
        let before = f.snapshot();
        assert_eq!(
            verified.recheck_use_interval(interval(expiry - 1, expiry - 1)),
            Ok(())
        );
        assert_eq!(
            verified.recheck_use_interval(interval(expiry - 1, expiry)),
            Err(Error::Authority)
        );
        assert_eq!(verified.snapshot().operation.as_ref(), Some(&row));
        assert_eq!(
            verified.eligibility_time_interval(),
            interval(NOW + 2, NOW + 2)
        );
        assert_eq!(f.snapshot(), before);
    }
}

#[test]
fn retained_completed_interval_preserves_timely_completion_after_reservation_expiry() {
    use iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCompleteV1;
    use sorafs_manifest::signer::protocol::{
        SignerOperationAuditHeadV1, SignerOperationCommitmentV1,
    };
    for release in [false, true] {
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
        let subject = if release {
            FinalPromotionCheckSubjectV1::BeforeRelease(row.clone())
        } else {
            FinalPromotionCheckSubjectV1::AfterCommit(row.clone())
        };
        let verified = verified_subject(&mut f, subject);
        let after_expiry = row.reservation.expires_at_unix_ms + 1;
        assert_eq!(
            verified.recheck_use_interval(interval(after_expiry, after_expiry)),
            Ok(())
        );
        assert_eq!(verified.snapshot().operation.as_ref(), Some(&row));
        assert_eq!(
            verified.eligibility_time_interval(),
            interval(NOW + 2, NOW + 2)
        );
    }
}
