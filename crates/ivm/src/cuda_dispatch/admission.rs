//! Exact-artifact kernel qualification; physical health stays in iroha_accel.

use iroha_accel::{PtxArtifact, cuda::CudaFailure};
use std::sync::{
    Mutex, OnceLock, TryLockError,
    atomic::{AtomicU8, Ordering},
};

const UNTESTED: u8 = 0;
const ADMITTED: u8 = 1;
const QUARANTINED: u8 = 2;

/// One compile-artifact binding. No configuration generation can clear failure.
#[derive(Debug, Default)]
pub(super) struct KernelAdmission {
    artifact: OnceLock<PtxArtifact>,
    state: AtomicU8,
    gate: Mutex<()>,
}

impl KernelAdmission {
    pub(super) fn admit(
        &self,
        artifact: PtxArtifact,
        validate: impl FnOnce() -> Result<bool, CudaFailure>,
    ) -> bool {
        if self.artifact.get().is_some_and(|bound| *bound != artifact) {
            return false;
        }
        match self.state.load(Ordering::Acquire) {
            ADMITTED => return self.artifact.get() == Some(&artifact),
            QUARANTINED => return false,
            _ => {}
        }
        // Another validation is a local busy result, never a reason to wait on a
        // parent worker or quarantine a correctly operating device/kernel.
        let _gate = match self.gate.try_lock() {
            Ok(gate) => gate,
            Err(TryLockError::WouldBlock) => return false,
            Err(TryLockError::Poisoned(_)) => {
                self.quarantine();
                return false;
            }
        };
        if *self.artifact.get_or_init(|| artifact) != artifact {
            return false;
        }
        match self.state.load(Ordering::Acquire) {
            ADMITTED => true,
            QUARANTINED => false,
            UNTESTED => {
                // A fault reported during validation wins over a successful result.
                struct ValidationGuard<'a>(&'a AtomicU8, bool);
                impl Drop for ValidationGuard<'_> {
                    fn drop(&mut self) {
                        if !self.1 {
                            self.0.store(QUARANTINED, Ordering::Release);
                        }
                    }
                }
                let mut completed = ValidationGuard(&self.state, false);
                let outcome = validate();
                let next = match outcome {
                    Ok(true) => ADMITTED,
                    // No kernel result was obtained. A later caller may repeat the
                    // same artifact's public self-test when local capacity returns.
                    Err(CudaFailure::Capacity | CudaFailure::Busy | CudaFailure::Unavailable) => {
                        UNTESTED
                    }
                    Ok(false)
                    | Err(
                        CudaFailure::Quarantined
                        | CudaFailure::InvalidRequest
                        | CudaFailure::Driver(_)
                        | CudaFailure::Timeout,
                    ) => QUARANTINED,
                };
                // A concurrent or in-validation quarantine is never cleared by
                // either successful completion or a retryable local refusal.
                self.state
                    .compare_exchange(UNTESTED, next, Ordering::AcqRel, Ordering::Acquire)
                    .ok();
                completed.1 = true;
                self.state.load(Ordering::Acquire) == ADMITTED
            }
            _ => false,
        }
    }

    pub(super) fn admitted(&self, artifact: PtxArtifact) -> bool {
        self.state.load(Ordering::Acquire) == ADMITTED && self.artifact.get() == Some(&artifact)
    }

    pub(super) fn can_attempt(&self, artifact: PtxArtifact) -> bool {
        self.state.load(Ordering::Acquire) != QUARANTINED
            && self.artifact.get().is_none_or(|bound| *bound == artifact)
    }

    pub(super) fn quarantine(&self) {
        self.state.store(QUARANTINED, Ordering::Release);
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
