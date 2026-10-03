//! Current readers preserve their original physical source and enclosing release custody.

use super::*;
use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

#[test]
fn original_current_reader_retains_busy_and_defers_unlock_notification() {
    let release_budget = iroha_allocation::AllocationBudget::new(
        2 * iroha_allocation::release::ReleaseRegistration::allocation_layout().size(),
    );
    let mut release_registration_1 = crate::release_test_support::registration(&release_budget);
    let mut release_registration_2 = crate::release_test_support::registration(&release_budget);

    let target = Storage::from_iter([(1_u64, 10_u64)]);
    let foreign = Storage::from_iter([(1_u64, 20_u64)]);
    let mut releases = target.reader_releases();
    let held = target.blocks.write().prepare_commit();
    let expected = target.observe_reader_release();
    let error = target.try_view_retaining(&mut releases).err().unwrap();
    assert!(matches!(error, PublicationPreparationError::Busy(ref wait) if wait == &expected));
    let mut pending = Box::pin(expected.wait_for_release(&mut release_registration_1));
    let mut cx = Context::from_waker(Waker::noop());
    drop(foreign.view());
    assert!(pending.as_mut().poll(&mut cx).is_pending());
    drop(held);
    assert_eq!(pending.as_mut().poll(&mut cx), Poll::Ready(()));
    let mut foreign_releases = foreign.reader_releases();
    assert!(matches!(
        target.try_view_retaining(&mut foreign_releases),
        Err(PublicationPreparationError::Changed)
    ));
    let view_release = target.observe_reader_release();
    let mut pending = Box::pin(view_release.wait_for_release(&mut release_registration_2));
    let view = target.try_view_retaining(&mut releases).unwrap();
    assert_eq!(view.get(&1), Some(&10));
    drop(view);
    assert!(pending.as_mut().poll(&mut cx).is_pending());
    drop(releases);
    assert_eq!(pending.as_mut().poll(&mut cx), Poll::Ready(()));
}
