//! Opaque local identity for one storage owner's published current/undo pair.

use crate::{BlockMode, ReleaseGuard, ReleaseNotification, ReleaseWait};
use std::sync::{Arc, Mutex, TryLockError};

struct Owner;
struct Version;

/// Why a detached journal could not prepare its exact original publication.
/// These are local installation conditions, not consensus validity verdicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicationPreparationError<E> {
    /// An original writer or the joint publication identity is already held.
    Busy(ReleaseWait),
    /// An original writer or joint publication panicked; reconstruct this local owner.
    Poisoned,
    /// The target owner or its jointly published current/undo pair changed.
    Changed,
    /// The caller refused the complete installation allocation/retention budget.
    Admission(E),
}

/// Successful preparation or the exact original journal and its local refusal.
/// The error retains custody so a caller can defer without rebuilding execution.
pub type PublicationPreparationResult<Prepared, Journal, E> =
    Result<Prepared, (Journal, PublicationPreparationError<E>)>;

impl<E> PublicationPreparationError<E> {
    /// Classify failed physical acquisition using its pre-probe observation.
    ///
    /// A writer that unwound may have poisoned an underlying lock whose try API
    /// reports only absence. Such an owner needs reconstruction, not another
    /// wait for a release that already happened and can never happen again.
    pub fn after_failed_acquisition(wait: ReleaseWait) -> Self {
        if wait.is_poisoned() {
            Self::Poisoned
        } else {
            Self::Busy(wait)
        }
    }
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
    released: ReleaseNotification,
}

pub(crate) struct CapturedPublication {
    owner: Arc<Owner>,
    version: Arc<Version>,
}

/// Opaque local equality of a block's original owner, predecessor and mode.
///
/// Capturing this identity borrows no values and acquires no locks. Equality
/// remains stable while a block stages changes; publication rotates the captured
/// current/undo predecessor even if values remain equal. This observation grants
/// no mutation or publication authority and is not a portable state commitment.
pub struct BlockPublicationIdentity {
    predecessor: CapturedPublication,
    mode: BlockMode,
}

impl BlockPublicationIdentity {
    pub(crate) fn capture(predecessor: &CapturedPublication, mode: BlockMode) -> Self {
        Self {
            predecessor: CapturedPublication {
                owner: Arc::clone(&predecessor.owner),
                version: Arc::clone(&predecessor.version),
            },
            mode,
        }
    }
}

impl std::fmt::Debug for BlockPublicationIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BlockPublicationIdentity")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl PartialEq for BlockPublicationIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.mode == other.mode && self.predecessor.same_as(&other.predecessor)
    }
}

impl Eq for BlockPublicationIdentity {}

impl Publication {
    pub(crate) fn new() -> Self {
        Self {
            owner: Arc::new(Owner),
            version: Mutex::new(Arc::new(Version)),
            released: ReleaseNotification::default(),
        }
    }

    fn lock_version(&self) -> ReleaseGuard<'_, std::sync::MutexGuard<'_, Arc<Version>>> {
        self.released
            .poisoning_guard(self.version.lock().expect("MV publication lock poisoned"))
    }

    // Call only after acquiring the original current and undo writers. The
    // published pair cannot change while those writers remain owned.
    pub(crate) fn capture(&self) -> CapturedPublication {
        CapturedPublication {
            owner: Arc::clone(&self.owner),
            version: Arc::clone(&self.lock_version()),
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
        let mut version = self.lock_version();
        publish();
        **version = next.0;
    }
}

impl CapturedPublication {
    /// Compare only the original owner without acquiring its publication lock.
    pub(crate) fn belongs_to(&self, publication: &Publication) -> bool {
        Arc::ptr_eq(&self.owner, &publication.owner)
    }

    /// Check the original owner and version without waiting on a publication cut.
    pub(crate) fn try_check_current<E>(
        &self,
        publication: &Publication,
    ) -> Result<(), PublicationPreparationError<E>> {
        if !Arc::ptr_eq(&self.owner, &publication.owner) {
            return Err(PublicationPreparationError::Changed);
        }
        let wait = publication.released.observe();
        match publication
            .version
            .try_lock()
            .map(|guard| publication.released.poisoning_guard(guard))
        {
            Ok(version) if Arc::ptr_eq(&self.version, &version) => Ok(()),
            Ok(_) => Err(PublicationPreparationError::Changed),
            Err(TryLockError::WouldBlock) => Err(PublicationPreparationError::Busy(wait)),
            Err(TryLockError::Poisoned(_)) => Err(PublicationPreparationError::Poisoned),
        }
    }

    pub(crate) fn matches(&self, publication: &Publication) -> bool {
        Arc::ptr_eq(&self.owner, &publication.owner)
            && Arc::ptr_eq(&self.version, &publication.lock_version())
    }

    pub(crate) fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.owner, &other.owner) && Arc::ptr_eq(&self.version, &other.version)
    }
}

#[cfg(test)]
#[path = "publication_nonblocking_tests.rs"]
mod nonblocking_tests;

#[cfg(test)]
mod tests {
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
