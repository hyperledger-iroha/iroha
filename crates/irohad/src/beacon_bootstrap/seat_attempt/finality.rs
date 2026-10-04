//! One original proof stream and atomic native clock, retaining local refusals.

use super::*;

/// Canonical journal and exact raw frame survive until the original cursor advances.
/// TODO: prepare/fund the journal's decoded block/index graph independently.
pub(super) struct FinalityInput {
    input: FrameInput,
    journal: Option<NativeFinalityJournal>,
    clock: NativeJournalCursor,
    height: u64,
    committed: bool,
}
impl FinalityInput {
    pub(super) fn new(input: FrameInput, clock: NativeJournalCursor, height: u64) -> Self {
        Self {
            input,
            journal: None,
            clock,
            height,
            committed: false,
        }
    }
    pub(super) fn height(&self) -> u64 {
        self.height
    }
    fn finish_frame(&mut self) -> std::result::Result<(), AttemptError> {
        self.input.consume_verified_frame()?;
        self.journal = None;
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
            if self.journal.is_none() {
                // Keep the sole existing canonical journal decoder. Raw frame
                // admission does not establish its decoded/index graph custody.
                self.journal = Some(NativeFinalityJournal::decode(
                    self.input.frame().ok_or(AttemptError::Phase)?,
                    self.clock.limits(),
                )?);
            }
            let journal = self.journal.as_ref().ok_or(AttemptError::Phase)?;
            let height = u64::try_from(journal.blocks.len()).map_err(|_| AttemptError::Height)?;
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
