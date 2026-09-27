/// Original deferred Apply and its continuously registered physical dependency.
struct RetainedLocalApply {
    task: ApplyTask,
    dependency: RetainedApplyDependency,
}

/// One release observation belongs to the original publication until dispatch.
/// Keeping the future alive preserves its wake registration between runner turns.
enum RetainedApplyDependency {
    Release {
        pending: concread::release::ReleaseFuture,
        wake: std::task::Waker,
        resource: &'static str,
    },
    RecoveryRequired(String),
}

impl RetainedApplyDependency {
    fn new(refusal: &super::v2_body_store::LocalValidationRefusal) -> Self {
        use super::v2_body_store::LocalValidationRefusal;
        match refusal {
            LocalValidationRefusal::PhysicalBusy(busy) => {
                let wake = busy.waker().clone();
                Self::Release {
                    pending: busy.wait.clone().wait_for_release(),
                    wake,
                    resource: busy.resource,
                }
            }
            LocalValidationRefusal::QueueRelease { wait, wake } => Self::Release {
                pending: wait.clone().wait_for_release(),
                wake: wake.clone(),
                resource: "queue-release",
            },
            LocalValidationRefusal::RecoveryRequired(reason) => {
                Self::RecoveryRequired(reason.clone())
            }
            LocalValidationRefusal::ObservationChanged { .. } => Self::RecoveryRequired(
                "validated Apply unexpectedly returned a pre-execution State refresh".into(),
            ),
            LocalValidationRefusal::Superseded => Self::RecoveryRequired(
                "validated Apply unexpectedly lost its finalized State owner".into(),
            ),
            LocalValidationRefusal::NativeSourceRecovery { .. } => Self::RecoveryRequired(
                "validated Apply lost its original Native source custody".into(),
            ),
        }
    }

    fn ready(&mut self) -> Result<bool, String> {
        match self {
            Self::Release { pending, wake, .. } => Ok(std::future::Future::poll(
                std::pin::Pin::new(pending),
                &mut std::task::Context::from_waker(wake),
            )
            .is_ready()),
            Self::RecoveryRequired(reason) => Err(reason.clone()),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Release { resource, .. } => resource,
            Self::RecoveryRequired(_) => "recovery-required",
        }
    }
}
