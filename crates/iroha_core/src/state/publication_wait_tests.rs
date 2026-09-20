//! Publication waiters require a stable committed frontier and retain no State fence.
use super::*;
use futures::poll;
use std::task::Poll;

fn state() -> State {
    State::new_for_testing(
        World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    )
}

fn header(height: u64) -> BlockHeader {
    BlockHeader::new(NonZeroU64::new(height).unwrap(), None, None, 0, 0)
}

#[tokio::test]
async fn published_frontier_is_ready_without_a_new_notification() {
    let state = state();
    state.append_committed_block_header_for_tests(header(1));
    let wait = state.wait_for_committed_height(1);
    tokio::pin!(wait);
    assert_eq!(poll!(wait.as_mut()), Poll::Ready(()));
}

#[tokio::test]
async fn all_waiters_observe_only_the_complete_publication() {
    let state = state();
    let first = state.wait_for_committed_height(1);
    let second = state.wait_for_committed_height(1);
    tokio::pin!(first, second);
    assert!(poll!(first.as_mut()).is_pending());
    assert!(poll!(second.as_mut()).is_pending());
    assert!(state.state_commit_lock.try_lock().is_some());
    assert!(state.state_write_lock.try_lock().is_some());

    let publication = state.begin_state_view_write();
    let mut hashes = state.block_hashes.block();
    hashes.push_for_tests(header(1).hash());
    hashes.commit_for_tests();
    assert_eq!(state.committed_height(), 1);
    assert!(
        poll!(first.as_mut()).is_pending(),
        "odd generation is not published"
    );
    assert!(poll!(second.as_mut()).is_pending());
    drop(publication);
    assert_eq!(poll!(first.as_mut()), Poll::Ready(()));
    assert_eq!(poll!(second.as_mut()), Poll::Ready(()));
}

#[tokio::test]
async fn a_wakeup_below_the_required_height_cannot_authorize_progress() {
    let state = state();
    let wait = state.wait_for_committed_height(2);
    tokio::pin!(wait);
    assert!(poll!(wait.as_mut()).is_pending());
    drop(state.begin_state_view_write());
    assert!(poll!(wait.as_mut()).is_pending());
    state.append_committed_block_header_for_tests(header(1));
    assert!(poll!(wait.as_mut()).is_pending());
    state.append_committed_block_header_for_tests(header(2));
    assert_eq!(poll!(wait.as_mut()), Poll::Ready(()));
}

#[tokio::test]
async fn cancelling_one_waiter_does_not_consume_another_waiters_publication() {
    let state = state();
    let mut cancelled = Box::pin(state.wait_for_committed_height(1));
    let retained = state.wait_for_committed_height(1);
    tokio::pin!(retained);
    assert!(poll!(cancelled.as_mut()).is_pending());
    assert!(poll!(retained.as_mut()).is_pending());
    drop(cancelled);
    state.append_committed_block_header_for_tests(header(1));
    assert_eq!(poll!(retained.as_mut()), Poll::Ready(()));
    assert!(state.state_commit_lock.try_lock().is_some());
}

#[tokio::test]
async fn publication_wakes_every_registered_task() {
    use std::{
        future::Future,
        sync::atomic::{AtomicUsize, Ordering},
        task::{Context, Wake, Waker},
    };
    struct WakeCount(AtomicUsize);
    impl Wake for WakeCount {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let state = state();
    let first_count = Arc::new(WakeCount(AtomicUsize::new(0)));
    let second_count = Arc::new(WakeCount(AtomicUsize::new(0)));
    let first_waker = Waker::from(first_count.clone());
    let second_waker = Waker::from(second_count.clone());
    let first = state.wait_for_committed_height(1);
    let second = state.wait_for_committed_height(1);
    tokio::pin!(first, second);
    assert!(
        first
            .as_mut()
            .poll(&mut Context::from_waker(&first_waker))
            .is_pending()
    );
    assert!(
        second
            .as_mut()
            .poll(&mut Context::from_waker(&second_waker))
            .is_pending()
    );
    state.append_committed_block_header_for_tests(header(1));
    assert!(first_count.0.load(Ordering::Relaxed) > 0);
    assert!(second_count.0.load(Ordering::Relaxed) > 0);
    assert!(
        first
            .as_mut()
            .poll(&mut Context::from_waker(&first_waker))
            .is_ready()
    );
    assert!(
        second
            .as_mut()
            .poll(&mut Context::from_waker(&second_waker))
            .is_ready()
    );
}
