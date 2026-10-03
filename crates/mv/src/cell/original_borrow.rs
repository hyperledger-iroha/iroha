//! Original non-Copy scalar pair borrowed under its physical writers, without EBR entry.

use super::*;
use concread::ebrcell::EbrCellWriterAcquisition;
use std::convert::Infallible;

/// Exact current and undo generations borrowed under their original physical writers.
///
/// This short-lived local observation neither clones payloads nor enters the epoch
/// collector. Both writers remain excluded until this owner drops. Callers must
/// bound work before acquiring it and release it before awaiting external work.
/// References cannot outlive this guard. It issues no publication permission.
pub struct CommittedCellBorrow<'a, V: Value, C: Send + Sync + 'static = Untracked> {
    source: &'a Cell<V, C>,
    original: CapturedPublication,
    current: Option<ReleaseGuard<'a, EbrCellWriterAcquisition<'a, V, C>>>,
    undo: Option<ReleaseGuard<'a, EbrCellWriterAcquisition<'a, Option<V>, C>>>,
}

/// Retained original publication after all physical observation guards release.
/// No payload or publication authority is owned by this identity-only token.
pub struct CommittedCellObservation<'a, V: Value, C: Send + Sync + 'static = Untracked> {
    source: &'a Cell<V, C>,
    original: CapturedPublication,
}
impl<V: Value, C: Send + Sync + 'static> CommittedCellObservation<'_, V, C> {
    /// Recheck the exact original Cell without a value read, clone or collector pin.
    ///
    /// # Errors
    /// Preserves the original publication mutex refusal.
    pub fn try_matches_current(&self) -> Result<bool, PublicationPreparationError<Infallible>> {
        Ok(self
            .original
            .same_as(&self.source.publication.try_capture_reads(|| true)?))
    }
}

impl<'a, V: Value, C: Send + Sync + 'static> CommittedCellBorrow<'a, V, C> {
    /// Retain the original identity while releasing both actual observation writers.
    /// The returned token permits only a future source check, never publication.
    ///
    /// # Errors
    /// Refuses original publication contention, poison or changed identity.
    pub fn release_observation(
        self,
    ) -> Result<CommittedCellObservation<'a, V, C>, PublicationPreparationError<Infallible>> {
        let original = self.source.publication.try_capture_reads(|| true)?;
        if !self.original.same_as(&original) {
            return Err(PublicationPreparationError::Changed);
        }
        let observation = CommittedCellObservation {
            source: self.source,
            original,
        };
        drop(self);
        Ok(observation)
    }

    /// Borrow the exact original current value without cloning or allocating.
    pub fn current(&self) -> &V {
        self.current
            .as_ref()
            .expect("complete original borrow")
            .borrow_current()
            .expect("original poison checked while acquiring sole writer")
    }
    /// Borrow the complete retained undo; None differs from Some(None) for optional values.
    pub fn undo(&self) -> &Option<V> {
        self.undo
            .as_ref()
            .expect("complete original borrow")
            .borrow_current()
            .expect("original poison checked while acquiring sole writer")
    }
    /// Verify the same source publication while retaining both physical originals.
    ///
    /// # Errors
    /// Preserves exact publication Busy, poison or changed identity without waiting.
    pub fn try_matches_current(&self) -> Result<bool, PublicationPreparationError<Infallible>> {
        Ok(self
            .original
            .same_as(&self.source.publication.try_capture_reads(|| true)?))
    }
}

impl<V: Value, C: Send + Sync + 'static> Drop for CommittedCellBorrow<'_, V, C> {
    fn drop(&mut self) {
        // Release every physical guard before waking any original waiter. No
        // arbitrary cleanup, callback or collector runs under another data lock.
        let current = self
            .current
            .take()
            .map(|held| held.release_deferred(drop).1);
        let undo = self.undo.take().map(|held| held.release_deferred(drop).1);
        drop((current, undo));
    }
}

impl<V: Value, C: Send + Sync + 'static> Cell<V, C> {
    /// Borrow the original non-Copy committed pair without clone, pin or successor.
    ///
    /// Native Cell order is undo then current. Acquiring either source is strictly
    /// nonblocking. A partial refusal releases original guards before notifications.
    /// No data writer is acquired while the publication identity lock is retained.
    ///
    /// # Errors
    /// Returns the original physical/publication Busy release, poison, or Changed.
    pub fn try_committed_borrow(
        &self,
    ) -> Result<CommittedCellBorrow<'_, V, C>, PublicationPreparationError<Infallible>> {
        let original = self.publication.try_capture_reads(|| true)?;
        let mut held = CommittedCellBorrow {
            source: self,
            original,
            current: None,
            undo: None,
        };
        let wait = self.revert_released.observe();
        let Some(undo) = self.revert.try_acquire_writer() else {
            return Err(if self.revert.is_poisoned() {
                PublicationPreparationError::Poisoned
            } else {
                PublicationPreparationError::Busy(wait)
            });
        };
        held.undo = Some(self.revert_released.poisoning_guard(undo));
        if held.undo.as_ref().unwrap().is_poisoned() {
            return Err(PublicationPreparationError::Poisoned);
        }
        let wait = self.blocks_released.observe();
        let Some(current) = self.blocks.try_acquire_writer() else {
            return Err(if self.blocks.is_poisoned() {
                PublicationPreparationError::Poisoned
            } else {
                PublicationPreparationError::Busy(wait)
            });
        };
        held.current = Some(self.blocks_released.poisoning_guard(current));
        if held.current.as_ref().unwrap().is_poisoned() {
            return Err(PublicationPreparationError::Poisoned);
        }
        if !held.try_matches_current()? {
            return Err(PublicationPreparationError::Changed);
        }
        Ok(held)
    }
}

#[cfg(test)]
#[path = "original_borrow_tests.rs"]
mod tests;
