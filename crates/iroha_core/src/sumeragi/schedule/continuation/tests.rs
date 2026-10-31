//! Original continuation admission, rollback and stale-authority controls.

use super::*;
use crate::{
    state::{StateBlock, World},
    sumeragi::{
        certified_chain::SuccessorContextPreparation,
        schedule::execution::{CapturedExecution, CompletedExecution, ScheduleStep},
        test_chain::{CertifiedTestChain, TestChainConfig},
    },
};
use iroha_allocation::{AllocationRefusal, release::ReleaseRegistration};
use std::task::{Context, Poll, Waker};

fn chain() -> CertifiedTestChain {
    CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap()
}
fn expected(
    chain: &CertifiedTestChain,
    header: &BlockHeader,
) -> GlobalThresholdBeaconPulseContextV1 {
    let view = chain.state().view();
    let tip = view.native_execution_tip().unwrap();
    let epoch = &view
        .world()
        .consensus_schedule()
        .ready(header.height().get())
        .unwrap()
        .epoch;
    GlobalThresholdBeaconPulseContextV1 {
        instance: chain.instance().0,
        epoch: epoch.authorization.epoch,
        epoch_context_id: epoch.context_id().unwrap(),
        parent_consensus_hash: tip.core_hash().0,
        parent_result: tip.result().0,
    }
}
fn pulse(
    view: &(impl StateReadOnly + ?Sized),
    header: &BlockHeader,
    expected: GlobalThresholdBeaconPulseContextV1,
) -> epoch_beacon::VerifiedEpochPulse {
    epoch_beacon::capture(
        SumeragiRootScope::Global,
        view.world(),
        view.block_hashes(),
        &view
            .world()
            .consensus_schedule()
            .ready(header.height().get())
            .unwrap()
            .epoch,
        header.height().get(),
        None,
        Some(expected),
    )
    .unwrap()
}
fn pointers(value: &NativeExecutionInputs) -> [usize; 5] {
    let current = &value.schedule.current;
    let super::super::ScheduledSlot::Ready(next) = &value.schedule.next else {
        panic!("ordinary fixture has the actual ready next slot")
    };
    [
        current.committee.as_ptr() as usize,
        current.committee[0]
            .validator
            .public_key()
            .borrowed_parts()
            .unwrap()
            .1
            .as_ptr() as usize,
        current.committee[0].proof_of_possession.as_ptr() as usize,
        next.epoch.committee.as_ptr() as usize,
        next.epoch.committee[0]
            .validator
            .public_key()
            .borrowed_parts()
            .unwrap()
            .1
            .as_ptr() as usize,
    ]
}
fn poll(
    registration: &mut ReleaseRegistration,
    refusal: &crate::execution_attempt::ExecutionDeferred,
) -> Poll<()> {
    let Some(AllocationRefusal::Capacity { release, .. }) = refusal.allocation_refusal() else {
        panic!("original pool capacity cause must survive")
    };
    registration.poll_wait(release, &mut Context::from_waker(Waker::noop()))
}
fn original_graph_bytes(view: &(impl StateReadOnly + ?Sized), height: u64) -> usize {
    let schedule = view.world().consensus_schedule();
    let value = epoch_election::capture_continuation(
        &schedule.ready(height).unwrap().epoch,
        height,
        schedule.get(height + 1),
        ChainParamsRecord::from_parameters(view.world().parameters().sumeragi()),
        &view.execution_budget(),
    )
    .unwrap();
    value.allocation_bytes().unwrap()
}

#[test]
fn ordinary_continuation_capacity_refusal_retains_bank_until_original_external_release() {
    let _retirement = crossbeam_epoch::pin();
    let chain = chain();
    let proposal = chain.proposal(Some(2_000), Vec::new());
    let header = proposal.header();
    let expected = expected(&chain, &header);
    let view = chain.state().view();
    let pool = view.execution_budget();
    let baseline = pool.reserved_bytes();
    let graph_bytes = original_graph_bytes(&view, header.height().get());
    assert_eq!(pool.reserved_bytes(), baseline);
    let shell = ChargedShared::<Bank>::allocation_layout();
    let watch = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let mut watch_credit = watch
        .try_reserve(ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut watch_credit).unwrap();
    registration.bind_thread(std::thread::current());
    let external = pool
        .try_reserve_bytes(pool.limit_bytes() - baseline - shell.size() - graph_bytes + 1)
        .unwrap();
    let mut original = ContinuationPreparation::default();
    let Err(ExecutionAttemptError::Deferred(refusal)) =
        original.prepare(&view, &header, None, expected)
    else {
        panic!("actual complete continuation graph must refuse after its original shell fits")
    };
    assert!(
        matches!(refusal.allocation_refusal(), Some(AllocationRefusal::Capacity { requested_bytes, .. }) if *requested_bytes == graph_bytes)
    );
    let bank = original.bank.as_ref().unwrap().clone();
    assert!(matches!(bank.contents.lock().value, Value::Pending));
    let retained = pool.reserved_bytes();
    assert_eq!(
        retained,
        baseline + external.remaining_bytes() + shell.size()
    );
    for _ in 0..3 {
        assert_eq!(
            poll(&mut registration, &refusal),
            Poll::Pending,
            "original schedule graph refusal must not self-wake by discarding its successful bank"
        );
        assert!(matches!(
            original.prepare(&view, &header, None, expected),
            Err(ExecutionAttemptError::Deferred(_))
        ));
        assert_eq!(pool.reserved_bytes(), retained);
        assert!(ChargedShared::ptr_eq(
            original.bank.as_ref().unwrap(),
            &bank
        ));
    }
    let foreign = AllocationBudget::new(graph_bytes);
    drop(foreign.try_reserve_bytes(graph_bytes).unwrap());
    assert_eq!(poll(&mut registration, &refusal), Poll::Pending);
    drop(external);
    assert_eq!(poll(&mut registration, &refusal), Poll::Ready(()));
    let (result, replaced) = crate::test_allocations::refuse_one_layout_during(shell, || {
        original.prepare(&view, &header, None, expected)
    });
    result.unwrap();
    assert!(
        !replaced,
        "retry must reuse the original physical schedule bank"
    );
    assert!(ChargedShared::ptr_eq(
        original.bank.as_ref().unwrap(),
        &bank
    ));
    assert_eq!(pool.reserved_bytes(), baseline + shell.size() + graph_bytes);
    drop(bank);
    drop(original);
    drop(refusal);
    assert_eq!(
        pool.reserved_bytes(),
        baseline,
        "cancellation retires graph before all original charges"
    );
}

#[test]
fn ordinary_continuation_requested_and_done_rollback_reuse_exact_graph_at_full_capacity() {
    let _retirement = crossbeam_epoch::pin();
    let chain = chain();
    let proposal = chain.proposal(Some(2_000), Vec::new());
    let header = proposal.header();
    let expected = expected(&chain, &header);
    let view = chain.state().view();
    let pool = view.execution_budget();
    let mut original = ContinuationPreparation::default();
    original.prepare(&view, &header, None, expected).unwrap();
    let verified = pulse(&view, &header, expected);
    let watch = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let mut credit = watch
        .try_reserve(ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut credit).unwrap();
    registration.bind_thread(std::thread::current());
    let held = pool
        .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
        .unwrap();
    let refusal =
        crate::execution_attempt::ExecutionDeferred::from(pool.try_reserve_bytes(1).unwrap_err());
    assert_eq!(poll(&mut registration, &refusal), Poll::Pending);
    let lease = original
        .lend(&view, &header, SumeragiRootScope::Global, &verified)
        .unwrap();
    let physical = pointers(lease.get());
    let pristine = norito::encode_canonical(&lease.get().schedule).unwrap();
    let next = *lease.get().schedule.next.params();
    let mut altered = *lease.get().schedule.after_next.params();
    altered.max_block_bytes += 1;
    assert!(
        original
            .lend(&view, &header, SumeragiRootScope::Global, &verified)
            .is_err(),
        "one graph cannot be lent twice"
    );
    let retained = pool.reserved_bytes();
    // Requested rollback returns the actual lease, with no graph destruction or new admission.
    drop(lease);
    assert_eq!(
        poll(&mut registration, &refusal),
        Poll::Pending,
        "ordinary schedule rollback must retain its original graph credit"
    );
    assert_eq!(pool.reserved_bytes(), retained);
    let lease = original
        .lend(&view, &header, SumeragiRootScope::Global, &verified)
        .unwrap();
    assert_eq!(
        pointers(lease.get()),
        physical,
        "rollback must return the original physical continuation graph"
    );
    let done = CompletedExecution::Retained(lease.complete(altered, None));
    assert_eq!(done.get().schedule.next.params(), &next);
    assert_eq!(done.get().schedule.after_next.params(), &altered);
    let step = ScheduleStep::Done(done);
    drop(step); // A later output-seal failure abandons Done as well as Requested.
    assert_eq!(
        poll(&mut registration, &refusal),
        Poll::Pending,
        "completed schedule rollback must retain its original graph credit"
    );
    assert_eq!(pool.reserved_bytes(), retained);
    let key_layout = view
        .world()
        .consensus_schedule()
        .ready(header.height().get())
        .unwrap()
        .epoch
        .committee[0]
        .validator
        .public_key()
        .retained_allocation_layout();
    let (lease, replaced) = crate::test_allocations::refuse_one_layout_during(key_layout, || {
        original
            .lend(&view, &header, SumeragiRootScope::Global, &verified)
            .unwrap()
    });
    assert!(
        !replaced,
        "rollback cannot allocate a replacement epoch public key"
    );
    assert_eq!(
        pointers(lease.get()),
        physical,
        "Done rollback must return the same original continuation graph"
    );
    assert_eq!(
        norito::encode_canonical(&lease.get().schedule).unwrap(),
        pristine
    );
    assert_eq!(lease.get().beacon, None);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _same_original = lease.complete(altered, None);
        panic!("abandon original schedule after later finalizer unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(
        poll(&mut registration, &refusal),
        Poll::Pending,
        "unwinding a later finalizer cannot refund the retained original schedule graph"
    );
    let lease = original
        .lend(&view, &header, SumeragiRootScope::Global, &verified)
        .unwrap();
    assert_eq!(pointers(lease.get()), physical);
    assert_eq!(
        norito::encode_canonical(&lease.get().schedule).unwrap(),
        pristine
    );
    let completed = CompletedExecution::Retained(lease.complete(altered, None)).into_inputs();
    assert_eq!(pointers(completed.get()), physical);
    assert!(completed.belongs_to(&pool));
    assert!(
        original.prepare(&view, &header, None, expected).is_err(),
        "consumed output cannot reopen its original capture"
    );
    drop(held);
    drop(completed);
}

fn requested<'a>(block: &'a StateBlock<'_>) -> &'a NativeExecutionInputs {
    match &block.sumeragi_schedule {
        ScheduleStep::Requested {
            captured: CapturedExecution::Retained(value),
            ..
        } => value.get(),
        _ => panic!("actual native request must hold its original continuation lease"),
    }
}

#[test]
fn ordinary_continuation_native_world_request_and_done_drop_return_original_custody() {
    let _retirement = crossbeam_epoch::pin();
    let chain = chain();
    let proposal = chain.proposal(Some(2_000), Vec::new());
    let header = proposal.header();
    let expected = expected(&chain, &header);
    let mut original = SuccessorContextPreparation::default();
    {
        let view = chain.state().view();
        original.authenticate(&view, &header, &expected).unwrap();
        original
            .prepare_continuation(&view, &header, None, expected)
            .unwrap();
    }
    let first = chain
        .state()
        .block_with_pristine_carrier_stage(&proposal, |block| {
            block.request_sumeragi_schedule_with_preparation(
                1,
                &proposal,
                None,
                Some(expected),
                Some(&original),
            )
        })
        .unwrap();
    let backing = pointers(requested(&first));
    let pristine = norito::encode_canonical(&requested(&first).schedule).unwrap();
    drop(first);
    let mut second = chain
        .state()
        .block_with_pristine_carrier_stage(&proposal, |block| {
            block.request_sumeragi_schedule_with_preparation(
                1,
                &proposal,
                None,
                Some(expected),
                Some(&original),
            )
        })
        .unwrap();
    assert_eq!(
        pointers(requested(&second)),
        backing,
        "World rollback must return its original continuation graph"
    );
    second.advance_requested_sumeragi_schedule().unwrap();
    let ScheduleStep::Done(done) = &second.sumeragi_schedule else {
        panic!("actual schedule finalizer must complete")
    };
    assert_eq!(pointers(done.get()), backing);
    drop(second);
    let mut third = chain
        .state()
        .block_with_pristine_carrier_stage(&proposal, |block| {
            block.request_sumeragi_schedule_with_preparation(
                1,
                &proposal,
                None,
                Some(expected),
                Some(&original),
            )
        })
        .unwrap();
    assert_eq!(
        pointers(requested(&third)),
        backing,
        "post-finalizer rollback must return original schedule custody"
    );
    assert_eq!(
        norito::encode_canonical(&requested(&third).schedule).unwrap(),
        pristine
    );
    third.advance_requested_sumeragi_schedule().unwrap();
    let complete = third.take_sumeragi_execution_inputs().unwrap();
    assert_eq!(pointers(complete.get()), backing);
    assert!(complete.belongs_to(&chain.state().ivm_execution_budget()));
    drop(complete);
    drop(third);
}

#[test]
fn ordinary_continuation_refuses_changed_source_cut_params_root_pulse_and_foreign_shell() {
    let _retirement = crossbeam_epoch::pin();
    let mut chain = chain();
    let proposal = chain.proposal(Some(2_000), Vec::new());
    let header = proposal.header();
    let expected = expected(&chain, &header);
    let view = chain.state().view();
    let pool = view.execution_budget();
    let mut original = ContinuationPreparation::default();
    original.prepare(&view, &header, None, expected).unwrap();
    let verified = pulse(&view, &header, expected);
    let bank = original.bank.as_ref().unwrap().clone();
    {
        let mut contents = bank.contents.lock();
        contents.original.params.max_block_bytes += 1;
    }
    assert!(
        original
            .lend(&view, &header, SumeragiRootScope::Global, &verified)
            .is_err()
    );
    bank.contents.lock().original.params.max_block_bytes -= 1;
    let saved_schedule = {
        let mut contents = bank.contents.lock();
        std::mem::take(&mut contents.original.schedule)
    };
    assert!(
        original
            .lend(&view, &header, SumeragiRootScope::Global, &verified)
            .is_err()
    );
    bank.contents.lock().original.schedule = saved_schedule;
    let wrong_scope = SumeragiRootScope::Dataspace {
        parent_network_id: *view.network_id(),
        dataspace_id: iroha_model_base::topology::DataSpaceId::new(7),
    };
    assert!(
        original
            .lend(&view, &header, wrong_scope, &verified)
            .is_err()
    );
    let unexpected_pulse = FinalizedGlobalThresholdBeaconPulseV1 {
        version: iroha_data_model::consensus::GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        network_id: *view.network_id(),
        session_id: [1; 32],
        roster_hash: [2; 32],
        transcript_hash: [3; 32],
        context: expected,
        height: header.height().get(),
        round: 0,
        finalized_chain_anchor: iroha_data_model::consensus::GlobalThresholdBeaconChainAnchorV1 {
            height: header.height().get() - 1,
            block_hash: header.prev_block_hash().unwrap(),
        },
        signature: [4; 48],
        seed: [5; 32],
        pulse_id: [6; 32],
    };
    assert!(
        matches!(
            original.prepare(&view, &header, Some(unexpected_pulse), expected),
            Err(ExecutionAttemptError::Rejected(_))
        ),
        "fresh ordinary-height pulse validation cannot be replaced by cached capture authority"
    );
    let other = chain.proposal(Some(3_000), Vec::new()).header();
    assert!(
        original
            .lend(&view, &other, SumeragiRootScope::Global, &verified)
            .is_err()
    );
    let mut wrong_expected = expected;
    wrong_expected.parent_result[0] ^= 1;
    let mut authentic = SuccessorContextPreparation::default();
    authentic.authenticate(&view, &header, &expected).unwrap();
    assert!(
        authentic
            .prepare_continuation(&view, &header, None, wrong_expected)
            .is_err()
    );
    // A real original-pool shell is required even if the offered fixed source is identical.
    let foreign = AllocationBudget::new(ChargedShared::<Bank>::allocation_layout().size());
    let mut reservation = foreign
        .try_reserve(ChargedShared::<Bank>::allocation_layout())
        .unwrap();
    let foreign_bank = ChargedShared::from_reservation(
        Bank {
            pool: foreign.clone(),
            contents: Mutex::new(Contents {
                original: Original {
                    header,
                    schedule: view.world().consensus_schedule().clone(),
                    params: ChainParamsRecord::from_parameters(
                        view.world().parameters().sumeragi(),
                    ),
                    root_scope: SumeragiRootScope::Global,
                    pulse: None,
                    link: None,
                },
                value: Value::Pending,
            }),
        },
        &mut reservation,
    )
    .unwrap_or_else(|(_, error)| panic!("foreign fixture shell: {error}"));
    let foreign_owner = ContinuationPreparation {
        bank: Some(foreign_bank),
    };
    assert!(
        matches!(foreign_owner.lend(&view, &header, SumeragiRootScope::Global, &verified),
        Err(ExecutionAttemptError::Deferred(reason)) if reason.reason() == ivm::error::ExecutionDeferral::LocalInvariantViolation)
    );
    assert!(
        original
            .lend(&view, &header, SumeragiRootScope::Global, &verified)
            .is_ok()
    );
    assert!(bank.belongs_to(&pool));
    // The same bank cannot grant authority after the committed parent cut advances.
    drop(view);
    drop(foreign_owner);
    drop(authentic);
    chain.commit_at(2_000, Vec::new());
    let next_header = chain.proposal(Some(3_000), Vec::new()).header();
    let next_expected = self::expected(&chain, &next_header);
    let next = chain.state().view();
    let next_pulse = pulse(&next, &next_header, next_expected);
    assert!(
        original
            .lend(&next, &next_header, SumeragiRootScope::Global, &next_pulse)
            .is_err()
    );
    assert!(
        original
            .prepare(&next, &next_header, None, next_expected)
            .is_err()
    );
    assert!(
        matches!(bank.contents.lock().value, Value::Available(_)),
        "stale cut refusal retains the original graph for cancellation"
    );
    drop(bank);
    drop(original);
}

#[test]
fn ordinary_continuation_physical_shell_refusal_preserves_original_error_and_graph_refusal_retains_shell()
 {
    let _retirement = crossbeam_epoch::pin();
    let chain = chain();
    let proposal = chain.proposal(Some(2_000), Vec::new());
    let header = proposal.header();
    let expected = expected(&chain, &header);
    let view = chain.state().view();
    let pool = view.execution_budget();
    let baseline = pool.reserved_bytes();
    let layout = ChargedShared::<Bank>::allocation_layout();
    let mut original = ContinuationPreparation::default();
    let (result, refused) = crate::test_allocations::refuse_one_layout_during(layout, || {
        original.prepare(&view, &header, None, expected)
    });
    assert!(refused);
    let Err(ExecutionAttemptError::Deferred(refusal)) = result else {
        panic!("physical shell refusal stays local")
    };
    assert!(refusal.allocator_refusal().is_some());
    assert!(refusal.allocation_refusal().is_none());
    assert!(original.bank.is_none());
    assert_eq!(pool.reserved_bytes(), baseline);
    let key = view
        .world()
        .consensus_schedule()
        .ready(header.height().get())
        .unwrap()
        .epoch
        .committee[0]
        .validator
        .public_key()
        .retained_allocation_layout();
    let (result, refused) = crate::test_allocations::refuse_one_layout_during(key, || {
        original.prepare(&view, &header, None, expected)
    });
    assert!(refused);
    assert!(matches!(result, Err(ExecutionAttemptError::Deferred(_))));
    assert!(matches!(
        original.bank.as_ref().unwrap().contents.lock().value,
        Value::Pending
    ));
    assert_eq!(pool.reserved_bytes(), baseline + layout.size());
    original.prepare(&view, &header, None, expected).unwrap();
    drop(original);
    drop(refusal);
    assert_eq!(pool.reserved_bytes(), baseline);
}
