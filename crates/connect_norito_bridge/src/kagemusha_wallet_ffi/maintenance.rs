//! Source-selected maintenance outside wallet ownership and both proof-priority guards.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum View {
    BeforeBootstrap,
    Released,
    Pending,
    ArchiveFold,
}

pub(super) enum DrainPlan {
    Ready,
    Finished(state::DeferredArchiveDrainV1),
    Pending,
}

/// Completed receipt classification, kept typed until publication and the final fence finish.
pub(super) enum ReceiptStep {
    Empty,
    Pending,
    Verified(Vec<u8>),
    Rejected,
}
impl ReceiptStep {
    pub(super) fn progressed(&self) -> bool {
        matches!(self, Self::Verified(_) | Self::Rejected)
    }
}
fn receipt_result(
    value: std::result::Result<Option<state::CreditProjectionV1>, state::Error>,
) -> Result<ReceiptStep> {
    match value {
        Ok(None) => Ok(ReceiptStep::Empty),
        Ok(Some(value)) => Ok(ReceiptStep::Verified(value.bytes())),
        // Only this typed, receipt-specific completed verdict can retire verification work.
        // Return success internally so the enclosing archive call publishes the rejection.
        Err(state::Error::ReceiptRejected(_)) => Ok(ReceiptStep::Rejected),
        Err(state::Error::Pending) => Ok(ReceiptStep::Pending),
        Err(error) => Err(error.into()),
    }
}
pub(super) fn receipt_response(value: ReceiptStep) -> Result<Response> {
    match value {
        ReceiptStep::Empty => Ok(Response {
            kind: 62,
            ..Response::default()
        }),
        ReceiptStep::Verified(value) => Ok(Response {
            kind: 47,
            bytes: value,
            ..Response::default()
        }),
        ReceiptStep::Rejected => Err(Failure::code(PROOF_REJECTED)),
        ReceiptStep::Pending => Err(Failure::code(UNCERTAIN)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WakeHint {
    Pending,
    Age(state::DeferredArchiveWakeHintV1),
}

pub(super) fn wait_tag(wait: state::DeferredArchiveWaitV1) -> u32 {
    use state::DeferredArchiveWaitV1 as W;
    match wait {
        W::SelectedPending => 1,
        W::ArchiveFold => 2,
        W::FoldBacklog => 3,
        W::ExchangeOpen => 4,
        W::NotDue => 5,
        W::Busy => 6,
    }
}

pub(super) fn blocked(wait: state::DeferredArchiveWaitV1) -> Response {
    Response {
        kind: 63,
        detail: wait_tag(wait),
        ..Response::default()
    }
}

pub(super) fn drain_response(value: state::DeferredArchiveDrainV1) -> Result<Response> {
    use state::DeferredArchiveDrainV1 as D;
    let (disposition, value) = match value {
        D::NoWork => {
            return Ok(Response {
                kind: 62,
                ..Response::default()
            });
        }
        D::Blocked(wait) => return Ok(blocked(wait)),
        D::Reconciled(value) => (1, value),
        D::Executed(value) => (2, value),
    };
    let inner = completion(Some(value))?;
    if inner.bytes.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 {
        return Err(Failure::code(INTERNAL));
    }
    let mut bytes = Vec::with_capacity(28 + inner.bytes.len());
    bytes.extend_from_slice(&(inner.kind as u32).to_le_bytes());
    bytes.extend_from_slice(&inner.detail.to_le_bytes());
    bytes.extend_from_slice(&inner.sequence.to_le_bytes());
    bytes.extend_from_slice(&(inner.bytes.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&inner.bytes);
    Ok(Response {
        kind: 64,
        detail: disposition,
        bytes,
        ..Response::default()
    })
}

impl<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send> NativeWallet<P, S> {
    pub(super) fn maintenance_inner(&mut self) -> Result<View> {
        let view = match self.wallet.maintenance_view() {
            Ok(view) => view,
            Err(state::Error::Pending) => return Ok(View::Pending),
            Err(error) => return Err(error.into()),
        };
        Ok(match view {
            state::MaintenanceViewV1::BeforeBootstrap => View::BeforeBootstrap,
            state::MaintenanceViewV1::SelectedPending => View::Pending,
            state::MaintenanceViewV1::AwaitingArchiveFold(_) => View::ArchiveFold,
            state::MaintenanceViewV1::Released(_) => View::Released,
        })
    }

    pub(super) fn receipt_inner(&mut self) -> Result<ReceiptStep> {
        receipt_result(self.wallet.verify_next_deferred_credited())
    }
    pub(super) fn observe_receipt_inner(
        &mut self,
        send: &[u8; 32],
        original: &[u8],
    ) -> Result<ReceiptStep> {
        receipt_result(
            self.wallet
                .observe_credited_for_send(send, original)
                .map(Some),
        )
    }

    pub(super) fn drain_plan_inner(&mut self) -> Result<DrainPlan> {
        use state::{DeferredArchiveDrainV1 as D, DeferredArchiveWaitV1 as W};
        let observed = match self.wallet.maintenance_view() {
            Ok(view) => view,
            Err(state::Error::Pending) => return Ok(DrainPlan::Pending),
            Err(error) => return Err(error.into()),
        };
        let snapshot = match observed {
            state::MaintenanceViewV1::BeforeBootstrap => return Ok(DrainPlan::Finished(D::NoWork)),
            state::MaintenanceViewV1::SelectedPending => return Ok(DrainPlan::Pending),
            state::MaintenanceViewV1::AwaitingArchiveFold(_) => {
                return Ok(DrainPlan::Finished(D::Blocked(W::ArchiveFold)));
            }
            state::MaintenanceViewV1::Released(snapshot) => snapshot,
        };
        let next = match self.wallet.reconcile_next_deferred_archive() {
            Ok(next) => next,
            Err(state::Error::Pending) => return Ok(DrainPlan::Pending),
            Err(error) => return Err(error.into()),
        };
        match next {
            None => return Ok(DrainPlan::Finished(D::NoWork)),
            Some(state::RequestStatusV1::Outcome(value)) => {
                return Ok(DrainPlan::Finished(D::Reconciled(value)));
            }
            Some(state::RequestStatusV1::Preparing) => {}
            Some(state::RequestStatusV1::Unknown) => return Err(Failure::code(CUSTODY_LOST)),
        }
        if snapshot.fold_backlog != 0 {
            return Ok(DrainPlan::Finished(D::Blocked(W::FoldBacklog)));
        }
        Ok(DrainPlan::Ready)
    }
}

/// Every wake only permits a new source read. Accepted exchange occupancy remains held,
/// while wallet ownership and both Payment guards are absent throughout the wait.
pub(super) fn await_fresh(
    invocation: &exchange::Invocation,
) -> Result<Option<state::DeferredArchiveWaitV1>> {
    let owner = invocation.owner();
    owner.background.retry(&owner)?;
    loop {
        invocation.validate()?;
        let view = with_wallet_owner(Arc::clone(&owner), Priority::None, |wallet| {
            wallet.maintenance()
        })?;
        match view {
            View::BeforeBootstrap => return Err(Failure::code(INVALID)),
            View::Released => return Ok(None),
            View::Pending => {
                with_wallet_owner(Arc::clone(&owner), Priority::Payment, |wallet| {
                    wallet.resume()
                })?;
                // One bounded recovery uses the selected operation. An unresolved result
                // never substitutes a new debit or grants fresh mutation permission.
                if matches!(
                    with_wallet_owner(Arc::clone(&owner), Priority::None, |wallet| wallet
                        .maintenance())?,
                    View::Pending
                ) {
                    return Ok(Some(state::DeferredArchiveWaitV1::SelectedPending));
                }
            }
            View::ArchiveFold => {
                owner.background.notify();
                // Surface an actual worker failure on this call: a serial SDK worker must
                // not need to enqueue a separate status read behind its own blocked wait.
                // Successful status DATA is never used as source state or admission.
                owner.background.status()?;
                owner
                    .scheduler
                    .wait_for_maintenance_change(std::time::Duration::from_secs(1))
                    .map_err(|error| Failure::from(state::Error::ArchiveAdmission(error)))?;
            }
        }
    }
}

pub(super) fn require_fresh(invocation: &exchange::Invocation) -> Result<()> {
    match await_fresh(invocation)? {
        None => Ok(()),
        Some(_) => Err(Failure::code(UNCERTAIN)),
    }
}

/// A fully fenced source observation. No returned value grants dispatch permission.
pub(super) fn source_view(owner: &Arc<Owner>) -> Result<View> {
    run(|| {
        with_wallet_owner(Arc::clone(owner), Priority::None, |wallet| {
            wallet.maintenance()
        })
    })
}
/// One selected recovery; the direct Core reservation cannot self-notify this worker.
pub(super) fn recover_selected(owner: &Arc<Owner>) -> Result<View> {
    run(|| {
        let _priority = owner.scheduler.payment_for(state::FoldCallClassV1::Payment);
        with_wallet_owner(Arc::clone(owner), Priority::None, |wallet| wallet.resume())
    })?;
    source_view(owner)
}
/// A completed rejection becomes progress only after archive publication and durability.
pub(super) fn receipt_step(owner: &Arc<Owner>) -> Result<ReceiptStep> {
    run(|| {
        with_wallet_owner(Arc::clone(owner), Priority::Ingest, |wallet| {
            wallet.deferred_receipt()
        })
    })
}
pub(super) fn observe_receipt(
    owner: &Arc<Owner>,
    send: &[u8; 32],
    original: &[u8],
) -> Result<ReceiptStep> {
    run(|| {
        with_wallet_owner(Arc::clone(owner), Priority::Ingest, |wallet| {
            wallet.observe_receipt(send, original)
        })
    })
}
pub(super) fn wake_hint(owner: &Arc<Owner>) -> Result<WakeHint> {
    run(|| {
        with_wallet_owner(Arc::clone(owner), Priority::None, |wallet| {
            wallet.deferred_wake_hint()
        })
    })
}

/// Internal bounded drain; no notification, recursive wait or foreign response decoding.
/// `recover` is false after the worker already spent its one recovery stage for this pass.
pub(super) fn drain_step(
    owner: &Arc<Owner>,
    recover: bool,
) -> Result<state::DeferredArchiveDrainV1> {
    use state::{
        ArchiveAdmissionErrorV1 as E, DeferredArchiveDrainV1 as D, DeferredArchiveWaitV1 as W,
    };
    match run(|| {
        with_wallet_owner(Arc::clone(owner), Priority::None, |wallet| {
            wallet.drain_plan()
        })
    })? {
        DrainPlan::Finished(value) => return Ok(value),
        DrainPlan::Pending => {
            if recover {
                recover_selected(owner)?;
            }
            return Ok(D::Blocked(W::SelectedPending));
        }
        DrainPlan::Ready => {}
    }
    let turn = match owner.scheduler.prepare_archive_turn() {
        Ok(turn) => turn,
        Err(E::ExchangeOpen) => return Ok(D::Blocked(W::ExchangeOpen)),
        Err(E::Busy) => return Ok(D::Blocked(W::Busy)),
        Err(error) => return Err(state::Error::ArchiveAdmission(error).into()),
    };
    run(|| {
        with_wallet_owner(Arc::clone(owner), Priority::None, |wallet| {
            wallet.drain_archive(turn)
        })
    })
}

/// One explicit bounded attempt, followed by a source-discovery notification on every result.
pub(super) fn drain(invocation: &exchange::Invocation) -> Result<Response> {
    invocation.validate()?;
    let owner = invocation.owner();
    owner.background.retry(&owner)?;
    let result = drain_step(&owner, true);
    owner.background.notify();
    drain_response(result?)
}

#[cfg(test)]
mod tests;
