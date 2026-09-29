//! Owner cleanup is atomic before lifecycle publication and independent of write-log credit.

use super::{IVM, VMError};

impl IVM {
    /// Validate and erase every private byte before retiring its tag.
    pub(super) fn scrub_private_memory(&mut self) -> Result<(), VMError> {
        self.memory
            .scrub_private_ranges(&self.private_memory_bytes)?;
        self.private_memory_bytes.clear();
        Ok(())
    }

    /// Keep all state intact if memory or diagnostic preflight refuses cleanup.
    pub(super) fn scrub_private_state(&mut self) -> Result<(), VMError> {
        let had_private_context =
            self.zk_mode || self.registers.has_private() || !self.private_memory_bytes.is_empty();
        self.scrub_private_memory()?;
        self.registers.scrub_private();
        if had_private_context {
            self.clear_zk_trace_logs();
            self.memory.clear_tracking();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
