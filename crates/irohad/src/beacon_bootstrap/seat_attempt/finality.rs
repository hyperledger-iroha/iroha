//! One original proof stream and atomic native clock, retaining local refusals.

use super::*;
use iroha_data_model::sumeragi::finality::PreparedNativeFinalityJournal;

/// Prepared source ranges and exact raw frame survive until the original cursor advances.
/// TODO: fund decoded nested SignedBlock/result graphs independently.
pub(super) struct FinalityInput {
    input: FrameInput,
    journal: PreparedNativeFinalityJournal,
    clock: NativeJournalCursor,
    height: u64,
    committed: bool,
}
impl FinalityInput {
    pub(super) fn new(
        input: FrameInput,
        clock: NativeJournalCursor,
        height: u64,
    ) -> std::result::Result<Self, AttemptError> {
        let journal =
            PreparedNativeFinalityJournal::new(clock.limits(), clock.allocation_budget())?;
        Ok(Self {
            input,
            journal,
            clock,
            height,
            committed: false,
        })
    }
    pub(super) fn height(&self) -> u64 {
        self.height
    }
    fn finish_frame(&mut self) -> std::result::Result<(), AttemptError> {
        self.input.consume_verified_frame()?;
        self.journal.clear_consumed();
        self.committed = false;
        Ok(())
    }
    pub(super) fn advance_to(
        &mut self,
        target: u64,
        cutoff: u64,
    ) -> std::result::Result<(), AttemptError> {
        if self.committed {
            self.finish_frame()?;
        }
        while self.height < target {
            self.input.read_until_complete()?;
            let frame = self.input.frame().ok_or(AttemptError::Phase)?;
            self.journal.decode(frame)?;
            let journal = self.journal.view(frame)?;
            let height = u64::try_from(journal.len()).map_err(|_| AttemptError::Height)?;
            check_rotation_phase_height(self.height, height, cutoff)
                .map_err(|_| AttemptError::Height)?;
            if height > target
                || self.clock.tip().map(|tip| tip.height()).unwrap_or(1) != self.height
            {
                return Err(AttemptError::Height);
            }
            let verified = self.clock.advance(journal)?;
            self.height = verified.height();
            self.committed = true;
            self.finish_frame()?;
        }
        if self.height != target {
            return Err(AttemptError::Height);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
