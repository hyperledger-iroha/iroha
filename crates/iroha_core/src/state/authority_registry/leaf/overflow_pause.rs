//! Test-only one-shot synchronization at an actual canonical writer overflow.

use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    rc::Rc,
    sync::mpsc::{Receiver, SyncSender},
    time::Duration,
};

struct Pause {
    reached: SyncSender<()>,
    resume: Receiver<()>,
}

thread_local! {
    static PAUSE: RefCell<Option<Pause>> = const { RefCell::new(None) };
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static OBSERVED: Cell<bool> = const { Cell::new(false) };
}

/// Thread-bound cleanup of an observer even if encoding or coordination unwinds.
pub(in crate::state) struct OverflowPauseGuard {
    _thread: PhantomData<Rc<()>>,
}

impl OverflowPauseGuard {
    /// Whether the real bounded writer consumed this thread's one-shot observer.
    pub(in crate::state) fn observed(&self) -> bool {
        OBSERVED.with(Cell::get)
    }
}

impl Drop for OverflowPauseGuard {
    fn drop(&mut self) {
        PAUSE.with(|slot| slot.borrow_mut().take());
        OBSERVED.with(|observed| observed.set(false));
        ACTIVE.with(|active| active.set(false));
    }
}

/// Pause the next real overflow without supplying an error or replacing the encoder.
/// Channels must be constructed before capture; a nested observer cannot replace its owner.
pub(in crate::state) fn pause_next_overflow(
    reached: SyncSender<()>,
    resume: Receiver<()>,
) -> OverflowPauseGuard {
    assert!(!ACTIVE.with(Cell::get), "nested bounded-writer observer");
    ACTIVE.with(|active| active.set(true));
    OBSERVED.with(|observed| observed.set(false));
    PAUSE.with(|slot| {
        *slot.borrow_mut() = Some(Pause { reached, resume });
    });
    OverflowPauseGuard {
        _thread: PhantomData,
    }
}

pub(super) fn observe_overflow() {
    // Take the observer and release the RefCell borrow before any synchronization.
    let Some(pause) = PAUSE.with(|slot| slot.borrow_mut().take()) else {
        return;
    };
    OBSERVED.with(|observed| observed.set(true));
    pause
        .reached
        .send(())
        .expect("report actual writer overflow");
    pause
        .resume
        .recv_timeout(Duration::from_secs(30))
        .expect("resume actual writer overflow");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{panic::catch_unwind, sync::mpsc};

    #[test]
    fn observer_cleanup_preserves_outer_owner_and_restores_after_unwind() {
        let (reached, _observed) = mpsc::sync_channel(0);
        let (_resume, waiting) = mpsc::sync_channel(0);
        let original = pause_next_overflow(reached, waiting);
        let nested = catch_unwind(|| {
            let (reached, _observed) = mpsc::sync_channel(0);
            let (_resume, waiting) = mpsc::sync_channel(0);
            pause_next_overflow(reached, waiting)
        });
        assert!(nested.is_err());
        assert!(ACTIVE.with(Cell::get));
        assert!(PAUSE.with(|slot| slot.borrow().is_some()));
        assert!(!original.observed());
        drop(original);
        assert!(!ACTIVE.with(Cell::get));
        assert!(PAUSE.with(|slot| slot.borrow().is_none()));
        let unwind = catch_unwind(|| {
            let (reached, _observed) = mpsc::sync_channel(0);
            let (_resume, waiting) = mpsc::sync_channel(0);
            let _original = pause_next_overflow(reached, waiting);
            panic!("abandon an unconsumed observer");
        });
        assert!(unwind.is_err());
        assert!(!ACTIVE.with(Cell::get));
        assert!(!OBSERVED.with(Cell::get));
        assert!(PAUSE.with(|slot| slot.borrow().is_none()));
    }
}
