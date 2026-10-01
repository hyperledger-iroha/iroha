//! Exact original publication waiter for coherent committed pair capture.

use super::*;
use crate::cell::{Cell, CommittedCellReadError};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

#[test]
fn original_publication_contention_retains_its_real_release_source() {
    let cell = Cell::new(1_u64);
    let expected = cell.publication.released.observe();
    let guard = cell.publication.lock_version();
    let error = cell
        .try_committed_view()
        .err()
        .expect("actual publication lock");
    assert_eq!(
        error,
        CommittedCellReadError::Publication(PublicationPreparationError::Busy(expected.clone()))
    );
    let mut wait = expected.wait_for_release();
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Pending);
    drop(guard);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Ready(()));
    assert!(cell.try_committed_view().is_ok());
}
