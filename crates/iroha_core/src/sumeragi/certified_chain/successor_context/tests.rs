//! Real successor history retains canonical sources and original refusal owners across retries.

use super::*;
use crate::{
    state::{World, WorldReadOnly},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_allocation::{AllocationRefusal, release::ReleaseRegistration};
use std::task::{Context, Poll, Waker};

fn chain() -> CertifiedTestChain {
    CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap()
}

fn context(
    chain: &CertifiedTestChain,
    time: u64,
) -> (IrohaHeader, GlobalThresholdBeaconPulseContextV1) {
    let proposal = chain.proposal(Some(time), Vec::new());
    let header = proposal.header();
    let view = chain.state().view();
    let tip = view.native_execution_tip().unwrap();
    let current = &view
        .world()
        .consensus_schedule()
        .ready(header.height().get())
        .unwrap()
        .epoch;
    (
        header,
        GlobalThresholdBeaconPulseContextV1 {
            instance: chain.instance().0,
            epoch: current.authorization.epoch,
            epoch_context_id: current.context_id().unwrap(),
            parent_consensus_hash: tip.core_hash().0,
            parent_result: tip.result().0,
        },
    )
}

fn pointers(owner: &SuccessorContextPreparation) -> [usize; 4] {
    let parent = owner.parent.as_ref().unwrap();
    [
        parent
            .carrier
            .acquisition
            .bytes_for_test()
            .unwrap()
            .as_slice()
            .as_ptr() as usize,
        std::ptr::from_ref::<SignedBlock>(parent.carrier.body.as_ref().unwrap()) as usize,
        parent
            .frame
            .committed
            .as_ref()
            .map_or(0, |slot| slot.as_slice().as_ptr() as usize),
        owner.genesis.as_ref().map_or(0, |genesis| {
            genesis
                .acquisition
                .bytes_for_test()
                .unwrap()
                .as_slice()
                .as_ptr() as usize
        }),
    ]
}

#[test]
fn successor_context_reuses_original_parent_and_genesis_at_full_pool_capacity() {
    let _retirement = crossbeam_epoch::pin();
    for parent_height in [1, 2] {
        let mut chain = chain();
        if parent_height == 2 {
            chain.commit_at(2_000, Vec::new());
        }
        let (header, expected) = context(&chain, (parent_height + 1) * 1_000);
        let view = chain.state().view();
        let pool = view.execution_budget();
        let baseline = pool.reserved_bytes();
        let mut original = SuccessorContextPreparation::default();
        original.authenticate(&view, &header, &expected).unwrap();
        let backing = pointers(&original);
        let retained = pool.reserved_bytes();
        assert!(retained > baseline);
        if parent_height == 1 {
            assert!(original.genesis.is_none());
            assert!(
                SharedSignedBlock::ptr_eq(
                    original.parent.as_ref().unwrap().frame.committed().block(),
                    original.genesis_authentication.as_ref().unwrap().genesis(),
                ),
                "genesis receipt and signed-body authentication must share the original physical carrier"
            );
        } else {
            assert_ne!(backing[0], backing[3]);
        }
        let held = pool
            .try_reserve_bytes(pool.limit_bytes() - retained)
            .unwrap();
        original.recheck(&view, &header, &expected).unwrap();
        original.authenticate(&view, &header, &expected).unwrap();
        assert_eq!(pointers(&original), backing);
        assert_eq!(pool.reserved_bytes(), pool.limit_bytes());
        drop(held);
        drop(original);
        assert_eq!(
            pool.reserved_bytes(),
            baseline,
            "cancellation refunds every original prepared stage"
        );
    }
}

#[test]
fn successor_context_late_receipt_refusal_retains_raw_source_without_self_wake() {
    let _retirement = crossbeam_epoch::pin();
    let chain = chain();
    let (header, expected) = context(&chain, 2_000);
    let view = chain.state().view();
    let pool = view.execution_budget();
    let baseline = pool.reserved_bytes();
    // Measure the actual canonical carrier's admitted children, excluding the receipt.
    let captured = Original::capture(&view).unwrap();
    let mut carrier = Carrier::new(&captured, 1, captured.genesis).unwrap();
    carrier.complete().unwrap();
    let carrier_bytes = pool.reserved_bytes() - baseline;
    drop(carrier);
    drop(captured);
    assert_eq!(pool.reserved_bytes(), baseline);
    let watch = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let mut credit = watch
        .try_reserve(ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut credit).unwrap();
    registration.bind_thread(std::thread::current());
    let held = pool
        .try_reserve_bytes(pool.limit_bytes() - baseline - carrier_bytes)
        .unwrap();
    let mut original = SuccessorContextPreparation::default();
    let Err(ExecutionAttemptError::Deferred(refusal)) =
        original.authenticate(&view, &header, &expected)
    else {
        panic!("the actual parent receipt must refuse after the original canonical carrier fits");
    };
    let Some(AllocationRefusal::Capacity {
        requested_bytes,
        release,
        ..
    }) = refusal.allocation_refusal()
    else {
        panic!("the original finite-pool receipt refusal must survive");
    };
    assert_eq!(
        *requested_bytes,
        std::alloc::Layout::array::<CommittedBlock>(1)
            .unwrap()
            .size()
    );
    assert_eq!(
        registration.poll_wait(release, &mut Context::from_waker(Waker::noop())),
        Poll::Pending
    );
    let backing = pointers(&original);
    assert!(!original.parent.as_ref().unwrap().complete);
    assert_eq!(pool.reserved_bytes(), pool.limit_bytes());
    drop(held);
    assert_eq!(
        registration.poll_wait(release, &mut Context::from_waker(Waker::noop())),
        Poll::Ready(())
    );
    original.authenticate(&view, &header, &expected).unwrap();
    let completed = pointers(&original);
    assert_eq!(
        &completed[..2],
        &backing[..2],
        "retry must retain the same canonical source and body"
    );
    drop(refusal);
    drop(original);
    assert_eq!(pool.reserved_bytes(), baseline);
}

#[test]
fn successor_context_physical_refusal_retains_original_raw_carrier_and_allocator_cause() {
    let _retirement = crossbeam_epoch::pin();
    let chain = chain();
    let (header, expected) = context(&chain, 2_000);
    let view = chain.state().view();
    let pool = view.execution_budget();
    let baseline = pool.reserved_bytes();
    let mut original = SuccessorContextPreparation::default();
    let layout = SharedSignedBlock::allocation_layout();
    let (result, actual) = crate::test_allocations::refuse_one_layout_during(layout, || {
        original.authenticate(&view, &header, &expected)
    });
    assert!(actual);
    let Err(ExecutionAttemptError::Deferred(refusal)) = result else {
        panic!("actual parent shell allocator refusal remains local");
    };
    assert!(
        matches!(refusal.allocator_refusal().unwrap().shared_from_charge_error(),
        Some(iroha_allocation::SharedFromChargeError::Allocator { layout: actual }) if *actual == layout)
    );
    assert!(refusal.allocation_refusal().is_none());
    let raw = original
        .parent
        .as_ref()
        .unwrap()
        .carrier
        .acquisition
        .bytes_for_test()
        .unwrap()
        .as_slice()
        .as_ptr();
    let occupied = pool.reserved_bytes();
    assert!(occupied >= baseline + layout.size());
    original.authenticate(&view, &header, &expected).unwrap();
    assert_eq!(
        original
            .parent
            .as_ref()
            .unwrap()
            .carrier
            .acquisition
            .bytes_for_test()
            .unwrap()
            .as_slice()
            .as_ptr(),
        raw
    );
    drop(refusal);
    drop(original);
    assert_eq!(pool.reserved_bytes(), baseline);
}

#[test]
fn successor_context_rechecks_current_cut_pool_source_and_every_context_field() {
    let _retirement = crossbeam_epoch::pin();
    let mut chain = chain();
    let (header, expected) = context(&chain, 2_000);
    let view = chain.state().view();
    let pool = view.execution_budget();
    let mut original = SuccessorContextPreparation::default();
    original.authenticate(&view, &header, &expected).unwrap();
    let backing = pointers(&original);
    for field in 0..5 {
        let mut changed = expected;
        match field {
            0 => changed.instance[0] ^= 1,
            1 => changed.epoch = changed.epoch.saturating_add(1),
            2 => changed.epoch_context_id[0] ^= 1,
            3 => changed.parent_consensus_hash[0] ^= 1,
            _ => changed.parent_result[0] ^= 1,
        }
        let ordinary =
            schedule::authenticate_successor_context(&view, &header, &changed).unwrap_err();
        let retained = original.recheck(&view, &header, &changed).unwrap_err();
        assert_eq!(
            ordinary.to_string(),
            retained.to_string(),
            "same policy predicate and error order for field {field}"
        );
    }
    let foreign_pool = AllocationBudget::new(pool.limit_bytes());
    let saved = std::mem::replace(
        &mut original.original.as_mut().unwrap().budget,
        foreign_pool.clone(),
    );
    assert!(matches!(
        original.recheck(&view, &header, &expected),
        Err(ExecutionAttemptError::Rejected(
            schedule::ScheduleError::CommittedSource(_)
        ))
    ));
    original.original.as_mut().unwrap().budget = saved;
    assert_eq!(foreign_pool.reserved_bytes(), 0);
    assert_eq!(pointers(&original), backing);
    let mut store = crate::kura::BlockStore::new(crate::kura::Kura::canonical_storage_path(
        &chain.kura().store_root(),
    ));
    let slot = store.read_block_index(0).unwrap();
    store
        .write_block_index(0, slot.start + 1, slot.length)
        .unwrap();
    assert!(
        original.recheck(&view, &header, &expected).is_err(),
        "changed physical slot cannot reuse captured authority"
    );
    store.write_block_index(0, slot.start, slot.length).unwrap();
    original.recheck(&view, &header, &expected).unwrap();
    assert_eq!(pointers(&original), backing);
    drop(view);
    chain.commit_at(2_000, Vec::new());
    let (next_header, next_context) = context(&chain, 3_000);
    let next = chain.state().view();
    assert!(
        matches!(
            original.recheck(&next, &next_header, &next_context),
            Err(ExecutionAttemptError::Rejected(
                schedule::ScheduleError::CommittedSource(_)
            ))
        ),
        "a new State cut must not reuse the earlier original preparation"
    );
    assert_eq!(pointers(&original), backing);
}

#[test]
fn successor_context_incomplete_or_wrong_header_precedes_history_admission() {
    let chain = chain();
    let (header, expected) = context(&chain, 2_000);
    let view = chain.state().view();
    let pool = view.execution_budget();
    let mut original = SuccessorContextPreparation::default();
    assert!(original.recheck(&view, &header, &expected).is_err());
    let wrong = chain.committed(1).block().header();
    let baseline = pool.reserved_bytes();
    let held = pool
        .try_reserve_bytes(pool.limit_bytes() - baseline)
        .unwrap();
    let ordinary = schedule::authenticate_successor_context(&view, &wrong, &expected).unwrap_err();
    let retained = original.authenticate(&view, &wrong, &expected).unwrap_err();
    assert_eq!(ordinary.to_string(), retained.to_string());
    assert!(original.original.is_none() && original.parent.is_none());
    drop(held);
}
