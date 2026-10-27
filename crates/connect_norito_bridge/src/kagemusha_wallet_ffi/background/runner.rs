//! Bounded staged maintenance and owner-free event/timer parking.

use super::*;
use crate::kagemusha_wallet_ffi::maintenance::{self, View};

const AGE_RECHECK_MIN: Duration = Duration::from_secs(60);
const AGE_RECHECK_MAX: Duration = Duration::from_secs(15 * 60);
const PENDING_RECHECK_MAX: u64 = 60;

struct Pass {
    fold: Option<(i32, Option<u128>)>,
    again: bool,
    pending: bool,
    hint: state::DeferredArchiveWakeHintV1,
}
impl Pass {
    fn idle() -> Self {
        Self {
            fold: None,
            again: false,
            pending: false,
            hint: state::DeferredArchiveWakeHintV1::NoAgeWake,
        }
    }
    fn pending() -> Self {
        Self {
            pending: true,
            ..Self::idle()
        }
    }
}

fn delay(hint: state::DeferredArchiveWakeHintV1, pending_attempts: u32) -> Option<Duration> {
    if pending_attempts != 0 {
        return Some(
            Duration::from_secs(1_u64 << (pending_attempts - 1).min(6))
                .min(Duration::from_secs(PENDING_RECHECK_MAX)),
        );
    }
    match hint {
        state::DeferredArchiveWakeHintV1::NoAgeWake | state::DeferredArchiveWakeHintV1::Due => None,
        state::DeferredArchiveWakeHintV1::RecheckAfter(duration) => {
            Some(duration.clamp(AGE_RECHECK_MIN, AGE_RECHECK_MAX))
        }
    }
}

/// One fold stage only; the observation cannot attribute intake/Archive work to a fold.
fn fold_stage(owner: &Arc<Owner>, inner: &Inner) -> Result<Option<(i32, Option<u128>)>> {
    {
        let state = inner.lock();
        if state.stopped || !state.enabled || state.payments != 0 {
            return Ok(None);
        }
    }
    let observation = FoldBridgeTurnV1::of(&owner.background);
    let result = run(|| {
        let prepared = with_wallet_owner(Arc::clone(owner), Priority::None, |wallet| {
            let state = inner.lock();
            if state.stopped || !state.enabled || state.payments != 0 {
                return Ok(ControlFlow::Break(Response {
                    kind: 6,
                    ..Response::default()
                }));
            }
            drop(state);
            wallet.fold()
        })?;
        let response = match prepared {
            ControlFlow::Break(value) => value,
            ControlFlow::Continue(proof) => {
                // Proof releases its own Core slot before publication acquires the wallet.
                let publication = proof()?;
                with_wallet_owner(Arc::clone(owner), Priority::None, publication)?
            }
        };
        finished(&response).map(Some)
    });
    observation.finish(result.is_ok());
    result
}

/// Worker continuation never treats a lost/non-Archive output as an empty queue.
/// The external drain envelope retains the original completion; this loop surfaces a
/// sticky failure while Native retains the exact unresolved queue entry.
fn archive_outcome(value: &state::DeferredArchiveDrainV1) -> Result<(bool, bool)> {
    use advance::KagemushaWalletNotPerformedV1 as N;
    use state::{Completion as C, DeferredArchiveDrainV1 as D, DeferredArchiveWaitV1 as W};
    let value = match value {
        D::NoWork => return Ok((false, false)),
        D::Blocked(wait) => return Ok((false, *wait == W::SelectedPending)),
        D::Executed(value) | D::Reconciled(value) => value,
    };
    match value {
        C::Complete(_) => Ok((true, false)),
        C::Pending => Ok((false, true)),
        C::Archived | C::DeliveryDataLoss => Err(Failure::code(CUSTODY_LOST)),
        C::CreditStatus(_) => Err(Failure::code(INTERNAL)),
        C::NotPerformed(N::CapacityWait) => Err(Failure::code(NOT_ENOUGH_STORAGE)),
        C::NotPerformed(N::StaleHead | N::Invalid { .. }) => Err(Failure::code(INVALID)),
    }
}

fn bounded_pass(owner: &Arc<Owner>, inner: &Inner) -> Result<Pass> {
    let view = match maintenance::source_view(owner)? {
        View::Pending => maintenance::recover_selected(owner)?,
        other => other,
    };
    match view {
        View::BeforeBootstrap => return Ok(Pass::idle()),
        View::Pending => return Ok(Pass::pending()),
        View::Released | View::ArchiveFold => {}
    }
    let receipt = maintenance::receipt_step(owner)?;
    if matches!(receipt, maintenance::ReceiptStep::Pending) {
        return Ok(Pass::pending());
    }
    let fold = match fold_stage(owner, inner) {
        Ok(value) => value,
        // Only expected fold-stage cancellation is continuation. Cancellation from receipt,
        // recovery, custody or Archive remains an operational error with its exact fields.
        Err(error) if error.status == CANCELLED => {
            return Ok(Pass {
                again: true,
                ..Pass::idle()
            });
        }
        Err(error) => return Err(error),
    };
    let drained = maintenance::drain_step(owner, false)?;
    use state::{DeferredArchiveDrainV1 as D, DeferredArchiveWaitV1 as W};
    let (progressed, mut pending) = archive_outcome(&drained)?;
    let mut hint = if pending {
        state::DeferredArchiveWakeHintV1::NoAgeWake
    } else {
        match maintenance::wake_hint(owner)? {
            maintenance::WakeHint::Pending => {
                pending = true;
                state::DeferredArchiveWakeHintV1::NoAgeWake
            }
            maintenance::WakeHint::Age(value) => value,
        }
    };
    // The threshold can cross after a fresh NotDue refusal and before this later hint.
    // Recheck that one observation gap without spinning on a mature blocked queue.
    if matches!(drained, D::Blocked(W::NotDue))
        && matches!(hint, state::DeferredArchiveWakeHintV1::Due)
    {
        hint = state::DeferredArchiveWakeHintV1::RecheckAfter(AGE_RECHECK_MIN);
    }
    Ok(Pass {
        fold,
        again: receipt.progressed()
            || progressed
            || fold.is_some_and(|(kind, _)| matches!(kind, 8 | 9)),
        pending,
        hint,
    })
}

pub(super) fn worker(owner: Weak<Owner>, inner: Arc<Inner>) {
    loop {
        {
            let mut control = inner.lock();
            loop {
                if control.stopped {
                    return;
                }
                if control.error.is_none() && control.payments == 0 {
                    let now = Instant::now();
                    if control.wake || control.deadline.is_some_and(|deadline| deadline <= now) {
                        // Consume only events that preceded this pass. Completion cannot erase
                        // a receipt/release/activity event that arrives during the work.
                        control.wake = false;
                        control.deadline = None;
                        control.running = true;
                        break;
                    }
                    if let Some(deadline) = control.deadline {
                        let (next, _) = inner
                            .ready
                            .wait_timeout(control, deadline.saturating_duration_since(now))
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        control = next;
                        continue;
                    }
                }
                control = inner
                    .ready
                    .wait(control)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        }
        let Some(owner) = owner.upgrade() else {
            return;
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            bounded_pass(&owner, &inner)
        }))
        .unwrap_or(Err(Failure::code(INTERNAL)));
        drop(owner);
        let mut control = inner.lock();
        control.running = false;
        complete(&mut control, result, Instant::now());
        inner.ready.notify_all();
    }
}

fn complete(control: &mut Control, result: Result<Pass>, now: Instant) {
    match result {
        Ok(pass) => {
            if let Some((_, Some(backlog))) = pass.fold {
                control.backlog = Some(backlog);
            }
            control.pending_attempts = if pass.pending {
                control.pending_attempts.saturating_add(1)
            } else {
                0
            };
            if pass.again && !pass.pending {
                control.wake = true;
            }
            control.deadline = delay(pass.hint, control.pending_attempts)
                .and_then(|duration| now.checked_add(duration));
        }
        Err(error) => {
            control.backlog = None;
            control.deadline = None;
            control.error = Some((error, false));
        }
    }
}

#[cfg(test)]
mod tests;
