//! Local diagnostic for the dynamic extent of cached paid-token BeforeAdmission only.

use super::StreamTokenIssuerError;
use std::{cell::Cell, io::Write};

#[derive(Clone, Copy)]
struct Facts {
    at: &'static str,
    floor: Option<u64>,
    candidate: Option<u64>,
    latest: Option<u64>,
    observer_floor: Option<u64>,
}

std::thread_local! {
    static ACTIVE: Cell<Option<Facts>> = const { Cell::new(None) };
}

struct Restore(Option<Facts>);

impl Drop for Restore {
    fn drop(&mut self) {
        ACTIVE.set(self.0);
    }
}

fn update(f: impl FnOnce(&mut Facts)) {
    if let Some(mut facts) = ACTIVE.get() {
        f(&mut facts);
        ACTIVE.set(Some(facts));
    }
}

pub(super) fn stage(at: &'static str) {
    update(|s| s.at = at);
}

pub(super) fn heights(candidate: Option<u64>, floor: u64, latest: Option<u64>) {
    update(|s| {
        s.candidate = candidate.or(s.candidate);
        s.floor = Some(floor);
        s.latest = latest.or(s.latest);
    });
}

pub(super) fn refused(condition: bool, at: &'static str) -> bool {
    if condition {
        stage(at);
    }
    condition
}

/// The native observer reports only the static floor branch that declined the original request.
#[derive(Clone, Copy, Debug)]
pub enum ObserverFloorRefusal {
    /// The native custody control reader refused.
    Read,
    /// The original native control row was absent.
    Absent,
    /// The complete original minimum anchor did not match.
    MinimumAnchor,
    /// The selected candidate was below the original minimum.
    CandidateBelowFloor,
    /// Native certification of the selected floor refused.
    Finality,
}

/// Record a refusal only inside this thread's active cached paid admission.
pub fn observer_floor_refusal(kind: ObserverFloorRefusal, candidate: u64, minimum: u64) {
    update(|s| {
        s.at = match kind {
            ObserverFloorRefusal::Read => "observer_floor_read",
            ObserverFloorRefusal::Absent => "observer_floor_absent",
            ObserverFloorRefusal::MinimumAnchor => "observer_floor_anchor",
            ObserverFloorRefusal::CandidateBelowFloor => "observer_candidate_below_floor",
            ObserverFloorRefusal::Finality => "observer_floor_finality",
        };
        s.candidate = Some(candidate);
        s.observer_floor = Some(minimum);
    });
}

pub(super) fn admission<T>(
    call: impl FnOnce() -> Result<T, StreamTokenIssuerError>,
) -> Result<T, StreamTokenIssuerError> {
    let active = Facts {
        at: "before_admission_body",
        floor: None,
        candidate: None,
        latest: None,
        observer_floor: None,
    };
    let previous = Restore(ACTIVE.replace(Some(active)));
    let result = call();
    let facts = ACTIVE.get().unwrap_or(active);
    drop(previous);
    let variant = match &result {
        Ok(_) => "accepted",
        Err(StreamTokenIssuerError::SignerStateChanged) => "signer_state_changed",
        Err(StreamTokenIssuerError::SignerEvidenceInvalid) => "signer_evidence_invalid",
        Err(StreamTokenIssuerError::SignerFinalityUnavailable) => "signer_finality_unavailable",
        Err(StreamTokenIssuerError::RuntimeSignerUnavailable) => "runtime_signer_unavailable",
        Err(StreamTokenIssuerError::SignerClockRollback) => "signer_clock_rollback",
        Err(StreamTokenIssuerError::SignerBindingMismatch) => "signer_binding_mismatch",
        Err(StreamTokenIssuerError::RuntimeSignerRefused) => "runtime_signer_refused",
        Err(StreamTokenIssuerError::RuntimeSignerOutputInvalid) => "runtime_signer_output_invalid",
        Err(StreamTokenIssuerError::BeforeProviderTipAdvanced) => "before_provider_tip_advanced",
        Err(StreamTokenIssuerError::TimeOverflow) => "time_overflow",
        Err(_) => "other_issuer_refusal",
    };
    let branch = if result.is_ok() { "accepted" } else { facts.at };
    let _ = writeln!(
        std::io::stderr().lock(),
        "[paid ready admission] phase=BeforeAdmission result={variant} branch={branch} candidate={:?} latest={:?} floor={:?} observer_floor={:?}",
        facts.candidate,
        facts.latest,
        facts.floor,
        facts.observer_floor
    );
    result
}
