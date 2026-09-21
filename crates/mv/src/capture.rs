//! Original capture notifications retained by the enclosing field aggregate.

use concread::release::DeferredRelease;

/// Native release custody from a successful original-owner capture.
///
/// Both physical writers have already released. Keep this owner until every
/// enclosing writer and fence has released, then drop it to notify the original
/// waiters. It does not allocate, mint a wake source, or authorize publication.
#[must_use = "retain capture cleanup through every enclosing physical owner"]
#[derive(Default)]
pub struct CaptureCleanup {
    current: Option<DeferredRelease>,
    undo: Option<DeferredRelease>,
}

impl CaptureCleanup {
    pub(crate) fn new(current: DeferredRelease, undo: DeferredRelease) -> Self {
        Self {
            current: Some(current),
            undo: Some(undo),
        }
    }
}

impl Drop for CaptureCleanup {
    fn drop(&mut self) {
        // Taking each field keeps the remaining actual notification armed if
        // the first callback unwinds. No physical guard remains in this owner.
        drop(self.current.take());
        drop(self.undo.take());
    }
}

/// Caller-owned original block capture, including admission refusal and unwind.
///
/// An aggregate must own every capture slot before invoking any slot's capture.
/// Its Drop releases all slots before destroying any slot. A refused or panicked
/// attempt is terminal; only successful capture can transfer a detached journal.
/// Callback-local allocations and destructors remain the callback's obligation.
pub trait BlockCapture<Admission>: Sized {
    /// The exact attached block borrowed by admission.
    type Block;
    /// The exact original journal, without physical writer guards.
    type Detached;
    /// Check and capture while retaining the original block in this caller slot.
    fn try_capture<E>(
        &mut self,
        admit: impl FnOnce(&Self::Block) -> Result<Admission, E>,
    ) -> Result<(), E>;
    /// Terminally unlock any still-attached block, retaining its cleanup in place.
    /// Captured journals remain untouched; this grants no publication authority.
    fn release(&mut self);
    /// Transfer an already captured original journal and its deferred releases.
    fn into_detached(self) -> (Self::Detached, CaptureCleanup);
}
