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
    // The original retained-source contract also applies to its enclosing
    // production adapters. Raw caller rho/digit arithmetic belongs only to
    // the lower kernel, not a second source-opening route.
    for owner in [
        source,
        include_str!("../../source_openings_v1.rs"),
        include_str!("../../../global_lookup_source_replay_v1.rs"),
    ] {
        assert!(!owner.contains("q_mask_kernel_commitment_v1"));
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

#[test]
fn prepared_small_signed_dispatch_advances_original_owner_and_consumes_failed_retry_and_prepared_opening_tail()
 {
    use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::{TestPreparedSmallSignedV1, prepared_commitment_test_guard_v1};
    let _guard = prepared_commitment_test_guard_v1();
    // Earlier inventory is synthetic; this exercises actual retained dispatch
    // and the actual signed-value MSM, not an authenticated end-to-end source.
    let previous = RnsNativeSmallSignedCommitmentsV1::test_completed_continuation_v1();
    let mut owner = RetainedSourceSessionV1 {
        phase: Some(RetainedSourcePhaseV1::ComparatorContinuation(previous)),
    };
    owner.require_small_signed_position_v1(7_224).unwrap();
    assert!(owner.require_small_signed_position_v1(7_225).is_err());
    let fixture = TestPreparedSmallSignedV1::for_ordinal_v1(7_224);
    let tail = owner
        .commit_prepared_small_signed_v1(&fixture.statement_v1(7_224))
        .unwrap();
    let tail = fixture.consume_with_original_tail_v1(tail, 7_224);
    let mut expected_entropy: GlobalLookupProofSessionEntropySourceV1<core::convert::Infallible> =
        GlobalLookupProofSessionEntropySourceV1::TestOnly(DeterministicProofSessionEntropyV1 {
            seed: [0x41; 32],
            fault: TestEntropyFaultV1::None,
        });
    let (_, rho) = sample_blinding_v1(&mut expected_entropy, 25_112).unwrap();
    assert_eq!(&tail.as_slice_v1()[..32], &rho.as_ref().to_be_bytes());
    assert_eq!(
        &tail.as_slice_v1()[32..65],
        &TestPreparedSmallSignedV1::expected_v1(7_224, rho.get())
            .to_non_identity_wire_bytes()
            .unwrap()
    );
    assert!(tail.as_slice_v1()[65..].iter().all(|byte| *byte == 0));
    assert!(matches!(
        owner.phase,
        Some(RetainedSourcePhaseV1::SmallSigned(_))
    ));
    owner.require_small_signed_position_v1(7_225).unwrap();
    assert!(owner.require_comparator_position_v1(688).is_err());
    assert!(owner.require_difference_start_v1().is_err());
    assert!(owner.require_low_digit_start_v1().is_err());
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    let fixture = TestPreparedSmallSignedV1::for_ordinal_v1(7_224);
    assert!(
        owner
            .commit_prepared_small_signed_v1(&fixture.statement_v1(7_224))
            .is_err()
    );
    assert!(owner.phase.is_none());
    assert!(zeroizing_t256_scalar_vec_drop_count_v1() >= before + 5);
    assert!(owner.require_small_signed_position_v1(7_225).is_err());
}

#[test]
fn prepared_small_signed_dispatch_refuses_and_consumes_an_earlier_source_stage() {
    use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::{TestPreparedSmallSignedV1, prepared_commitment_test_guard_v1};
    let _guard = prepared_commitment_test_guard_v1();
    let (source, _) = complete_source_v1();
    let mut owner = RetainedSourceSessionV1::from_source_complete_v1(source).unwrap();
    let fixture = TestPreparedSmallSignedV1::for_ordinal_v1(7_224);
    assert!(owner.require_small_signed_position_v1(7_224).is_err());
    assert!(
        owner
            .commit_prepared_small_signed_v1(&fixture.statement_v1(7_224))
            .is_err()
    );
    assert!(owner.phase.is_none());
    assert!(owner.require_low_digit_start_v1().is_err());
    assert!(
        owner
            .commit_prepared_small_signed_v1(&fixture.statement_v1(7_224))
            .is_err()
    );
}

#[test]
fn retained_source_packing_preparation_refuses_an_earlier_stage_and_consumes_owner() {
    let (session, _) = complete_source_v1();
    let mut owner = RetainedSourceSessionV1::from_source_complete_v1(session).unwrap();
    assert!(owner.prepare_source_packing_openings_v1().is_err());
    assert!(owner.phase.is_none());
    assert!(owner.prepare_source_packing_openings_v1().is_err());
    assert!(owner.require_low_digit_start_v1().is_err());
}

#[test]
fn retained_stored_replay_admission_is_consuming_and_cannot_restart_mutation() {
    use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::prepared_commitment_test_guard_v1;
    let _guard = prepared_commitment_test_guard_v1();
    for attempt in 0..3 {
        let mut owner = RetainedSourceSessionV1 {
            phase: Some(RetainedSourcePhaseV1::SmallSigned(
                RnsNativeSmallSignedCommitmentsV1::test_completed_signed_v1(),
            )),
        };
        // Tail replay is unavailable on a mutable, merely completed session.
        assert!(
            owner
                .validate_stored_plane_tail_v1(0, &[0; 16_384])
                .is_err()
        );
        owner.begin_stored_plane_replay_v1().unwrap();
        assert!(matches!(
            owner.phase,
            Some(RetainedSourcePhaseV1::StoredPlaneReplay(_))
        ));
        assert!(owner.require_low_digit_start_v1().is_err());
        assert!(owner.require_comparator_position_v1(0).is_err());
        assert!(owner.require_small_signed_position_v1(7_224).is_err());
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        let result = match attempt {
            0 => owner.begin_stored_plane_replay_v1(),
            1 => owner.prepare_source_packing_openings_v1(),
            _ => owner
                .commit_prepared_low_digit_v1(&TestPreparedLowDigitV1::new_v1().statement_v1(0)),
        };
        assert!(result.is_err());
        assert!(owner.phase.is_none());
        assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
    }
}

#[test]
fn retained_stored_replay_rejects_earlier_stage_and_cleared_session() {
    let (session, _) = complete_source_v1();
    let mut owner = RetainedSourceSessionV1::from_source_complete_v1(session).unwrap();
    assert!(owner.begin_stored_plane_replay_v1().is_err());
    assert!(owner.phase.is_none());
    assert!(owner.begin_stored_plane_replay_v1().is_err());
    assert!(
        owner
            .validate_stored_plane_tail_v1(0, &[0; 16_384])
            .is_err()
    );
}

#[test]
#[cfg(unix)]
fn qmask_first_sample_rejects_earlier_retained_phase_and_destroys_only_original_session() {
    use crate::testing::TestDirectory;
    use crate::vega::zk_ams::mkhe::global_lookup_statement_v1::{
        OrderedPlaneSpoolWriterV1, OrderedStorageSessionBudgetV1,
    };
    let directory = TestDirectory::new("qmask-retained-earlier");
    let mut storage = OrderedStorageSessionBudgetV1::new_v1();
    let mut writer = OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(
        directory.path(),
        [91; 32],
        &mut storage,
    )
    .unwrap();
    for slot in 0..66 {
        let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
        if slot % 33 == 32 {
            chunk.as_mut_slice_v1()[31] = 1;
            chunk.as_mut_slice_v1()[32..65].copy_from_slice(
                &Point::canonical_generator()
                    .unwrap()
                    .to_non_identity_wire_bytes()
                    .unwrap(),
            );
        }
        writer.write_slot_v1(slot, chunk).unwrap();
    }
    let pair = writer.seal_v1().unwrap();
    let plan = pair.q_mask_s_file_plan_v1().unwrap();
    struct UnusedEntropy(std::rc::Rc<core::cell::Cell<usize>>);
    impl crate::vega::MaskedRelaxedRandomSourceV1 for UnusedEntropy {
        fn fill_bytes(
            &mut self,
            _: &mut [u8],
        ) -> Result<(), crate::vega::MaskedRelaxedRandomErrorV1> {
            panic!("earlier phase must reject before original entropy");
        }
    }
    impl Drop for UnusedEntropy {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = std::rc::Rc::new(core::cell::Cell::new(0));
    let (mut fixture, _) = complete_source_v1();
    let live = fixture.live.take().unwrap();
    let mut source = GlobalLookupCommitmentSessionV1::<_, SourceOpeningCompleteStageV1> {
        live: Some(GlobalLookupCommitmentSessionLiveV1 {
            proof_resources: live.proof_resources,
            entropy: GlobalLookupProofSessionEntropySourceV1::Production {
                original_random: UnusedEntropy(std::rc::Rc::clone(&drops)),
                commitment_entropy_bytes: 0,
                q_mask_entropy_bytes: 0,
            },
            inventory: live.inventory,
            proof_session_context_digest: live.proof_session_context_digest,
            source_opening_context_digest: live.source_opening_context_digest,
            next_global_ordinal: live.next_global_ordinal,
            next_purpose: live.next_purpose,
            next_purpose_ordinal: live.next_purpose_ordinal,
            pending_source: live.pending_source,
        }),
        state: PhantomData,
    };
    // Isolated memory-admission fixture only: the actual earlier phase must
    // still reject these private reservations. No authority is synthesized.
    let live = source.live.as_mut().unwrap();
    live.next_global_ordinal = 27_176;
    live.next_purpose = GlobalLookupCommitmentPurposeV1::QMaskDigit;
    live.next_purpose_ordinal = 0;
    let (memory, file_memory) = QMaskFirstBlockMemoryV1::new_v1(live, &plan).unwrap();
    let (retry_memory, retry_file_memory) = QMaskFirstBlockMemoryV1::new_v1(live, &plan).unwrap();
    live.next_global_ordinal = 344;
    live.next_purpose = GlobalLookupCommitmentPurposeV1::ExistingDifferenceLow;
    live.next_purpose_ordinal = 0;
    let mut owner = RetainedSourceSessionV1::from_source_complete_v1(source).unwrap();
    assert!(matches!(
        owner.reserve_q_mask_first_memory_v1(&plan),
        Err(QMaskSErrorV1::Source)
    ));
    assert!(owner.phase.is_some());
    assert_eq!(drops.get(), 0);
    assert!(matches!(
        owner.sample_q_mask_first_block_v1(memory),
        Err(QMaskSErrorV1::Source)
    ));
    assert!(owner.phase.is_none());
    assert_eq!(drops.get(), 1);
    assert!(matches!(
        owner.sample_q_mask_first_block_v1(retry_memory),
        Err(QMaskSErrorV1::Source)
    ));
    assert!(owner.phase.is_none());
    assert_eq!(drops.get(), 1);
    drop((file_memory, retry_file_memory));
    assert_eq!(storage.test_usage_words_v1()[0], 66 * 16_400);
}

#[test]
#[cfg(unix)]
fn qmask_first_openings_retained_wrong_phase_consumes_only_after_admission_boundary() {
    super::super::q_mask_first_block_v1::with_first_openings_for_retained_refusal_v1(
        |block, file, admission| {
            let (source, _) = complete_source_v1();
            let mut owner = RetainedSourceSessionV1::from_source_complete_v1(source).unwrap();
            assert!(matches!(
                owner.admit_first_q_mask_openings_v1(&block, &file),
                Err(QMaskSErrorV1::Source)
            ));
            // A borrowed preflight failure must not consume an unrelated earlier
            // phase; the actual outer wrapper only retries Capacity, not Source.
            owner.require_low_digit_start_v1().unwrap();
            assert!(matches!(
                owner.produce_first_q_mask_openings_v1(block, admission),
                Err(QMaskSErrorV1::Source)
            ));
            assert!(owner.phase.is_none());
            assert!(owner.require_low_digit_start_v1().is_err());
            assert!(owner.begin_q_mask_kernel_v1().is_err());
        },
    );
}

#[test]
#[cfg(unix)]
fn qmask_s_stream_retained_wrong_phase_destroys_source_before_continuation_and_reentry() {
    super::super::q_mask_first_block_v1::with_s_stream_for_retained_refusal_v1(
        |stream, file, admission| {
            let (source, _) = complete_source_v1();
            let mut owner = RetainedSourceSessionV1::from_source_complete_v1(source).unwrap();
            assert!(matches!(
                owner.admit_next_q_mask_s_block_v1(&stream, &file),
                Err(QMaskSErrorV1::Source)
            ));
            owner.require_low_digit_start_v1().unwrap();
            let mut file = file.resume_next_block_v1().unwrap();
            assert!(matches!(
                owner.continue_q_mask_s_block_v1(stream, &mut file, admission),
                Err(QMaskSErrorV1::Source)
            ));
            assert!(owner.phase.is_none());
            assert!(owner.require_low_digit_start_v1().is_err());
            assert!(owner.begin_q_mask_kernel_v1().is_err());
        },
    );
}

#[test]
#[cfg(unix)]
fn qmask_complement_retained_wrong_phase_consumes_each_dispatch_and_rejects_reentry() {
    super::super::q_mask_first_block_v1::with_complement_for_retained_refusal_v1(
        |openings, file, complements| {
            for operation in 0..3 {
                let (source, _) = complete_source_v1();
                let mut owner = RetainedSourceSessionV1::from_source_complete_v1(source).unwrap();
                let result = match operation {
                    0 => owner
                        .begin_q_mask_complements_v1(openings, file)
                        .map(|_| ()),
                    1 => owner.produce_q_mask_complement_block_v1(openings, file, complements),
                    _ => owner.finish_q_mask_complements_v1(openings, file, complements),
                };
                assert_eq!(result, Err(QMaskSErrorV1::Source));
                assert!(owner.phase.is_none());
                assert!(owner.require_low_digit_start_v1().is_err());
                assert!(owner.begin_q_mask_kernel_v1().is_err());
                assert!(matches!(
                    owner.begin_q_mask_complements_v1(openings, file),
                    Err(QMaskSErrorV1::Source)
                ));
            }
        },
    );
}

#[test]
fn low_digit_workspace_original_phase_refusal_keeps_identity_rng_and_inventory() {
    use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::{LowDigitWorkspaceV1, LowDigitWorkspaceErrorV1};
    struct NeverRead(std::rc::Rc<core::cell::Cell<usize>>);
    impl crate::vega::MaskedRelaxedRandomSourceV1 for NeverRead {
        fn fill_bytes(
            &mut self,
            _: &mut [u8],
        ) -> Result<(), crate::vega::MaskedRelaxedRandomErrorV1> {
            self.0.set(self.0.get() + 1);
            panic!("workspace admission must precede entropy");
        }
    }
    // Prior source commitments remain an isolated inventory fixture, not a
    // production replay seal. The production entropy branch and ledger are real.
    let (mut fixture, points) = complete_source_v1();
    let live = fixture.live.take().unwrap();
    let calls = std::rc::Rc::new(core::cell::Cell::new(0));
    let source = GlobalLookupCommitmentSessionV1::<_, SourceOpeningCompleteStageV1> {
        live: Some(GlobalLookupCommitmentSessionLiveV1 {
            proof_resources: live.proof_resources,
            entropy: GlobalLookupProofSessionEntropySourceV1::Production {
                original_random: NeverRead(std::rc::Rc::clone(&calls)),
                commitment_entropy_bytes: 11_008,
                q_mask_entropy_bytes: 0,
            },
            inventory: live.inventory,
            proof_session_context_digest: live.proof_session_context_digest,
            source_opening_context_digest: live.source_opening_context_digest,
            next_global_ordinal: live.next_global_ordinal,
            next_purpose: live.next_purpose,
            next_purpose_ordinal: live.next_purpose_ordinal,
            pending_source: live.pending_source,
        }),
        state: PhantomData,
    };
    let mut owner = RetainedSourceSessionV1::from_source_complete_v1(source).unwrap();
    let Some(RetainedSourcePhaseV1::SourceComplete(session)) = owner.phase.as_mut() else {
        panic!("source phase")
    };
    let live = session.live.as_mut().unwrap();
    live.proof_resources
        .set_test_workspace_limit_v1(LowDigitWorkspaceV1::bytes_v1());
    let blocker = live.proof_resources.reserve_workspace_v1(1, 0).unwrap();
    for _ in 0..2 {
        assert!(matches!(
            owner.admit_low_digit_workspace_v1(),
            Err(LowDigitWorkspaceErrorV1::Capacity)
        ));
        owner
            .validate_completed_source_prefix_v1([0x53; 32], [0x32; 32], points, [0x52; 32])
            .unwrap();
        let Some(RetainedSourcePhaseV1::SourceComplete(session)) = owner.phase.as_ref() else {
            panic!("same source phase")
        };
        let live = session.live.as_ref().unwrap();
        assert_eq!(
            (live.next_global_ordinal, live.next_purpose_ordinal),
            (344, 0)
        );
        assert_eq!(
            live.next_purpose,
            GlobalLookupCommitmentPurposeV1::ExistingDifferenceLow
        );
        assert_eq!(
            live.inventory
                .slots
                .iter()
                .filter(|slot| slot.is_some())
                .count(),
            344
        );
        assert!(blocker.belongs_to_v1(&live.proof_resources));
        assert_eq!(live.proof_resources.live_bytes().unwrap(), 1);
        assert_eq!(live.proof_resources.consumed().unwrap(), 0);
        let GlobalLookupProofSessionEntropySourceV1::Production {
            commitment_entropy_bytes,
            q_mask_entropy_bytes,
            ..
        } = &live.entropy
        else {
            panic!("original entropy branch")
        };
        assert_eq!(
            (*commitment_entropy_bytes, *q_mask_entropy_bytes),
            (11_008, 0)
        );
    }
    drop(blocker);
    let memory = owner.admit_low_digit_workspace_v1().unwrap();
    let Some(RetainedSourcePhaseV1::SourceComplete(session)) = owner.phase.as_ref() else {
        panic!("same source phase")
    };
    let live = session.live.as_ref().unwrap();
    assert!(memory.belongs_to_v1(&live.proof_resources));
    assert_eq!(
        live.proof_resources.live_bytes().unwrap(),
        LowDigitWorkspaceV1::bytes_v1()
    );
    assert_eq!(calls.get(), 0);
    drop(memory);
    assert_eq!(live.proof_resources.live_bytes().unwrap(), 0);
}

#[test]
fn low_digit_workspace_wrong_or_poisoned_phase_cannot_reserve() {
    use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::LowDigitWorkspaceErrorV1;
    let (source, _) = complete_source_v1();
    let mut owner = RetainedSourceSessionV1::from_source_complete_v1(source).unwrap();
    let Some(RetainedSourcePhaseV1::SourceComplete(session)) = owner.phase.as_mut() else {
        panic!("source phase")
    };
    let live = session.live.as_mut().unwrap();
    live.next_global_ordinal += 1;
    assert!(matches!(
        owner.admit_low_digit_workspace_v1(),
        Err(LowDigitWorkspaceErrorV1::Source)
    ));
    let Some(RetainedSourcePhaseV1::SourceComplete(session)) = owner.phase.as_ref() else {
        panic!("same invalid owner")
    };
    assert_eq!(
        session
            .live
            .as_ref()
            .unwrap()
            .proof_resources
            .live_bytes()
            .unwrap(),
        0
    );
    drop(owner.phase.take());
    assert!(matches!(
        owner.admit_low_digit_workspace_v1(),
        Err(LowDigitWorkspaceErrorV1::Source)
    ));
    assert!(owner.phase.is_none());
}
