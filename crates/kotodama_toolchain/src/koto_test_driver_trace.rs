//! Immutable original-pool trace custody for nested test reports and checkpoints.

use super::{IVM, KotoTestHost, TraceMode};
use ivm::{VMError, zk::RuntimeTraceCapture};

/// Retain the root followed by completed nested observations in report order.
pub(super) fn capture_report(
    vm: &IVM,
    supplemental: Option<&RuntimeTraceCapture>,
) -> Result<Option<RuntimeTraceCapture>, VMError> {
    if vm.trace_mode() == TraceMode::Off {
        return Ok(supplemental.cloned());
    }
    let root = vm.try_runtime_trace_capture()?;
    match supplemental {
        Some(nested) => root.try_combine(nested).map(Some),
        None => Ok(Some(root)),
    }
}

impl KotoTestHost {
    /// Fund the complete replacement before changing any retained report owner.
    pub(super) fn record_nested_trace(&mut self, nested_vm: &IVM) -> Result<(), VMError> {
        if nested_vm.trace_mode() == TraceMode::Off {
            return Ok(());
        }
        let following = nested_vm.try_runtime_trace_capture()?;
        let replacement = match self.supplemental_trace.as_ref() {
            Some(prior) => prior.try_combine(&following)?,
            None => following,
        };
        self.supplemental_trace = Some(replacement);
        Ok(())
    }
}

#[cfg(test)]
#[path = "koto_test_driver_trace_tests.rs"]
mod tests;
