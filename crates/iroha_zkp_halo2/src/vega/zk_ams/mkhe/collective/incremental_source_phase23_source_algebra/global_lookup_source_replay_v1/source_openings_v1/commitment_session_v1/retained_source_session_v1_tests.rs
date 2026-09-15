//! Immutable source-prefix validation and same-session failure custody.
use super::*;
use crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::TestPreparedLowDigitV1;

// Isolated inventory fixture: it is not an authenticated source/replay provider.
fn complete_source_v1() -> (
    GlobalLookupCommitmentSessionV1<core::convert::Infallible, SourceOpeningCompleteStageV1>,
    [u8; 32],
) {
    let mut session =
        GlobalLookupProofSessionEntropySealV1::test_only_v1([0x31; 32], [0x41; 32]).unwrap();
    session.bind_source_opening_context_v1([0x32; 32]).unwrap();
    let point = Point::canonical_generator().unwrap();
    for ordinal in 0..344 {
        drop(session.sample_source_blinding_v1(ordinal).unwrap());
        session.adopt_source_commitment_v1(ordinal, &point).unwrap();
    }
    let points = session
        .live
        .as_ref()
        .unwrap()
        .inventory
        .adopted_source_commitments_root_v1([0x32; 32])
        .unwrap();
    (
        session
            .complete_source_opening_v1([0x53; 32], [0x32; 32], points, [0x52; 32])
            .unwrap(),
        points,
    )
}

#[test]
fn immutable_prefix_checks_all_actual_axes_and_cannot_be_resealed() {
    let (mut source, points) = complete_source_v1();
    source
        .validate_completed_source_prefix_v1([0x53; 32], [0x32; 32], points, [0x52; 32])
        .unwrap();
    for axis in 0..4 {
        let mut axes = [[0x53; 32], [0x32; 32], points, [0x52; 32]];
        axes[axis][31] ^= 1;
        assert!(
            source
                .validate_completed_source_prefix_v1(axes[0], axes[1], axes[2], axes[3])
                .is_err()
        );
    }
    assert!(
        source
            .live
            .as_mut()
            .unwrap()
            .inventory
            .seal_source_prefix_v1([0x54; 32], [0x31; 32], [0x32; 32], points, [0x52; 32])
            .is_err()
    );
    source.live.as_mut().unwrap().proof_session_context_digest[31] ^= 1;
    assert!(
        source
            .validate_completed_source_prefix_v1([0x53; 32], [0x32; 32], points, [0x52; 32])
            .is_err()
    );
}

#[test]
fn progressed_real_computed_commitment_retains_prefix_but_cannot_restart_source_stage() {
    let _guard = crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::prepared_commitment_test_guard_v1();
    let fixture = TestPreparedLowDigitV1::new_v1();
    let (session, points) = complete_source_v1();
    let mut owner = RetainedSourceSessionV1::from_source_complete_v1(session).unwrap();
    owner.require_low_digit_start_v1().unwrap();
    owner
        .commit_prepared_low_digit_v1(&fixture.statement_v1(0))
        .unwrap();
    assert!(matches!(
        owner.phase,
        Some(RetainedSourcePhaseV1::ExistingLow(_))
    ));
    owner
        .validate_completed_source_prefix_v1([0x53; 32], [0x32; 32], points, [0x52; 32])
        .unwrap();
    assert!(owner.require_low_digit_start_v1().is_err());
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    assert!(
        owner
            .commit_prepared_low_digit_v1(&fixture.statement_v1(0))
            .is_err()
    );
    assert!(owner.phase.is_none());
    assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
    assert!(
        owner
            .validate_completed_source_prefix_v1([0x53; 32], [0x32; 32], points, [0x52; 32])
            .is_err()
    );
}

#[test]
fn skip_entropy_rejection_and_unwind_drop_the_only_retained_session() {
    let _guard = crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::prepared_commitment_test_guard_v1();
    let fixture = TestPreparedLowDigitV1::new_v1();
    let (session, _) = complete_source_v1();
    let mut skipped = RetainedSourceSessionV1::from_source_complete_v1(session).unwrap();
    assert!(
        skipped
            .commit_prepared_low_digit_v1(&fixture.statement_v1(1))
            .is_err()
    );
    assert!(skipped.phase.is_none());
    for fault in [
        TestEntropyFaultV1::ErrorAt(344),
        TestEntropyFaultV1::ZeroAt(344),
        TestEntropyFaultV1::PanicAt(344),
    ] {
        let (mut session, _) = complete_source_v1();
        let GlobalLookupProofSessionEntropySourceV1::TestOnly(entropy) =
            &mut session.live.as_mut().unwrap().entropy
        else {
            unreachable!()
        };
        entropy.fault = fault;
        let mut owner = RetainedSourceSessionV1::from_source_complete_v1(session).unwrap();
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.commit_prepared_low_digit_v1(&fixture.statement_v1(0))
        }));
        assert!(matches!(result, Err(_) | Ok(Err(_))));
        assert!(owner.phase.is_none());
        assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
        assert!(
            owner
                .commit_prepared_low_digit_v1(&fixture.statement_v1(0))
                .is_err()
        );
    }
}

#[test]
fn sole_phase_is_taken_before_failure_and_exposes_no_rebind_or_point_adoption() {
    let source = include_str!("retained_source_session_v1.rs");
    let compact = source.split_whitespace().collect::<String>();
    let commit = compact
        .split("fncommit_prepared_low_digit_v1(")
        .nth(1)
        .unwrap();
    assert!(
        commit.find("self.phase.take()").unwrap()
            < commit
                .find("assembly.commit_prepared_values_v1(statement)")
                .unwrap()
    );
    for forbidden in [
        "impl Clone",
        "impl Copy",
        "pub fn",
        "fn into_parts",
        "fn session_v1",
        "point: &",
        "blinding: &",
        "TestOnly",
    ] {
        assert!(
            !source.contains(forbidden),
            "unexpected retained surface: {forbidden}"
        );
    }
    let prefix = include_str!("../commitment_session_v1.rs");
    let validator = prefix
        .split("fn validate_completed_source_prefix_v1<R: crate::vega::MaskedRelaxedRandomSourceV1>(\n    live:")
        .nth(1)
        .unwrap()
        .split("\nfn ")
        .next()
        .unwrap();
    assert!(!validator.contains("next_global_ordinal"));
}

#[test]
fn comparator_cannot_run_before_actual_completed_ds_or_recover_the_consumed_phase() {
    let _guard = crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::prepared_commitment_test_guard_v1();
    use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::TestPreparedComparatorV1;
    let fixture = TestPreparedComparatorV1::new_v1();
    let (source, _) = complete_source_v1();
    let mut owner = RetainedSourceSessionV1::from_source_complete_v1(source).unwrap();
    assert!(owner.require_comparator_position_v1(0).is_err());
    assert!(
        owner
            .commit_prepared_comparator_v1(&fixture.statement_v1(0))
            .is_err()
    );
    assert!(owner.phase.is_none());
    assert!(owner.require_low_digit_start_v1().is_err());
    assert!(
        owner
            .commit_prepared_comparator_v1(&fixture.statement_v1(0))
            .is_err()
    );
}

#[test]
fn comparator_entropy_rejection_and_unwind_drop_all_retained_opening_buffers() {
    let _guard = crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::prepared_commitment_test_guard_v1();
    use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::TestPreparedComparatorV1;
    let fixture = TestPreparedComparatorV1::new_v1();
    for fault in [
        TestEntropyFaultV1::ErrorAt(12_040),
        TestEntropyFaultV1::ZeroAt(12_040),
        TestEntropyFaultV1::PanicAt(12_040),
    ] {
        let previous = super::super::existing_radix_candidate_v1::tests::complete_patterned_candidate_with_fault_v1(fault);
        let mut owner = RetainedSourceSessionV1 {
            phase: Some(RetainedSourcePhaseV1::ExistingLowComplete(previous)),
        };
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.commit_prepared_comparator_v1(&fixture.statement_v1(0))
        }));
        assert!(matches!(
            result,
            Err(_) | Ok(Err(ZkAmsMkheErrorV1::RandomUnavailable))
        ));
        assert!(owner.phase.is_none());
        assert!(zeroizing_t256_scalar_vec_drop_count_v1() >= before + 2);
        assert!(
            owner
                .commit_prepared_comparator_v1(&fixture.statement_v1(0))
                .is_err()
        );
    }
}
