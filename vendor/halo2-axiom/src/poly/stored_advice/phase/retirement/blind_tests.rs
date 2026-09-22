//! Original advice blind folding and complete-owner rejection/cleanup tests.

use super::*;
use crate::poly::commitment::Blind;

fn fold_success<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let (owner, provider) = retirement_fixture(&params, false);
    // Read the independent original phase guards before their consuming handoff.
    let original = owner
        .lagrange
        .session
        .as_ref()
        .unwrap()
        .columns
        .iter()
        .map(|column| (column.blind.0).0)
        .collect::<Vec<_>>();
    let allocation = owner.prepare_coefficient_only_handoff().unwrap();
    let mut retained = owner.into_coefficient_only(allocation).unwrap();
    let columns = retained.columns.as_ptr();
    let challenges = retained.challenges.as_ptr();
    let phases = retained.plan.phases.as_ptr();
    let cursor = retained.greatest_ordinal().unwrap();
    let context = retained.proof_context().unwrap();
    let io = (provider.bank.reads.get(), provider.bank.creates.get());
    let mut accumulator = Blind(C::Scalar::from(19));
    let mut expected = accumulator.0;
    take_blind_clear_observations();
    // Include zero, one and nontrivial challenges, and repeated/interleaved original phases.
    for (column, challenge) in [(4, 0), (0, 1), (3, 7), (1, 3), (2, 11), (4, 13)] {
        let challenge = C::Scalar::from(challenge);
        expected = expected * challenge + original[column];
        retained = retained
            .fold_opening_blind(column as u32, challenge, &mut accumulator)
            .unwrap();
        assert_eq!(accumulator.0, expected);
        assert_eq!(retained.columns.as_ptr(), columns);
        assert_eq!(retained.challenges.as_ptr(), challenges);
        assert_eq!(retained.plan.phases.as_ptr(), phases);
        assert_eq!(retained.greatest_ordinal().unwrap(), cursor);
        assert_eq!(retained.proof_context().unwrap(), context);
        assert!(std::ptr::eq(retained.params().unwrap(), &params));
        assert_eq!(
            retained
                .columns
                .iter()
                .map(|c| (c.blind.0).0)
                .collect::<Vec<_>>(),
            original
        );
        assert_eq!((provider.bank.reads.get(), provider.bank.creates.get()), io);
        assert_eq!(take_blind_clear_observations(), (0, true));
    }
    drop(retained);
    assert_eq!(provider.bank.live.get(), 0);
    assert_eq!(take_blind_clear_observations(), (5, true));
}

#[test]
fn both_pasta_opening_blind_fold_matches_original_horner_without_detaching_phase_owners() {
    fold_success::<EqAffine>();
    fold_success::<EpAffine>();
}

fn fold_failures<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    // Measure the actual two full-owner sweeps, then inject both errors and panics at each call.
    let (owner, provider) = retirement_fixture(&params, false);
    let allocation = owner.prepare_coefficient_only_handoff().unwrap();
    let retained = owner.into_coefficient_only(allocation).unwrap();
    provider.bank.layout_calls.set(0);
    let retained = retained
        .fold_opening_blind(2, C::Scalar::ONE, &mut Blind(C::Scalar::ONE))
        .unwrap();
    let calls = provider.bank.layout_calls.get();
    assert_eq!(
        calls, 10,
        "five original coefficients checked before and after folding"
    );
    drop(retained);
    for panic in [false, true] {
        for target in 0..calls {
            let (owner, provider) = retirement_fixture(&params, false);
            let allocation = owner.prepare_coefficient_only_handoff().unwrap();
            let retained = owner.into_coefficient_only(allocation).unwrap();
            let io = (provider.bank.reads.get(), provider.bank.creates.get());
            provider.bank.layout_calls.set(0);
            provider.bank.fold_fault.set(Some((target, panic)));
            take_blind_clear_observations();
            let mut accumulator = Blind(C::Scalar::from(23));
            let result = catch_unwind(AssertUnwindSafe(|| {
                retained.fold_opening_blind(2, C::Scalar::from(7), &mut accumulator)
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert!(provider.bank.fold_fault.get().is_none());
            assert_eq!(accumulator.0, C::Scalar::ZERO);
            assert_eq!(provider.bank.live.get(), 0);
            assert_eq!(provider.bank.writers.get(), 0);
            assert_eq!((provider.bank.reads.get(), provider.bank.creates.get()), io);
            assert_eq!(take_blind_clear_observations(), (5, true));
        }
    }
    for (empty, column) in [(false, 5), (false, u32::MAX), (true, 0)] {
        let (owner, provider) = retirement_fixture(&params, empty);
        let allocation = owner.prepare_coefficient_only_handoff().unwrap();
        let retained = owner.into_coefficient_only(allocation).unwrap();
        take_blind_clear_observations();
        let mut accumulator = Blind(C::Scalar::from(17));
        assert!(
            retained
                .fold_opening_blind(column, C::Scalar::ONE, &mut accumulator)
                .is_err()
        );
        assert_eq!(accumulator.0, C::Scalar::ZERO);
        assert_eq!(provider.bank.live.get(), 0);
        assert_eq!(
            take_blind_clear_observations(),
            (if empty { 0 } else { 5 }, true)
        );
    }
}

#[test]
fn both_pasta_opening_blind_preflight_and_every_validation_failure_clear_output_and_destroy_owner()
{
    fold_failures::<EqAffine>();
    fold_failures::<EpAffine>();
}
