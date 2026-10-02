//! Nonblocking original scalar copy without creating a collector participant.

use super::*;
use concread::ebrcell::EbrCellWriterAdmissionError;
use std::convert::Infallible;

/// One copied value bound to the original Cell publication, with no physical lock.
///
/// This local source token carries no publication permission and no State root.
/// It cannot substitute an equal-valued foreign Cell or an equal republication.
pub struct CommittedCellCopy<V: Value + Copy> {
    value: V,
    original: CapturedPublication,
}

impl<V: Value + Copy> CommittedCellCopy<V> {
    /// Borrow the original inline copy without cloning or acquiring a reader.
    pub fn current(&self) -> &V {
        &self.value
    }

    /// Compare the exact source publication without a collector pin or value read.
    ///
    /// # Errors
    /// Preserves contention or poisoning of the original publication identity lock.
    pub fn try_matches_current<C: Send + Sync + 'static>(
        &self,
        cell: &Cell<V, C>,
    ) -> Result<bool, PublicationPreparationError<Infallible>> {
        Ok(self
            .original
            .same_as(&cell.publication.try_capture_reads(|| true)?))
    }
}

impl<V: Value + Copy, C: Send + Sync + 'static> Cell<V, C> {
    /// Copy one original scalar while briefly retaining its actual current writer.
    ///
    /// The original publication observations bracket physical acquisition. No
    /// epoch collector is entered, no value is cloned and no successor is made.
    /// Current-writer release is notified through the same source used by native
    /// Cell acquisition. The copied value remains bound to its exact publication.
    ///
    /// # Errors
    /// Returns original Busy release custody, Poisoned or Changed without spinning.
    pub fn try_committed_copy(
        &self,
    ) -> Result<CommittedCellCopy<V>, PublicationPreparationError<Infallible>> {
        let before = self.publication.try_capture_reads(|| true)?;
        let wait = self.blocks_released.observe();
        let Some(acquired) = self.blocks.try_acquire_writer() else {
            return Err(if self.blocks.is_poisoned() {
                PublicationPreparationError::Poisoned
            } else {
                PublicationPreparationError::Busy(wait)
            });
        };
        let acquired = self.blocks_released.poisoning_guard(acquired);
        let value = acquired.copy_current().map_err(|error| match error {
            EbrCellWriterAdmissionError::Poisoned => PublicationPreparationError::Poisoned,
            EbrCellWriterAdmissionError::Refused(never) => match never {},
        })?;
        let after = self.publication.try_capture_reads(|| true)?;
        if !before.same_as(&after) {
            return Err(PublicationPreparationError::Changed);
        }
        drop(acquired);
        Ok(CommittedCellCopy {
            value,
            original: before,
        })
    }
}

#[cfg(test)]
#[path = "copy_read_tests.rs"]
mod tests;
