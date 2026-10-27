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
