//! Retained-cut time checks use real native execution and fixed-roster finality.
//! These fixtures do not qualify UTC or turn a past Check into a new current-state observation.
use super::*;

fn verified_current(f: &mut Fixture) -> VerifiedFinalPromotionAccountCheckV1 {
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
        (2000 + 60_000, None),
        (2000 + 60_001, Some(Error::Authority)),
        (99_999, Some(Error::Authority)),
    ] {
        let mut f = Fixture::with_enrollment_time(2_000);
        assert_eq!(f.policy.max_anchor_age_ms, 60_000);
        let pending = f.pending();
        assert_eq!(
            f.commit(NOW, vec![pending.signed_transaction().clone()], true, true),
            [true]
        );
        let result = pending.verify_finalized(|| Ok(interval(2000, upper)));
        if let Some(error) = expected {
            assert_eq!(result.err(), Some(error));
        } else {
            let verified = result.unwrap();
            assert_eq!(verified.eligibility_time_interval(), interval(2000, upper));
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
    let revoke = f.instruction(FinalPromotionAccountCustodyActionV1::Revoke(
        FinalPromotionAccountCustodyRevocationV1 {
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

#[test]
fn retained_account_interval_rechecks_custody_expiry_before_the_original_age_limit() {
    let mut f = Fixture::new();
    let pending = f.pending();
    assert_eq!(
        f.commit(NOW, vec![pending.signed_transaction().clone()], true, true),
        [true]
    );
    let verified = pending
        .verify_finalized(|| Ok(interval(50_000, 50_000)))
        .unwrap();
    assert_eq!(
        verified.recheck_use_interval(interval(99_999, 99_999)),
        Ok(())
    );
    assert_eq!(
        verified.recheck_use_interval(interval(99_999, 100_000)),
        Err(Error::Authority)
    );
    assert_eq!(
        verified.eligibility_time_interval(),
        interval(50_000, 50_000)
    );
    assert_eq!(verified.applied_floor().height, 3);
}
