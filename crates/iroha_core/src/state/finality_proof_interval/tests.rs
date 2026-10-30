//! Actual publisher release and versioned State append custody for finite proof work.
use super::*;
use crate::sumeragi::{
    certified_chain::{CertifiedChain, QcVerification, relation_counts},
    test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_data_model::sumeragi_finality::MAX_FINALITY_BLOCK_BYTES;
use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::Arc,
};

fn test_budget(frames: iroha_allocation::AllocationBudget) -> NativeFinalityProofBudget {
    NativeFinalityProofBudget::new(
        frames,
        iroha_allocation::AllocationBudget::new(33_312_848),
        norito::DecodeLimits::new(
            2_562_487,
            MAX_FINALITY_BLOCK_BYTES,
            33_312_848,
            33_312_848,
            norito::core::MAX_VALUE_NESTING_DEPTH,
        ),
    )
}

fn limits(height: u64) -> NativeFinalityProofIntervalLimits {
    NativeFinalityProofIntervalLimits::new(
        NonZeroU64::new(2).unwrap(),
        NonZeroU64::new(height).unwrap(),
        NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES).unwrap(),
        NonZeroUsize::new(usize::MAX).unwrap(),
        Instant::now() + Duration::from_secs(900),
    )
    .unwrap()
}
fn context() -> norito::core::DecodeBudgetContext {
    norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
        1_000_000,
        MAX_FINALITY_BLOCK_BYTES,
        8_000_000,
        usize::MAX,
        128,
    ))
}
fn chain() -> CertifiedTestChain {
    chain_of(4)
}
fn chain_of(height: u64) -> CertifiedTestChain {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
    while chain.height() < height {
        chain.commit(Vec::new());
    }
    chain
}

#[test]
fn interval_busy_capture_returns_exact_writer_release_without_work_or_refund_under_guard() {
    use std::{
        future::Future,
        task::{Context, Waker},
    };
    // Keep retired fixture World versions from refunding this exact State pool
    // during the request-work comparisons. Publisher release and physical
    // response retirement below do not depend on epoch reclamation.
    let _epoch = crossbeam_epoch::pin();
    let chain = chain();
    let state = chain.state();
    let pool = state.ivm_execution_budget();
    let counter = context();
    // Response allocations have their actual request owner; persistent checkpoints
    // and the publisher release registration retain the original State pool.
    let frames = iroha_allocation::AllocationBudget::new(pool.limit_bytes());
    let owner = test_budget(frames.clone());
    let mut registration = crate::unit_test_support::release_registration(&pool);
    // The original waiter is fixture-owned; measure request work after its admission.
    let reserved = pool.reserved_bytes();
    chain.kura().reset_canonical_query_reads_for_test();
    let wait = state.with_held_view_publication_for_reader_test(|expected| {
        let result = counter.with(|| {
            state.read_finality_proof_interval(
                &owner,
                &limits(chain.height()),
                &AtomicBool::new(false),
            )
        });
        let Err(FinalityProofIntervalReadError::StateView(StateViewError::Busy(original))) = result
        else {
            panic!("finite proof capture must preserve the actual held publisher's Busy release");
        };
        assert_eq!(original, expected);
        assert_eq!(pool.reserved_bytes(), reserved);
        assert_eq!(frames.reserved_bytes(), 0);
        assert!(state.finality_prefix_checkpoints.heights().is_empty());
        assert_eq!(counter.consumed_allocated_bytes(), 0);
        assert_eq!(chain.kura().canonical_query_reads_for_test(), (0, 0));
        let mut pending = std::pin::pin!(original.clone().wait_for_release(&mut registration));
        assert!(
            pending
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        original
    });
    let mut completed = std::pin::pin!(wait.wait_for_release(&mut registration));
    assert!(
        completed
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
    let retained = state
        .read_finality_proof_interval(&owner, &limits(chain.height()), &AtomicBool::new(false))
        .unwrap();
    assert!(pool.reserved_bytes() > reserved);
    assert!(frames.reserved_bytes() > 0);
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4]);
    let retained_state_bytes = pool.reserved_bytes();
    assert_eq!(
        retained.proof(2).unwrap().block_header.hash(),
        chain.committed(4).block_hash()
    );
    drop(retained);
    assert_eq!(
        frames.reserved_bytes(),
        0,
        "the response owner physically retires"
    );
    assert_eq!(pool.reserved_bytes(), retained_state_bytes);
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4]);
}

#[test]
fn interval_original_hash_generation_allows_actual_stable_tip_publication() {
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let pool = state.ivm_execution_budget();
    let original = chain.committed(4).block_hash();
    let owner = test_budget(pool);
    let retained = state
        .read_finality_proof_interval_after_read(
            &owner,
            &limits(4),
            &AtomicBool::new(false),
            None,
            || {
                // Real signed commit executes/publishes while the original versioned hash
                // generation is retained. There is no enclosing World or writer guard.
                chain.commit(Vec::new());
            },
        )
        .unwrap();
    assert_eq!(state.committed_height(), 5);
    assert_eq!(retained.proof(2).unwrap().block_header.hash(), original);
    assert_eq!(retained.len(), 3);
}

#[test]
fn interval_post_read_writer_refusal_retires_response_and_preserves_original_release() {
    let chain = chain();
    let state = Arc::clone(chain.state());
    // The writer may retire node-cache originals; measure this request alone.
    let pool = iroha_allocation::AllocationBudget::new(2_562_487);
    let owner = test_budget(pool.clone());
    let reserved = pool.reserved_bytes();
    let (request_tx, request_rx) = std::sync::mpsc::sync_channel(1);
    let (held_tx, held_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let (result, expected) = std::thread::scope(|scope| {
        let state = Arc::clone(&state);
        let worker = scope.spawn(move || {
            if request_rx.recv_timeout(Duration::from_secs(5)).is_err() {
                return;
            }
            state.with_held_view_publication_for_reader_test(|wait| {
                held_tx.send(wait).unwrap();
                // Natural cleanup even if the producer unexpectedly unwinds.
                let _ = release_rx.recv_timeout(Duration::from_secs(5));
            });
        });
        let mut expected = None;
        let result = chain.state().read_finality_proof_interval_after_read(
            &owner,
            &limits(4),
            &AtomicBool::new(false),
            None,
            || {
                request_tx.send(()).unwrap();
                expected = Some(held_rx.recv_timeout(Duration::from_secs(5)).unwrap());
            },
        );
        let _ = release_tx.send(());
        worker.join().unwrap();
        (result, expected)
    });
    let Err(FinalityProofIntervalReadError::StateView(StateViewError::Busy(original))) = result
    else {
        panic!(
            "finite interval must reject an actual publishing State owner after native verification and source join"
        );
    };
    assert_eq!(Some(original), expected);
    assert_eq!(
        pool.reserved_bytes(),
        reserved,
        "refused response values retire before the original pool refund"
    );
    let retained = state
        .read_finality_proof_interval(&owner, &limits(4), &AtomicBool::new(false))
        .unwrap();
    drop(retained);
    assert_eq!(pool.reserved_bytes(), reserved);
}

fn waiter(pool: &iroha_allocation::AllocationBudget) -> StateViewWaiter {
    let mut reservation = pool
        .try_reserve(StateViewWaiter::allocation_layout())
        .unwrap();
    StateViewWaiter::for_current_thread(&mut reservation).unwrap()
}
fn limits_until(height: u64, deadline: Instant) -> NativeFinalityProofIntervalLimits {
    NativeFinalityProofIntervalLimits::new(
        NonZeroU64::new(2).unwrap(),
        NonZeroU64::new(height).unwrap(),
        NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES).unwrap(),
        NonZeroUsize::new(usize::MAX).unwrap(),
        deadline,
    )
    .unwrap()
}

#[test]
fn waiting_interval_captures_again_once_a_busy_publication_completes_within_its_deadline() {
    let chain = chain();
    let state = chain.state();
    let frames = iroha_allocation::AllocationBudget::new(2_562_487);
    let owner = test_budget(frames.clone());
    let waiters = iroha_allocation::AllocationBudget::new(4096);
    chain.kura().reset_canonical_query_reads_for_test();
    let (result, waiting, reads_while_held) = std::thread::scope(|scope| {
        let (reader, waiting, reads_while_held) =
            state.with_held_view_publication_for_reader_test(|_| {
                // The reader starts while this publication is held: its capture refuses.
                let reader = scope.spawn(|| {
                    let mut waiter = waiter(&waiters);
                    state
                        .read_finality_proof_interval_waiting(
                            &owner,
                            &limits_until(4, Instant::now() + Duration::from_secs(60)),
                            &AtomicBool::new(false),
                            &mut waiter,
                        )
                        .map(|interval| interval.proof(2).unwrap().block_header.hash())
                        .map_err(|error| error.to_string())
                });
                std::thread::sleep(Duration::from_millis(100));
                let waiting = !reader.is_finished();
                (
                    reader,
                    waiting,
                    chain.kura().canonical_query_reads_for_test(),
                )
            });
        (reader.join().unwrap(), waiting, reads_while_held)
    });
    assert!(
        waiting,
        "a held publication is waited out instead of refused"
    );
    assert_eq!(
        reads_while_held,
        (0, 0),
        "no proof work starts while it is held"
    );
    assert_eq!(result, Ok(chain.committed(4).block_hash()));
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4]);
    assert_eq!(frames.reserved_bytes(), 0);
    assert_eq!(waiters.reserved_bytes(), 0);
}

#[test]
fn waiting_interval_keeps_the_original_busy_release_once_its_deadline_passes() {
    let chain = chain();
    let state = chain.state();
    let frames = iroha_allocation::AllocationBudget::new(2_562_487);
    let owner = test_budget(frames.clone());
    let waiters = iroha_allocation::AllocationBudget::new(4096);
    let mut waiter = waiter(&waiters);
    let wait = Duration::from_millis(150);
    chain.kura().reset_canonical_query_reads_for_test();
    let (result, expected, waited) = state.with_held_view_publication_for_reader_test(|expected| {
        // This thread holds the publication, so only the deadline can end the wait.
        let started = Instant::now();
        let result = state.read_finality_proof_interval_waiting(
            &owner,
            &limits_until(4, started + wait),
            &AtomicBool::new(false),
            &mut waiter,
        );
        (result, expected, started.elapsed())
    });
    let Err(FinalityProofIntervalReadError::StateView(StateViewError::Busy(original))) = result
    else {
        panic!("an expired wait must keep the publisher's original Busy release");
    };
    assert_eq!(original, expected);
    assert!(waited >= wait, "the reader waited out its whole deadline");
    assert_eq!(chain.kura().canonical_query_reads_for_test(), (0, 0));
    assert!(state.finality_prefix_checkpoints.heights().is_empty());
    assert_eq!(frames.reserved_bytes(), 0);
    assert!(waiters.reserved_bytes() > 0);
    drop(waiter);
    assert_eq!(waiters.reserved_bytes(), 0);
}

#[test]
fn waiting_interval_repeats_only_a_busy_recheck_and_keeps_its_single_walk() {
    let chain = chain();
    let state = Arc::clone(chain.state());
    let frames = iroha_allocation::AllocationBudget::new(2_562_487);
    let owner = test_budget(frames.clone());
    let waiters = iroha_allocation::AllocationBudget::new(4096);
    let mut waiter = waiter(&waiters);
    let (request_tx, request_rx) = std::sync::mpsc::sync_channel(1);
    let (held_tx, held_rx) = std::sync::mpsc::sync_channel(1);
    let (result, counts) = std::thread::scope(|scope| {
        let holder = {
            let state = Arc::clone(&state);
            scope.spawn(move || {
                if request_rx.recv_timeout(Duration::from_secs(5)).is_err() {
                    return;
                }
                state.with_held_view_publication_for_reader_test(|_| {
                    held_tx.send(()).unwrap();
                    // Retire after the reader's recheck has met this publication.
                    std::thread::sleep(Duration::from_millis(100));
                });
            })
        };
        let read = relation_counts::measure(|| {
            state.read_finality_proof_interval_after_read(
                &owner,
                &limits_until(4, Instant::now() + Duration::from_secs(60)),
                &AtomicBool::new(false),
                Some(&mut waiter),
                || {
                    request_tx.send(()).unwrap();
                    held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                },
            )
        });
        holder.join().unwrap();
        read
    });
    let retained = result.expect("the verified interval returns once its recheck completes");
    assert_eq!(
        counts.qcs,
        [2, 3, 4],
        "the walk ran once; only its recheck repeated"
    );
    assert_eq!(
        retained.proof(2).unwrap().block_header.hash(),
        chain.committed(4).block_hash()
    );
    drop(retained);
    assert_eq!(frames.reserved_bytes(), 0);
}

#[test]
fn waiting_interval_stops_when_its_request_stops_and_unparks_the_worker() {
    let chain = chain();
    let state = chain.state();
    let owner = test_budget(iroha_allocation::AllocationBudget::new(2_562_487));
    let waiters = iroha_allocation::AllocationBudget::new(4096);
    let stop = AtomicBool::new(false);
    let stopped = std::thread::scope(|scope| {
        state.with_held_view_publication_for_reader_test(|_| {
            let reader = scope.spawn(|| {
                let mut waiter = waiter(&waiters);
                matches!(
                    state.read_finality_proof_interval_waiting(
                        &owner,
                        &limits_until(4, Instant::now() + Duration::from_secs(60)),
                        &stop,
                        &mut waiter,
                    ),
                    Err(FinalityProofIntervalReadError::Proof(
                        NativeFinalityProofIntervalError::Cancelled
                    ))
                )
            });
            std::thread::sleep(Duration::from_millis(50));
            stop.store(true, Ordering::Release);
            reader.thread().unpark();
            // The publication is still held: only the stop can end this wait.
            reader.join().unwrap()
        })
    });
    assert!(
        stopped,
        "a stopped request ends its wait with its stop policy"
    );
    assert_eq!(waiters.reserved_bytes(), 0);
}

fn single(height: u64) -> NativeFinalityProofIntervalLimits {
    NativeFinalityProofIntervalLimits::new(
        NonZeroU64::new(height).unwrap(),
        NonZeroU64::new(height).unwrap(),
        NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES).unwrap(),
        NonZeroUsize::new(usize::MAX).unwrap(),
        Instant::now() + Duration::from_secs(900),
    )
    .unwrap()
}
fn ladder(
    state: &State,
    owner: &NativeFinalityProofBudget,
    height: u64,
    step: u64,
) -> Result<FundedLadderReceipt, FinalityProofIntervalReadError> {
    state.read_certified_ladder_receipt_in_steps(
        owner,
        NonZeroU64::new(height).unwrap(),
        NonZeroUsize::new(usize::MAX).unwrap(),
        Instant::now() + Duration::from_secs(900),
        &AtomicBool::new(false),
        step,
    )
}

#[test]
fn ladder_receipt_climbs_cold_history_in_retained_steps_then_resumes_at_its_tip() {
    let mut chain = chain_of(9);
    let state = Arc::clone(chain.state());
    let owner = test_budget(iroha_allocation::AllocationBudget::new(2_562_487));
    // A cold ladder climbs from signed genesis, retaining every completed step.
    let (receipt, counts) = relation_counts::measure(|| ladder(&state, &owner, 9, 3));
    assert_eq!(
        counts.qcs,
        [2, 3, 4, 5, 6, 7, 8, 9],
        "each height verifies once"
    );
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4, 7, 9]);
    assert_eq!(
        receipt.unwrap().as_ref().block_hash(),
        chain.committed(9).block_hash()
    );
    // The next tip verifies only the heights committed after the retained one.
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let (receipt, counts) = relation_counts::measure(|| ladder(&state, &owner, 11, 3));
    assert_eq!(counts.qcs, [10, 11]);
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4, 7, 9, 11]);
    assert_eq!(
        receipt.unwrap().as_ref().block_hash(),
        chain.committed(11).block_hash()
    );
}

#[test]
fn ladder_receipt_keeps_completed_steps_when_a_later_step_fails() {
    let chain = chain_of(9);
    let state = chain.state();
    let owner = test_budget(iroha_allocation::AllocationBudget::new(2_562_487));
    // Height 12 is not committed: the steps to 4 and 7 complete before the step to 10 fails.
    assert!(ladder(state, &owner, 12, 3).is_err());
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4, 7]);
    // A retry resumes above the retained steps instead of at signed genesis.
    let (receipt, counts) = relation_counts::measure(|| ladder(state, &owner, 9, 3));
    assert_eq!(counts.qcs, [8, 9]);
    assert_eq!(
        receipt.unwrap().as_ref().block_hash(),
        chain.committed(9).block_hash()
    );
}

#[test]
fn ladder_receipt_is_the_receipt_a_cold_genesis_walk_certifies() {
    let chain = chain_of(6);
    let state = chain.state();
    let owner = test_budget(iroha_allocation::AllocationBudget::new(2_562_487));
    let retained_receipt = ladder(state, &owner, 6, 2).unwrap();
    let receipt = retained_receipt.as_ref();
    let view = state.view();
    let reader = CertifiedChain::new(&view).unwrap();
    let cold = reader.certified(6).unwrap();
    assert_eq!(cold.verification(), QcVerification::Verified);
    let cold = cold.into_committed();
    assert_eq!(receipt.block_hash(), cold.block_hash());
    assert_eq!(receipt.core_hash(), cold.core_hash());
    assert_eq!(receipt.result(), cold.result());
    assert_eq!(receipt.id(), cold.id());
}

#[test]
fn ladder_receipt_join_refuses_another_heights_frame() {
    let chain = chain();
    let owner = test_budget(iroha_allocation::AllocationBudget::new(2_562_487));
    let interval = chain
        .state()
        .read_finality_proof_interval(&owner, &single(4), &AtomicBool::new(false))
        .unwrap();
    let proof = interval.proof(0).unwrap();
    assert!(receipt_carries_verified_wire(&chain.committed(4), proof));
    assert!(!receipt_carries_verified_wire(&chain.committed(3), proof));
}


#[test]
fn ladder_completed_interval_cancellation_refuses_before_receipt_read() {
    let chain = chain();
    let state = chain.state();
    let frames = iroha_allocation::AllocationBudget::new(2_562_487);
    let owner = test_budget(frames.clone());
    let limits = single(4);
    let interval = state
        .read_finality_proof_interval(&owner, &limits, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4]);
    chain.kura().reset_canonical_query_reads_for_test();
    let result = state.finish_certified_ladder_receipt(
        &owner,
        &interval,
        &limits,
        &AtomicBool::new(true),
    );
    assert!(matches!(
        result,
        Err(FinalityProofIntervalReadError::Proof(
            NativeFinalityProofIntervalError::Cancelled
        ))
    ));
    assert_eq!(chain.kura().canonical_query_reads_for_test(), (0, 0));
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4]);
    drop(interval);
    assert_eq!(frames.reserved_bytes(), 0);
}

#[test]
fn ladder_completed_interval_deadline_refuses_before_receipt_read() {
    let chain = chain();
    let state = chain.state();
    let frames = iroha_allocation::AllocationBudget::new(2_562_487);
    let owner = test_budget(frames.clone());
    let interval = state
        .read_finality_proof_interval(&owner, &single(4), &AtomicBool::new(false))
        .unwrap();
    let expired = NativeFinalityProofIntervalLimits::new(
        NonZeroU64::new(4).unwrap(),
        NonZeroU64::new(4).unwrap(),
        NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES).unwrap(),
        NonZeroUsize::new(usize::MAX).unwrap(),
        Instant::now(),
    )
    .unwrap();
    chain.kura().reset_canonical_query_reads_for_test();
    let result = state.finish_certified_ladder_receipt(
        &owner,
        &interval,
        &expired,
        &AtomicBool::new(false),
    );
    assert!(matches!(
        result,
        Err(FinalityProofIntervalReadError::Proof(
            NativeFinalityProofIntervalError::Deadline
        ))
    ));
    assert_eq!(chain.kura().canonical_query_reads_for_test(), (0, 0));
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4]);
    drop(interval);
    assert_eq!(frames.reserved_bytes(), 0);
}

#[test]
fn ladder_completed_interval_busy_receipt_preserves_original_publisher_release() {
    let chain = chain();
    let state = Arc::clone(chain.state());
    let frames = iroha_allocation::AllocationBudget::new(2_562_487);
    let owner = test_budget(frames.clone());
    let limits = single(4);
    let interval = state
        .read_finality_proof_interval(&owner, &limits, &AtomicBool::new(false))
        .unwrap();
    chain.kura().reset_canonical_query_reads_for_test();
    let (held_tx, held_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let (result, expected) = std::thread::scope(|scope| {
        let held_state = Arc::clone(&state);
        let publisher = scope.spawn(move || {
            held_state.with_held_view_publication_for_reader_test(|original| {
                held_tx.send(original).unwrap();
                // Natural bounded release even if a regression retries the Busy view.
                let _ = release_rx.recv_timeout(Duration::from_secs(5));
            });
        });
        let expected = held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let result = state.finish_certified_ladder_receipt(
        &owner,
            &interval,
            &limits,
            &AtomicBool::new(false),
        );
        let _ = release_tx.send(());
        publisher.join().unwrap();
        (result, expected)
    });
    let Err(FinalityProofIntervalReadError::StateView(StateViewError::Busy(original))) = result
    else {
        panic!("completed proof receipt must preserve the actual publisher's Busy release");
    };
    assert_eq!(original, expected);
    assert_eq!(chain.kura().canonical_query_reads_for_test(), (0, 0));
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4]);
    drop(interval);
    assert_eq!(frames.reserved_bytes(), 0);
}


#[test]
fn ladder_receipt_retains_original_frame_and_graph_owners_until_drop() {
    let chain = chain();
    let frames = iroha_allocation::AllocationBudget::new(2_562_487);
    let graphs = iroha_allocation::AllocationBudget::new(33_312_848);
    let owner = NativeFinalityProofBudget::new(
        frames.clone(), graphs.clone(),
        norito::DecodeLimits::new(2_562_487, MAX_FINALITY_BLOCK_BYTES,
            33_312_848, 33_312_848, norito::core::MAX_VALUE_NESTING_DEPTH),
    );
    let receipt = ladder(chain.state(), &owner, 4, 3).unwrap();
    assert_eq!(receipt.as_ref().id(), chain.committed(4).id());
    assert!(frames.reserved_bytes() > 0);
    assert!(graphs.reserved_bytes() > 0);
    drop(receipt);
    assert_eq!(frames.reserved_bytes(), 0);
    assert_eq!(graphs.reserved_bytes(), 0);
}

#[test]
fn ladder_receipt_refuses_exhausted_original_frame_pool_without_state_fallback() {
    let chain = chain();
    let state = chain.state();
    let frames = iroha_allocation::AllocationBudget::new(2_562_487);
    let owner = test_budget(frames.clone());
    let limits = single(4);
    let interval = state.read_finality_proof_interval(&owner, &limits, &AtomicBool::new(false)).unwrap();
    let remaining = frames.limit_bytes() - frames.reserved_bytes();
    assert!(remaining > 0);
    let original = frames.try_reserve(std::alloc::Layout::array::<u8>(remaining).unwrap()).unwrap();
    let result = state.finish_certified_ladder_receipt(&owner, &interval, &limits, &AtomicBool::new(false));
    assert!(matches!(result, Err(FinalityProofIntervalReadError::Proof(NativeFinalityProofIntervalError::Deferred(_)))));
    drop(original);
    let receipt = state.finish_certified_ladder_receipt(&owner, &interval, &limits, &AtomicBool::new(false)).unwrap();
    assert_eq!(receipt.as_ref().id(), chain.committed(4).id());
    drop(receipt);
    drop(interval);
    assert_eq!(frames.reserved_bytes(), 0);
}

#[test]
fn ladder_receipt_refuses_exhausted_original_graph_pool_before_source_read() {
    let chain = chain();
    let state = chain.state();
    let frames = iroha_allocation::AllocationBudget::new(2_562_487);
    let graphs = iroha_allocation::AllocationBudget::new(33_312_848);
    let owner = NativeFinalityProofBudget::new(
        frames.clone(), graphs.clone(),
        norito::DecodeLimits::new(2_562_487, MAX_FINALITY_BLOCK_BYTES,
            33_312_848, 33_312_848, norito::core::MAX_VALUE_NESTING_DEPTH),
    );
    let limits = single(4);
    let interval = state.read_finality_proof_interval(&owner, &limits, &AtomicBool::new(false)).unwrap();
    assert_eq!(graphs.reserved_bytes(), 0);
    let original = graphs.try_reserve(std::alloc::Layout::array::<u8>(graphs.limit_bytes()).unwrap()).unwrap();
    chain.kura().reset_canonical_query_reads_for_test();
    let result = state.finish_certified_ladder_receipt(&owner, &interval, &limits, &AtomicBool::new(false));
    assert!(matches!(result, Err(FinalityProofIntervalReadError::Proof(NativeFinalityProofIntervalError::Deferred(_)))));
    assert_eq!(chain.kura().canonical_query_reads_for_test(), (0, 0));
    drop(original);
    drop(interval);
    assert_eq!(frames.reserved_bytes(), 0);
    assert_eq!(graphs.reserved_bytes(), 0);
}

#[test]
fn ladder_receipt_rejects_advanced_tip_before_receipt_ancestry_work() {
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let frames = iroha_allocation::AllocationBudget::new(2_562_487);
    let owner = test_budget(frames.clone());
    let limits = single(4);
    let interval = state.read_finality_proof_interval(&owner, &limits, &AtomicBool::new(false)).unwrap();
    chain.commit(Vec::new());
    chain.kura().reset_canonical_query_reads_for_test();
    let result = state.finish_certified_ladder_receipt(&owner, &interval, &limits, &AtomicBool::new(false));
    assert!(matches!(result, Err(FinalityProofIntervalReadError::Proof(NativeFinalityProofIntervalError::Source { height: 4 }))));
    assert_eq!(chain.kura().canonical_query_reads_for_test(), (0, 0));
    drop(interval);
    assert_eq!(frames.reserved_bytes(), 0);
}

#[test]
fn ladder_same_tip_reverifies_the_gap_below_its_strict_checkpoint() {
    let chain = chain_of(6);
    let state = chain.state();
    let owner = test_budget(iroha_allocation::AllocationBudget::new(2_562_487));
    drop(ladder(state, &owner, 6, 2).unwrap());
    assert_eq!(state.finality_prefix_checkpoints.heights(), [3, 5, 6]);
    let (receipt, counts) = relation_counts::measure(|| ladder(state, &owner, 6, 2));
    assert_eq!(counts.qcs, [6]);
    assert_eq!(receipt.unwrap().as_ref().id(), chain.committed(6).id());
}

fn interval(from: u64, to: u64) -> NativeFinalityProofIntervalLimits {
    NativeFinalityProofIntervalLimits::new(
        NonZeroU64::new(from).unwrap(),
        NonZeroU64::new(to).unwrap(),
        NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES).unwrap(),
        NonZeroUsize::new(usize::MAX).unwrap(),
        Instant::now() + Duration::from_secs(900),
    )
    .unwrap()
}

#[test]
fn cold_interval_climbs_in_retained_ladder_steps_and_a_warm_read_verifies_only_its_gap() {
    let chain = chain_of(2 * LADDER_STEP + 12);
    let state = chain.state();
    let tip = chain.height();
    let owner = test_budget(iroha_allocation::AllocationBudget::new(2_562_487));
    let read = |from: u64, to: u64| {
        state.read_finality_proof_interval(&owner, &interval(from, to), &AtomicBool::new(false))
    };
    // A height this State has not committed refuses before any ladder work.
    let (refused, _) = relation_counts::measure(|| read(10 * tip, 10 * tip));
    assert!(refused.is_err());
    assert!(state.finality_prefix_checkpoints.heights().is_empty());
    // A restarted node retains no prefix: its first read near the tip climbs from
    // signed genesis, one retained step at a time, and verifies each height once.
    let started = Instant::now();
    let (cold, counts) = relation_counts::measure(|| read(tip - 9, tip));
    let cold_elapsed = started.elapsed();
    let cold = cold.unwrap();
    assert_eq!(counts.qcs, (2..=tip).collect::<Vec<_>>());
    assert_eq!(
        state.finality_prefix_checkpoints.heights(),
        [1 + LADDER_STEP, 1 + 2 * LADDER_STEP, tip]
    );
    for (index, height) in (tip - 9..=tip).enumerate() {
        assert_eq!(
            cold.proof(index).unwrap().block_header.hash(),
            chain.committed(height).block_hash()
        );
    }
    drop(cold);
    // Once warm, a read verifies only the frames above its nearest retained prefix.
    let started = Instant::now();
    let (warm, counts) = relation_counts::measure(|| read(tip - 1, tip));
    let warm_elapsed = started.elapsed();
    assert_eq!(counts.qcs, (2 * LADDER_STEP + 2..=tip).collect::<Vec<_>>());
    assert_eq!(
        warm.unwrap().proof(1).unwrap().block_header.hash(),
        chain.committed(tip).block_hash()
    );
    eprintln!(
        "finality ladder at height {tip}: cold read verified {} heights in {cold_elapsed:?}, \
         warm read {} heights in {warm_elapsed:?}",
        tip - 1,
        tip - 2 * LADDER_STEP - 1,
    );
}

#[test]
fn interval_climb_keeps_its_completed_steps_when_stopped() {
    let chain = chain_of(9);
    let state = chain.state();
    let owner = test_budget(iroha_allocation::AllocationBudget::new(2_562_487));
    let cancelled = AtomicBool::new(false);
    // A withdrawn request or an expired deadline stops between the climb's steps.
    let result = state.climb_finality_ladder(&owner, &single(9), &cancelled, 3, None, || {
        cancelled.store(true, std::sync::atomic::Ordering::Release);
    });
    assert!(matches!(
        result,
        Err(FinalityProofIntervalReadError::Proof(
            NativeFinalityProofIntervalError::Cancelled
        ))
    ));
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4]);
    // The next read resumes above the kept step instead of at signed genesis.
    let (proofs, counts) = relation_counts::measure(|| {
        state.read_finality_proof_interval(&owner, &single(9), &AtomicBool::new(false))
    });
    assert_eq!(counts.qcs, [5, 6, 7, 8, 9]);
    assert_eq!(
        proofs.unwrap().proof(0).unwrap().block_header.hash(),
        chain.committed(9).block_hash()
    );
}

#[test]
fn interval_climb_keeps_its_completed_steps_through_a_busy_publication() {
    let chain = chain_of(9);
    let state = Arc::clone(chain.state());
    let owner = test_budget(iroha_allocation::AllocationBudget::new(2_562_487));
    let (request_tx, request_rx) = std::sync::mpsc::sync_channel(1);
    let (held_tx, held_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let (result, expected) = std::thread::scope(|scope| {
        let publisher = Arc::clone(&state);
        let worker = scope.spawn(move || {
            if request_rx.recv_timeout(Duration::from_secs(5)).is_err() {
                return;
            }
            publisher.with_held_view_publication_for_reader_test(|wait| {
                held_tx.send(wait).unwrap();
                let _ = release_rx.recv_timeout(Duration::from_secs(5));
            });
        });
        let mut expected = None;
        // The first step's walk retains its prefix; then an actual publication
        // makes that step's final capture Busy.
        let result = state.climb_finality_ladder(
            &owner,
            &single(9),
            &AtomicBool::new(false),
            3,
            None,
            || {
                if expected.is_none() {
                    request_tx.send(()).unwrap();
                    expected = Some(held_rx.recv_timeout(Duration::from_secs(5)).unwrap());
                }
            },
        );
        let _ = release_tx.send(());
        worker.join().unwrap();
        (result, expected)
    });
    let Err(FinalityProofIntervalReadError::StateView(StateViewError::Busy(original))) = result
    else {
        panic!("a busy step must return the actual publisher's release");
    };
    assert_eq!(Some(original), expected);
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4]);
    // After that release the retry resumes above the step the busy read completed.
    let (proofs, counts) = relation_counts::measure(|| {
        state.read_finality_proof_interval(&owner, &single(9), &AtomicBool::new(false))
    });
    assert_eq!(counts.qcs, [5, 6, 7, 8, 9]);
    assert_eq!(
        proofs.unwrap().proof(0).unwrap().block_header.hash(),
        chain.committed(9).block_hash()
    );
}

#[test]
fn waiting_climb_waits_out_a_busy_step_and_verifies_each_height_once() {
    let chain = chain_of(9);
    let state = Arc::clone(chain.state());
    let owner = test_budget(iroha_allocation::AllocationBudget::new(2_562_487));
    let waiters = iroha_allocation::AllocationBudget::new(4096);
    let mut waiter = waiter(&waiters);
    let (request_tx, request_rx) = std::sync::mpsc::sync_channel(1);
    let (held_tx, held_rx) = std::sync::mpsc::sync_channel(1);
    let (result, counts) = std::thread::scope(|scope| {
        let holder = {
            let state = Arc::clone(&state);
            scope.spawn(move || {
                if request_rx.recv_timeout(Duration::from_secs(5)).is_err() {
                    return;
                }
                state.with_held_view_publication_for_reader_test(|_| {
                    held_tx.send(()).unwrap();
                    // Retire after the first step's recheck has met this publication.
                    std::thread::sleep(Duration::from_millis(100));
                });
            })
        };
        let mut first = true;
        let climbed = relation_counts::measure(|| {
            state.climb_finality_ladder(
                &owner,
                &single(9),
                &AtomicBool::new(false),
                3,
                Some(&mut waiter),
                || {
                    if std::mem::take(&mut first) {
                        request_tx.send(()).unwrap();
                        held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    }
                },
            )
        });
        holder.join().unwrap();
        climbed
    });
    result.expect("a waiting climb outlasts a busy step within its deadline");
    // Steps to 4 and 7; the busy step repeated only its recheck, never its walk.
    assert_eq!(counts.qcs, [2, 3, 4, 5, 6, 7]);
    assert_eq!(state.finality_prefix_checkpoints.heights(), [4, 7]);
    drop(waiter);
    assert_eq!(waiters.reserved_bytes(), 0);
}
