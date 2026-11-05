//! Bounded startup observations, never authority, retries, or retained signing material.

use std::{fmt::Write as _, sync::Mutex, time::Instant};

use super::{Mode, OperationStatus};
use crate::managed::{Error, ManagedBootstrapFailure, provider_funding::FundingStep};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::managed) enum Stage {
    Parent,
    Reserve,
    Custody,
    Account,
    Funding,
    Ingest,
    Gateway,
    Reputation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::managed) enum ErrorClass {
    Bootstrap(ManagedBootstrapFailure),
    NoSelection,
    Io(std::io::ErrorKind),
    Invalid,
    CapacityPredecessor,
    CapacityPreparation,
    CapacityRecovery,
    RuntimeMismatch,
    CheckpointHttp(iroha::http::PublicHttpError),
    CheckpointProof,
    ContractCall,
    Busy,
    Timeout,
    NativeDeadline,
    WorkerFailure,
    ParentDeadline,
    ParentProgressDeadline,
}
impl ErrorClass {
    pub(in crate::managed) fn of(error: &Error) -> Self {
        match error {
            Error::Bootstrap(value) => Self::Bootstrap(*value),
            Error::NoSelection => Self::NoSelection,
            Error::Io(value) => Self::Io(value.kind()),
            Error::Invalid(message) => match message.as_str() {
                "fresh capacity predecessor unavailable"
                | "fresh capacity declaration predecessor unavailable; retain original intent" => {
                    Self::CapacityPredecessor
                }
                "provider capacity declaration preparation failed; retain original intent and journal" => {
                    Self::CapacityPreparation
                }
                "provider capacity declaration transaction unresolved; recover its original journal" => {
                    Self::CapacityRecovery
                }
                _ => Self::Invalid,
            },
            Error::RuntimeMismatch(_) => Self::RuntimeMismatch,
            Error::ParentCheckpoint { source, .. } => match source.as_ref() {
                crate::bootstrap::CheckpointReadError::Http(value) => Self::CheckpointHttp(*value),
                crate::bootstrap::CheckpointReadError::Bootstrap(_) => Self::CheckpointProof,
            },
            Error::ContractCall { .. } => Self::ContractCall,
            Error::Busy(_) => Self::Busy,
            Error::Timeout(_) => Self::Timeout,
            Error::NativeDeadline => Self::NativeDeadline,
            Error::WorkerFailure { .. } => Self::WorkerFailure,
            Error::ParentDeadline => Self::ParentDeadline,
            Error::ParentProgressDeadline { .. } => Self::ParentProgressDeadline,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::managed) enum Outcome {
    Running,
    Complete,
    Pending(OperationStatus),
    Funding(FundingStep, OperationStatus),
    Error(ErrorClass),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Observation {
    stage: Stage,
    mode: Mode,
    calls: u16,
    failures: u16,
    attempts: u16,
    outcome: Outcome,
    last_error: Option<ErrorClass>,
    started_ms: u32,
    elapsed_ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Running,
    Complete,
    Pending(OperationStatus),
    Funding(FundingStep, OperationStatus),
    Error,
}
impl From<Outcome> for State {
    fn from(value: Outcome) -> Self {
        match value {
            Outcome::Running => Self::Running,
            Outcome::Complete => Self::Complete,
            Outcome::Pending(status) => Self::Pending(status),
            Outcome::Funding(step, status) => Self::Funding(step, status),
            Outcome::Error(_) => Self::Error,
        }
    }
}

// The failure travels by value through existing Result owners. Keep only the displayed
// scalars and one error class, without duplicating a clock or error in each observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Summary {
    stage: Stage,
    mode: Mode,
    calls: u16,
    attempts: u16,
    outcome: State,
    last_error: Option<ErrorClass>,
    elapsed_ms: u32,
}

/// Five fixed owners: parent call, network phases, and the three provider workers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::managed) struct Snapshot([Option<Summary>; 5]);
impl Snapshot {
    pub(in crate::managed) fn message(self) -> String {
        let mut message = String::new();
        for (slot, observation) in self.0.into_iter().enumerate() {
            if let Some(value) = observation {
                let label = ["parent", "network", "provider0", "provider1", "provider2"][slot];
                // Every displayed value is a closed enum or scalar. No Error::Display,
                // profile, endpoint, provider identifier, path, or response is retained.
                let _ = write!(
                    message,
                    " {label}={:?}/{:?},calls={},attempts={},outcome={:?},last_error={:?},elapsed_ms={};",
                    value.mode,
                    value.stage,
                    value.calls,
                    value.attempts,
                    value.outcome,
                    value.last_error,
                    value.elapsed_ms
                );
            }
        }
        message
    }
}

pub(in crate::managed) struct Progress {
    started: Instant,
    observations: Mutex<[Option<Observation>; 5]>,
}
impl Default for Progress {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            observations: Mutex::new([None; 5]),
        }
    }
}
impl Progress {
    fn elapsed_ms(&self) -> u32 {
        u32::try_from(self.started.elapsed().as_millis()).unwrap_or(u32::MAX)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, [Option<Observation>; 5]> {
        self.observations
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub(super) fn begin(&self, stage: Stage, mode: Mode, provider: Option<usize>) {
        let Some(index) = index(stage, provider) else {
            return;
        };
        let now = self.elapsed_ms();
        let mut observations = self.lock();
        let previous = observations[index];
        observations[index] = Some(Observation {
            stage,
            mode,
            calls: previous.map_or(1, |value| value.calls.saturating_add(1)),
            failures: previous.map_or(0, |value| value.failures),
            attempts: previous
                .map_or(0, |value| value.attempts)
                .saturating_add(u16::from(
                    stage == Stage::Parent || (mode == Mode::Advance && stage == Stage::Custody),
                )),
            outcome: Outcome::Running,
            last_error: previous.and_then(|value| value.last_error),
            started_ms: now,
            elapsed_ms: 0,
        });
    }

    pub(super) fn finish(&self, stage: Stage, provider: Option<usize>, outcome: Outcome) {
        let Some(index) = index(stage, provider) else {
            return;
        };
        let now = self.elapsed_ms();
        if let Some(value) = &mut self.lock()[index] {
            value.elapsed_ms = now.saturating_sub(value.started_ms);
            value.outcome = outcome;
            if let Outcome::Error(error) = outcome {
                value.failures = value.failures.saturating_add(1);
                value.last_error = Some(error);
            }
        }
    }

    pub(in crate::managed) fn parent_begin(&self, advancing: bool) {
        self.begin(
            Stage::Parent,
            if advancing {
                Mode::Advance
            } else {
                Mode::Recover
            },
            None,
        );
    }

    // Complete here means the parent call returned; child observations retain pending states.
    pub(in crate::managed) fn parent_finish<T>(&self, result: &crate::managed::Result<T>) {
        self.finish(
            Stage::Parent,
            None,
            match result {
                Ok(_) => Outcome::Complete,
                Err(error) => Outcome::Error(ErrorClass::of(error)),
            },
        );
    }

    pub(in crate::managed) fn snapshot(&self) -> Option<Snapshot> {
        let now = self.elapsed_ms();
        let observations = *self.lock();
        let snapshot = Snapshot(observations.map(|value| {
            value.map(|value| Summary {
                stage: value.stage,
                mode: value.mode,
                calls: value.calls,
                attempts: value.attempts,
                outcome: value.outcome.into(),
                last_error: value.last_error,
                elapsed_ms: if value.outcome == Outcome::Running {
                    now.saturating_sub(value.started_ms)
                } else {
                    value.elapsed_ms
                },
            })
        }));
        snapshot.0.iter().any(Option::is_some).then_some(snapshot)
    }
}

fn index(stage: Stage, provider: Option<usize>) -> Option<usize> {
    match (stage, provider) {
        (Stage::Parent, None) => Some(0),
        (_, None) => Some(1),
        (_, Some(slot @ 0..3)) => Some(slot + 2),
        _ => None,
    }
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
