//! One nonblocking State reader engine with original physical release custody.

use super::*;
use iroha_allocation::release::{DeferredReleaseBatch, ReleaseWait};

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

/// The exhaustive World inventory selects each field's actual reader-release owner.
/// Cell reads are lock-free EBR pins; collector allocation remains a separate owner.
pub(crate) trait StateFieldReader {
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
    fn try_read_field(&self, _: &mut ()) -> Result<Self::View<'_>, StateViewError> {
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
    /// Hold the real header writer for an exact nonblocking-reader regression.
    pub(crate) fn with_held_header_for_reader_test<R>(
        &self,
        f: impl FnOnce(ReleaseWait) -> R,
    ) -> R {
        let _held = self.latest_block_header.write();
        f(self.latest_block_header.observe_release())
    }
}

impl World {
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
