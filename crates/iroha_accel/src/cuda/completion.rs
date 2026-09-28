//! Exact-stream completion decisions and sticky uncertain-custody transitions.

use super::CudaFailure;
use crate::{custody::Phase, resources::DeviceHealth};
use cust::sys::CUresult;

pub(super) fn observe(
    query: Result<CUresult, CudaFailure>,
    phase: &mut Phase,
    health: &DeviceHealth,
    expired: bool,
) -> Result<bool, CudaFailure> {
    let failure = match query {
        Ok(CUresult::CUDA_SUCCESS) => {
            return if phase.complete(health.uncertain()) {
                Ok(true)
            } else {
                Err(CudaFailure::Quarantined)
            };
        }
        Ok(CUresult::CUDA_ERROR_NOT_READY) if !expired => return Ok(false),
        Ok(CUresult::CUDA_ERROR_NOT_READY) => CudaFailure::Timeout,
        Ok(error) => CudaFailure::Driver(error as i32),
        Err(error) => error,
    };
    // A query error is never proof of completion. Native backing must remain
    // owned just as it does after a timeout, including after policy reset.
    health.quarantine(true);
    Err(failure)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_then_exact_ready_preserve_health_and_allow_publication() {
        let health = DeviceHealth::default();
        let mut phase = Phase::Pending;
        for _ in 0..2 {
            assert_eq!(
                observe(
                    Ok(CUresult::CUDA_ERROR_NOT_READY),
                    &mut phase,
                    &health,
                    false
                ),
                Ok(false)
            );
            assert_eq!(phase, Phase::Pending);
        }
        assert_eq!(
            observe(Ok(CUresult::CUDA_SUCCESS), &mut phase, &health, false),
            Ok(true)
        );
        assert!(health.usable());
        assert!(!health.uncertain());
        assert!(phase.may_publish(health.usable()));
    }
    #[test]
    fn timeout_retains_pending_phase_and_cannot_be_repaired_by_a_late_ready() {
        let health = DeviceHealth::default();
        let mut phase = Phase::Pending;
        assert_eq!(
            observe(
                Ok(CUresult::CUDA_ERROR_NOT_READY),
                &mut phase,
                &health,
                true
            ),
            Err(CudaFailure::Timeout)
        );
        assert_eq!(phase, Phase::Pending);
        assert!(!health.usable());
        assert!(health.uncertain());
        assert_eq!(
            observe(Ok(CUresult::CUDA_SUCCESS), &mut phase, &health, false),
            Err(CudaFailure::Quarantined)
        );
        assert_eq!(phase, Phase::Pending);
        assert!(!phase.may_publish(health.usable()));
    }
    #[test]
    fn driver_and_context_query_errors_retain_uncertain_native_custody() {
        for query in [
            Ok(CUresult::CUDA_ERROR_INVALID_VALUE),
            Err(CudaFailure::Unavailable),
        ] {
            let health = DeviceHealth::default();
            let mut phase = Phase::Pending;
            assert!(observe(query, &mut phase, &health, false).is_err());
            assert!(health.uncertain());
            assert!(!health.usable());
            assert_eq!(phase, Phase::Pending);
            assert!(!phase.may_publish(health.usable()));
        }
    }
}
