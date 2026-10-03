//! Preflight and publication of the admitted per-cycle commitment rows.
//!
//! Only fixed cycle-root storage is covered here. Delta trace and event/path
//! allocations retain their separate resource boundaries.

use super::{IVM, VMError};

impl IVM {
    /// Admit every completed instruction or padding cycle before its effects.
    pub(super) fn prepare_cycle_logs(&mut self, additional: u64) -> Result<(), VMError> {
        if self.zk_trace_collection_enabled() && self.max_cycles != 0 {
            self.step_log.prepare_cycles(additional)?;
        }
        Ok(())
    }

    /// Publish only rows already admitted by the instruction/padding owner.
    pub(super) fn flush_cycle_logs(&mut self, last_logged_cycle: &mut u64) {
        // Semantic ZK execution need not collect diagnostic/proof trace roots.
        if !self.zk_trace_collection_enabled() || self.max_cycles == 0 {
            return;
        }
        while *last_logged_cycle < self.cycles {
            self.trace_log.record(
                self.pc,
                self.registers.snapshot(),
                self.registers.snapshot_tags(),
            );
            self.step_log.record_reserved(
                self.pc,
                self.registers.merkle_root(),
                self.memory.current_root(),
            );
            *last_logged_cycle += 1;
        }
    }
}

#[cfg(test)]
mod tests;
