//! Erase initialized diagnostic register fields before owner replacement or release.

use super::{
    DiagnosticStepRecord, DiagnosticStepRecorder, DiagnosticStepState, PendingDiagnosticStep,
};
use iroha_crypto::zeroize_value_for_confidential_discard;

impl DiagnosticStepState {
    pub(super) fn scrub(&mut self) {
        zeroize_value_for_confidential_discard(&mut self.registers);
        zeroize_value_for_confidential_discard(&mut self.tags);
    }
}

impl DiagnosticStepRecord {
    pub(super) fn scrub(&mut self) {
        self.before.scrub();
        self.after.scrub();
    }
}

impl Drop for PendingDiagnosticStep {
    fn drop(&mut self) {
        self.before.scrub();
    }
}

impl Drop for DiagnosticStepRecorder {
    fn drop(&mut self) {
        // These private buffers have fixed backing and never truncate or grow.
        // Only initialized register/tag fields are touched, never padding,
        // enum discriminants, or the original allocation/reservation owners.
        for record in self.records.as_mut_slice() {
            record.scrub();
        }
        if let Some(end) = &mut self.end {
            end.state.scrub();
        }
    }
}

#[cfg(test)]
mod tests;
