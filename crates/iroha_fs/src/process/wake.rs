//! Runtime-independent bounded polling wakeups; this helper never owns process authority.

use std::{
    future::poll_fn,
    io,
    sync::{Arc, Condvar, Mutex},
    task::{Poll, Waker},
    thread,
    time::Duration,
};

#[derive(Default)]
struct State {
    stopped: bool,
    ready: bool,
    waker: Option<Waker>,
}

type Shared = (Mutex<State>, Condvar);

pub(super) struct PollWake(Arc<Shared>);

impl PollWake {
    pub(super) fn new() -> io::Result<Self> {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let worker = Arc::clone(&shared);
        thread::Builder::new()
            .name("iroha-process-wake".into())
            .spawn(move || {
                let (state, changed) = &*worker;
                let mut state = state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                loop {
                    state = changed
                        .wait_while(state, |state| !state.stopped && state.waker.is_none())
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if state.stopped {
                        break;
                    }
                    // Sleep only while a task is waiting. Cancellation wakes this wait
                    // immediately; no original process or admission owner is locked here.
                    let (next, _) = changed
                        .wait_timeout_while(state, Duration::from_millis(20), |state| {
                            !state.stopped
                        })
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    state = next;
                    if state.stopped {
                        break;
                    }
                    state.ready = true;
                    let waker = state.waker.take();
                    drop(state);
                    if let Some(waker) = waker {
                        waker.wake();
                    }
                    state = worker
                        .0
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
            })?;
        Ok(Self(shared))
    }

    pub(super) async fn tick(&mut self) {
        poll_fn(|context| {
            let (state, changed) = &*self.0;
            let mut state = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.ready {
                state.ready = false;
                return Poll::Ready(());
            }
            state.waker = Some(context.waker().clone());
            changed.notify_one();
            Poll::Pending
        })
        .await
    }
}

impl Drop for PollWake {
    fn drop(&mut self) {
        let (state, changed) = &*self.0;
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.stopped = true;
        state.waker = None;
        changed.notify_one();
    }
}
