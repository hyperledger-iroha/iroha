//! Atomic first-capture publication and sticky failure across errors and unwinding.

use super::{Hash, StateBlock, output_capacity::ExecutionOutputPlanState};

/// Retain the prior lane seal until every witness-derived output can be published.
/// A failed attempt discards cached outputs and cannot be repaired by a new capture.
/// The checked recorder drain separately owns recorder reset on errors and unwinding.
pub(super) struct WitnessCaptureGuard<'capture, 'state> {
    pub(super) state: &'capture mut StateBlock<'state>,
    prior_lane_seal: Option<Hash>,
    finished: bool,
}

impl<'capture, 'state> WitnessCaptureGuard<'capture, 'state> {
    pub(super) fn new(state: &'capture mut StateBlock<'state>) -> Self {
        Self {
            prior_lane_seal: state.lane_consensus_contexts_seal,
            state,
            finished: false,
        }
    }

    /// Finish only after all fallible work and all three output assignments.
    pub(super) fn finish(mut self) {
        self.finished = true;
    }

    /// Preserve the exact first error and discard the attempted capture's outputs.
    pub(super) fn reject(mut self, error: String) -> String {
        let error = self.invalidate(error);
        self.finished = true;
        error
    }

    fn invalidate(&mut self, error: String) -> String {
        self.state.lane_consensus_contexts_seal = self.prior_lane_seal;
        if self.state.execution_output_plan.is_some() {
            self.state.execution_output_plan = Some(ExecutionOutputPlanState::Poisoned);
        }
        self.state.reject_fastpq_witness_content(error)
    }
}

impl Drop for WitnessCaptureGuard<'_, '_> {
    fn drop(&mut self) {
        if !self.finished {
            self.invalidate("execution witness capture was interrupted".into());
        }
    }
}
