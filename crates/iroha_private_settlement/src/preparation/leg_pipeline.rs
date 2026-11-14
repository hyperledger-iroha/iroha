//! Bounded staged preparation of the independent legs of one original round.
//!
//! Admission (context, wallet opening and reservation) and completion (validation, byte
//! budget and durable publication) stay on the original native worker in leg order. Only
//! proof construction, which reads nothing but its own admitted leg, overlaps. Every
//! prover joins, thread-local destruction included, before any proof is used, and the
//! first leg in order that was absent, refused or panicked decides the result exactly as
//! a leg-by-leg loop would: the legs before it are complete and durable, no later leg is.

use std::{
    any::Any,
    collections::VecDeque,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
};

/// Greatest number of legs one operation admits, proves and completes as a unit.
///
/// It bounds the reserved witnesses and unpublished proofs an operation holds at once and
/// the provers it runs together: its original native worker plus at most `LEG_WINDOW - 1`
/// helpers. A later window is admitted only after the previous one is durable.
pub(super) const LEG_WINDOW: usize = 4;

/// Process-wide allowance of prover threads beyond each operation's original native worker.
///
/// An operation always proves on its own admitted worker, so an exhausted allowance only
/// returns its legs to one proof at a time. A genuine leg prover holds about 1.5 GB of
/// working set, so the allowance bounds that additional memory for the whole process
/// however many sessions prepare at once. Proof arithmetic still runs on the one global
/// Rayon pool: helpers add callers, never Rayon width.
pub(super) struct ProverHelpers(AtomicUsize);

impl ProverHelpers {
    /// An allowance of `helpers` additional concurrent provers.
    pub(super) const fn new(helpers: usize) -> Self {
        Self(AtomicUsize::new(helpers))
    }

    // Never waits: a busy allowance must not delay the original worker's own proof.
    fn try_acquire(&self) -> Option<ProverHelper<'_>> {
        self.0
            .try_update(Ordering::AcqRel, Ordering::Acquire, |free| {
                free.checked_sub(1)
            })
            .ok()
            .map(|_| ProverHelper(self))
    }
}

/// The allowance shared by every preparation in this process.
pub(super) static PROCESS_PROVER_HELPERS: ProverHelpers = ProverHelpers::new(LEG_WINDOW - 1);

// Returned when the helper thread's closure ends, before that thread is joined.
struct ProverHelper<'a>(&'a ProverHelpers);

impl Drop for ProverHelper<'_> {
    fn drop(&mut self) {
        self.0.0.fetch_add(1, Ordering::AcqRel);
    }
}

/// What admission found for one leg, decided on the original worker before any proof.
pub(super) enum Admission<J, R> {
    /// A durable original was reopened and validated; nothing is proved again.
    Reopened(R),
    /// Every pre-proof check passed and the wallet inputs are reserved.
    Admitted(J),
    /// No durable original exists and this call may not prepare one.
    Absent,
}

/// One leg handed to completion, in leg order, after every prover of its window joined.
pub(super) enum Ready<P, R> {
    /// The durable original found at admission.
    Reopened(R),
    /// A proof its prover released after verifying it.
    Proved(P),
}

enum Outcome<P, E> {
    Proved(P),
    Refused(E),
    Panicked(Box<dyn Any + Send>),
}

// Why admission of a window ended before its last leg.
enum Stop<E> {
    Absent,
    Refused(E),
    Panicked(Box<dyn Any + Send>),
}

/// Prepare `legs` independent legs in windows of [`LEG_WINDOW`].
///
/// `admit` and `complete` run on the calling thread in leg order; `prove` runs for the
/// admitted legs of one window together, on the calling thread and on helpers from
/// `helpers`. Returns `None` when admission found an absent leg, after completing the
/// legs before it.
///
/// # Errors
/// The first failure in leg order. An admission or prover panic that comes first in leg
/// order resumes on the calling thread after the legs before it are complete.
pub(super) fn run_legs<J, R, P, D, E>(
    legs: usize,
    helpers: &ProverHelpers,
    mut admit: impl FnMut(usize) -> Result<Admission<J, R>, E>,
    prove: impl Fn(usize, J) -> Result<P, E> + Sync,
    mut complete: impl FnMut(usize, Ready<P, R>) -> Result<D, E>,
) -> Result<Option<Vec<D>>, E>
where
    J: Send,
    P: Send,
    E: Send,
{
    let mut completed = Vec::with_capacity(legs);
    let mut first = 0;
    while first < legs {
        let last = legs.min(first + LEG_WINDOW);
        let mut reopened = Vec::with_capacity(last - first);
        let mut jobs = VecDeque::new();
        // The leg that ended admission, if any: it decides after the legs before it.
        let mut stop = None;
        for index in first..last {
            // An unwinding wallet is that leg's outcome too: the admitted legs before it
            // are still proved and published before the original panic resumes.
            match catch_unwind(AssertUnwindSafe(|| admit(index))) {
                Ok(Ok(Admission::Reopened(original))) => reopened.push(Some(original)),
                Ok(Ok(Admission::Admitted(job))) => {
                    jobs.push_back((reopened.len(), job));
                    reopened.push(None);
                }
                Ok(Ok(Admission::Absent)) => {
                    stop = Some(Stop::Absent);
                    break;
                }
                Ok(Err(error)) => {
                    stop = Some(Stop::Refused(error));
                    break;
                }
                Err(payload) => {
                    stop = Some(Stop::Panicked(payload));
                    break;
                }
            }
        }
        let mut proofs = prove_admitted(first, reopened.len(), jobs, helpers, &prove);
        for (offset, original) in reopened.into_iter().enumerate() {
            let ready = match (original, proofs[offset].take()) {
                (Some(original), _) => Ready::Reopened(original),
                (None, Some(Outcome::Proved(proof))) => Ready::Proved(proof),
                (None, Some(Outcome::Refused(error))) => return Err(error),
                (None, Some(Outcome::Panicked(payload))) => resume_unwind(payload),
                (None, None) => {
                    panic!("an admitted private-settlement leg has no joined prover outcome")
                }
            };
            completed.push(complete(first + offset, ready)?);
        }
        match stop {
            Some(Stop::Absent) => return Ok(None),
            Some(Stop::Refused(error)) => return Err(error),
            Some(Stop::Panicked(payload)) => resume_unwind(payload),
            None => {}
        }
        first = last;
    }
    Ok(Some(completed))
}

// Prove one window's admitted legs and return their outcomes by window offset. Every
// started prover has exited before this returns; an unwinding prover is an outcome.
fn prove_admitted<J, P, E>(
    first: usize,
    width: usize,
    jobs: VecDeque<(usize, J)>,
    helpers: &ProverHelpers,
    prove: &(impl Fn(usize, J) -> Result<P, E> + Sync),
) -> Vec<Option<Outcome<P, E>>>
where
    J: Send,
    P: Send,
    E: Send,
{
    let pending = jobs.len();
    let queue = Mutex::new(jobs);
    let outcomes = Mutex::new((0..width).map(|_| None).collect::<Vec<_>>());
    let drain = || {
        loop {
            let next = queue
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pop_front();
            let Some((offset, job)) = next else {
                break;
            };
            let outcome = match catch_unwind(AssertUnwindSafe(|| prove(first + offset, job))) {
                Ok(Ok(proof)) => Outcome::Proved(proof),
                Ok(Err(error)) => Outcome::Refused(error),
                Err(payload) => Outcome::Panicked(payload),
            };
            outcomes.lock().unwrap_or_else(PoisonError::into_inner)[offset] = Some(outcome);
        }
    };
    if pending != 0 {
        std::thread::scope(|scope| {
            let drain = &drain;
            let mut started = Vec::new();
            // The calling worker is always one prover; helpers only add to it.
            while started.len() + 1 < pending {
                let Some(helper) = helpers.try_acquire() else {
                    break;
                };
                let spawned = std::thread::Builder::new()
                    .name("aps-leg-prover".into())
                    .spawn_scoped(scope, move || {
                        let _helper = helper;
                        drain();
                    });
                match spawned {
                    Ok(child) => started.push(child),
                    // The unspawned closure already returned its helper allowance.
                    Err(_) => break,
                }
            }
            drain();
            for child in started {
                // An explicit join includes the helper's thread-local destruction. A helper
                // unwinds only inside `prove`, where it is caught; a lost outcome fails
                // closed in `run_legs`.
                let _ = child.join();
            }
        });
    }
    outcomes
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
