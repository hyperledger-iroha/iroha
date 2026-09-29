//! Local original-pool refusal through the actual borrowed World projection.

use super::*;
use crate::state::World;
use mv::allocation::AllocationRefusal;
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

#[test]
fn cold_world_projection_returns_typed_original_admission_without_a_baseline() {
    let world = World::default();
    let block = world.block();
    let budget = AllocationBudget::new(0);
    assert!(matches!(
        WorldStateBaseline::capture_current(&block, &budget),
        Err(WorldBaselineError::Update(MerkleMapError::Admission(
            AllocationRefusal::ExceedsLimit { limit_bytes: 0, .. }
        )))
    ));
    assert_eq!(budget.reserved_bytes(), 0);
    budget.set_limit_bytes(1024 * 1024);
    let baseline = WorldStateBaseline::capture_current(&block, &budget).unwrap();
    assert!(budget.reserved_bytes() > 0);
    drop(baseline);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_overlay_refusal_keeps_parent_and_original_release_through_final_clone() {
    let world = World::default();
    let mut block = world.block();
    let budget = AllocationBudget::new(1024 * 1024);
    let parent = WorldStateBaseline::capture_current(&block, &budget).unwrap();
    let root = parent.root();
    let retained = budget.reserved_bytes();
    let borrowed = parent.clone();
    *block.soradns_last_publish_ms.get_mut() = Some(42);
    budget.set_limit_bytes(retained);
    let Err(WorldBaselineError::Update(MerkleMapError::Admission(AllocationRefusal::Capacity {
        release,
        ..
    }))) = parent.apply_block(&block)
    else {
        panic!("new World nodes must retain the original pool refusal");
    };
    assert_eq!(parent.root(), root);
    assert_eq!(borrowed.root(), root);
    assert_eq!(budget.reserved_bytes(), retained);
    assert_eq!(*block.soradns_last_publish_ms.get(), Some(42));
    let mut wait = pin!(release.wait_for_release());
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
    let unrelated = AllocationBudget::new(1);
    drop(unrelated.try_reserve_bytes(1).unwrap());
    drop(parent);
    assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
    drop(borrowed);
    assert_eq!(wait.as_mut().poll(&mut context), Poll::Ready(()));
    assert_eq!(budget.reserved_bytes(), 0);
    budget.set_limit_bytes(1024 * 1024);
    let retry = WorldStateBaseline::capture_current(&block, &budget).unwrap();
    assert_ne!(retry.root(), root);
    drop(retry);
    drop(block);
    assert_eq!(*world.block().soradns_last_publish_ms.get(), None);
    assert_eq!(budget.reserved_bytes(), 0);
}
