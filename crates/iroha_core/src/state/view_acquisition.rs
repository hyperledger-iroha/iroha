//! One nonblocking State reader engine with original physical release custody.

use super::*;
use iroha_allocation::release::{DeferredReleaseBatch, ReleaseRegistration, ReleaseWait};

/// An unfinished physical State read is distinct from an invalid runtime projection.
#[derive(Debug, thiserror::Error)]
pub enum StateViewError {
    /// The exact original reader or publishing writer must release before retry.
    #[error("original State reader is busy")]
    Busy(ReleaseWait),
    /// Original reader control custody did not belong to this storage owner.
    #[error("original State reader custody changed")]
    Changed,
    /// A physical reader owner was poisoned by an earlier failed publication.
    #[error("original State reader is poisoned")]
    Poisoned,
    /// The captured stable runtime and World do not form a valid projection.
    #[error(transparent)]
    Runtime(#[from] LaneLifecycleError),
}

impl From<ReleaseWait> for StateViewError {
    fn from(wait: ReleaseWait) -> Self {
        Self::Busy(wait)
    }
}
impl From<mv::PublicationPreparationError<std::convert::Infallible>> for StateViewError {
    fn from(error: mv::PublicationPreparationError<std::convert::Infallible>) -> Self {
        match error {
            mv::PublicationPreparationError::Busy(wait) => Self::Busy(wait),
            mv::PublicationPreparationError::Poisoned => Self::Poisoned,
            mv::PublicationPreparationError::Changed => Self::Changed,
            mv::PublicationPreparationError::Admission(error) => match error {},
        }
    }
}

/// One admitted worker's prepaid wait for a refused State acquisition to release.
///
/// The reusable registration node is charged to the caller's own request pool
/// before any acquisition and is bound to the thread that builds it; the waiter
/// never moves to another thread. A wait ends at the exact busy owner's release,
/// the caller's absolute deadline or its stop flag, whichever comes first, and
/// extends neither bound. Whoever sets the stop flag must also unpark the thread.
/// Nothing from the refused attempt is held while parked, and a release grants
/// nothing: every retry acquires its original State owners again.
pub struct StateViewWaiter {
    registration: ReleaseRegistration,
    _thread: std::marker::PhantomData<*mut ()>,
}

impl StateViewWaiter {
    /// Exact waiter node layout the caller prepays from its own request pool.
    #[must_use]
    pub fn allocation_layout() -> std::alloc::Layout {
        ReleaseRegistration::allocation_layout()
    }

    /// Bind one prepaid waiter to the current physical worker thread.
    ///
    /// # Errors
    /// Returns the exact reservation shortage or allocator refusal; nothing waits.
    pub fn for_current_thread(
        reservation: &mut iroha_allocation::AllocationReservation,
    ) -> Result<Self, iroha_allocation::PrepaidSharedError> {
        let mut registration = ReleaseRegistration::from_reservation(reservation)?;
        registration.bind_thread(std::thread::current());
        Ok(Self {
            registration,
            _thread: std::marker::PhantomData,
        })
    }

    /// Park until `release` completes; `false` once `deadline` passes or `stop` is set.
    ///
    /// Spurious and foreign wakes recheck the same release and never reset time.
    pub fn wait(&mut self, release: &ReleaseWait, deadline: Instant, stop: &AtomicBool) -> bool {
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let released = loop {
            if stop.load(Ordering::Acquire) {
                break false;
            }
            let Some(remaining) = deadline
                .checked_duration_since(Instant::now())
                .filter(|remaining| !remaining.is_zero())
            else {
                break false;
            };
            if self
                .registration
                .poll_wait(release, &mut context)
                .is_ready()
            {
                break true;
            }
            std::thread::park_timeout(remaining);
        };
        self.registration.cancel();
        released
    }
}

/// The exhaustive World inventory selects each field's actual reader-release owner.
/// Cell reads are lock-free EBR pins; collector allocation remains a separate owner.
pub trait StateFieldReader {
    type Releases;
    type View<'a>
    where
        Self: 'a;
    fn reader_releases(&self) -> Self::Releases;
    fn try_read_field(
        &self,
        releases: &mut Self::Releases,
    ) -> Result<Self::View<'_>, StateViewError>;
}
impl<K: mv::Key, V: mv::Value, M: mv::storage::StorageMode<K, V>> StateFieldReader
    for Storage<K, V, M>
{
    type Releases = DeferredReleaseBatch;
    type View<'a>
        = StorageView<'a, K, V, M>
    where
        Self: 'a;
    fn reader_releases(&self) -> Self::Releases {
        self.reader_releases()
    }
    fn try_read_field(
        &self,
        releases: &mut Self::Releases,
    ) -> Result<Self::View<'_>, StateViewError> {
        self.try_view_retaining(releases).map_err(Into::into)
    }
}
impl<V: mv::Value, C: Send + Sync + 'static> StateFieldReader for Cell<V, C> {
    type Releases = ();
    type View<'a>
        = mv::cell::View<'a, V>
    where
        Self: 'a;
    fn reader_releases(&self) {}
    fn try_read_field(&self, (): &mut ()) -> Result<Self::View<'_>, StateViewError> {
        Ok(self.view())
    }
}
impl StateFieldReader for TriggerSet {
    type Releases = crate::smartcontracts::isi::triggers::set::SetReadReleases;
    type View<'a> = TriggerSetView<'a>;
    fn reader_releases(&self) -> Self::Releases {
        Self::Releases::new(self)
    }
    fn try_read_field(
        &self,
        releases: &mut Self::Releases,
    ) -> Result<Self::View<'_>, StateViewError> {
        self.try_view_retaining(releases)
    }
}

pub(super) use super::authority_registry::world::WorldReadReleases;

#[cfg(test)]
impl State {
    /// Probe original State physical publication and reader owners from a test waker.
    pub(crate) fn assert_view_physical_fences_released_for_reader_test(&self) {
        let _commit = self
            .state_commit_lock
            .try_lock()
            .expect("original refund callback runs after the State commit fence");
        let _write = self
            .state_write_lock
            .try_lock()
            .expect("original refund callback runs after the State writer");
        let _view = self
            .try_view_once()
            .expect("original refund callback can acquire all actual State reader owners");
    }
}

#[cfg(any(test, feature = "iroha-core-tests"))]
impl State {
    /// Hold the actual State publisher and visibility interval for nonblocking reader tests.
    /// The callback receives the existing physical writer's original release observation.
    pub fn with_held_view_publication_for_reader_test<R>(
        &self,
        f: impl FnOnce(ReleaseWait) -> R,
    ) -> R {
        let mut notice = self.state_view_publication();
        let mut release = self.state_write_lock.defer_notifications();
        let _held = release.lock();
        let _publication = notice.begin();
        f(self.state_write_lock.observe_release())
    }

    /// Hold the original cached-header writer for a native nonblocking-reader fixture.
    ///
    /// The callback receives this writer's actual release observation. The guard changes
    /// neither the cached header nor the certified State publication generation.
    pub fn with_held_header_for_reader_test<R>(&self, f: impl FnOnce(ReleaseWait) -> R) -> R {
        let _held = self.latest_block_header.write();
        f(self.latest_block_header.observe_release())
    }
}

impl World {
    /// Retain raw auth sources without manufacturing a universal catalog allocation.
    /// The enclosing stable auth view supplies its independently validated borrowed catalog.
    pub(super) fn try_authorization_view_retaining(
        &self,
        releases: &mut WorldReadReleases,
    ) -> Result<WorldView<'_>, StateViewError> {
        let catalog = DataSpaceCatalog::new(Vec::new()).map_err(runtime_catalog_invalid)?;
        with_world_overlay_fields!(build_world_view_from_fields, self, releases, catalog)
    }

    /// Synchronous callers retain the same reader notices through their outer fences.
    pub(super) fn view_retaining(&self, releases: &mut WorldReadReleases) -> WorldView<'_> {
        loop {
            match self.try_view_retaining(releases) {
                Ok(view) => return view,
                Err(StateViewError::Busy(_)) => std::thread::yield_now(),
                Err(error) => panic!("original World reader refused: {error}"),
            }
        }
    }

    /// Pin the complete existing World inventory without waiting or dispatching callbacks.
    pub(super) fn try_view_retaining(
        &self,
        releases: &mut WorldReadReleases,
    ) -> Result<WorldView<'_>, StateViewError> {
        build_world_view!(self, releases)
    }
}

#[cfg(test)]
#[path = "view_acquisition_tests.rs"]
mod tests;
