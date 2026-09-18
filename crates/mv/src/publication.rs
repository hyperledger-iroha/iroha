//! Opaque local identity for one storage owner's published current/undo pair.

use std::sync::{Arc, Mutex};

struct Owner;
struct Version;

/// Why a detached journal could not prepare its exact original publication.
/// These are local installation conditions, not consensus validity verdicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicationPreparationError<E> {
    /// An original current or undo writer is already held.
    Busy,
    /// The target owner or its jointly published current/undo pair changed.
    Changed,
    /// The caller refused the complete installation allocation/retention budget.
    Admission(E),
}

pub(crate) struct NextPublication(Arc<Version>);

impl NextPublication {
    /// Allocate the next identity before the first visible component is changed.
    pub(crate) fn new() -> Self {
        Self(Arc::new(Version))
    }
}

pub(crate) struct Publication {
    owner: Arc<Owner>,
    version: Mutex<Arc<Version>>,
}

pub(crate) struct CapturedPublication {
    owner: Arc<Owner>,
    version: Arc<Version>,
}

impl Publication {
    pub(crate) fn new() -> Self {
        Self {
            owner: Arc::new(Owner),
            version: Mutex::new(Arc::new(Version)),
        }
    }

    // Call only after acquiring the original current and undo writers. The
    // published pair cannot change while those writers remain owned.
    pub(crate) fn capture(&self) -> CapturedPublication {
        CapturedPublication {
            owner: Arc::clone(&self.owner),
            version: Arc::clone(&self.version.lock().expect("MV publication lock poisoned")),
        }
    }

    // Lock order is original data writers, then this identity lock. Observation
    // takes only this lock and must never acquire a data writer while holding it.
    // All mutation paths use this short lock across their actual publication.
    // A successor may acquire the data writers as commit releases them, but its
    // identity capture waits until BOTH publications and this rotation finish.
    pub(crate) fn publish(&self, publish: impl FnOnce()) {
        self.publish_prepared(NextPublication::new(), publish);
    }

    pub(crate) fn publish_prepared(&self, next: NextPublication, publish: impl FnOnce()) {
        let mut version = self.version.lock().expect("MV publication lock poisoned");
        publish();
        *version = next.0;
    }
}

impl CapturedPublication {
    pub(crate) fn matches(&self, publication: &Publication) -> bool {
        Arc::ptr_eq(&self.owner, &publication.owner)
            && Arc::ptr_eq(
                &self.version,
                &publication
                    .version
                    .lock()
                    .expect("MV publication lock poisoned"),
            )
    }

    pub(crate) fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.owner, &other.owner) && Arc::ptr_eq(&self.version, &other.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn identity_observation_is_excluded_until_both_actual_cell_writes_finish() {
        let cell = crate::cell::Cell::new(10_u64);
        let original = cell.publication.capture();
        let (first_written, first_read) = mpsc::channel();
        let (finish, finish_read) = mpsc::channel();
        std::thread::scope(|scope| {
            let owner = &cell;
            let publication = scope.spawn(move || {
                let mut undo = owner.revert.write();
                let mut current = owner.blocks.write();
                *undo.get_mut() = Some(10);
                *current.get_mut() = 20;
                owner.publication.publish(|| {
                    current.commit();
                    first_written.send(()).unwrap();
                    finish_read.recv().unwrap();
                    undo.commit();
                });
            });
            first_read.recv().unwrap();
            // Deliberately pause the same two-write owner between its writes.
            // Raw independent views may see that cut; an identity observer must
            // not receive the old token or the new token in that interval.
            let token_is_locked = cell.publication.version.try_lock().is_err();
            let visible_current = *cell.view();
            let visible_undo = *cell.predecessor_view();
            let observer = scope.spawn(|| original.matches(&cell.publication));
            finish.send(()).unwrap();
            publication.join().unwrap();
            assert!(!observer.join().unwrap());
            assert!(token_is_locked);
            assert_eq!(visible_current, 20);
            assert_eq!(visible_undo, None);
        });
        assert_eq!(*cell.view(), 20);
        assert_eq!(*cell.predecessor_view(), Some(10));
        assert!(!original.matches(&cell.publication));
    }
}
