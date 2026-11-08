//! Exchange-local cancellation for ephemeral suffix verification and unused reviews.
//!
//! This does not own an accepted monetary child or selected Advance recovery. Ordinary
//! exchanges retain their existing accepted-child semantics. Lock order is lifetime then
//! review cell; cancellation and review destructors run after both locks are released.
use std::sync::{Arc, Mutex, Weak};

use super::{state, Failure, Result, CLOSED, INTERNAL, INVALID, RESOURCE};

const MAX_REVIEWS: usize = 8;

pub(crate) type ExchangeLifetime = Lifetime<state::ReviewedSuffixUnloadV1>;
pub(crate) type SuffixReviewCell = ReviewCell<state::ReviewedSuffixUnloadV1>;

struct Job {
    id: u64,
    cancel: state::SuffixPreparationCancellationV1,
}

struct State<T> {
    ended: bool,
    next_job: u64,
    job: Option<Job>,
    reviews: Vec<Weak<Mutex<Option<T>>>>,
}

/// Clones refer to one exact exchange; a new default is a separate origin.
pub(crate) struct Lifetime<T> {
    state: Arc<Mutex<State<T>>>,
}

impl<T> Default for Lifetime<T> {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                ended: false,
                next_job: 0,
                job: None,
                reviews: Vec::new(),
            })),
        }
    }
}

impl<T> Clone for Lifetime<T> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl<T> Lifetime<T> {
    pub(crate) fn require_live(&self) -> Result<()> {
        let state = self.state.lock().map_err(|_| Failure::code(INTERNAL))?;
        if state.ended {
            Err(Failure::code(CLOSED))
        } else {
            Ok(())
        }
    }

    /// Poison cannot make a review usable. Cleanup still drains a poisoned state in `end`.
    pub(crate) fn ended(&self) -> bool {
        self.state.lock().map_or(true, |state| state.ended)
    }

    pub(crate) fn end(&self) {
        let (job, reviews) = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.ended = true;
            let job = state.job.take();
            let mut reviews = Vec::with_capacity(state.reviews.len());
            for weak in state.reviews.drain(..) {
                if let Some(cell) = weak.upgrade() {
                    let review = cell
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .take();
                    if let Some(review) = review {
                        reviews.push(review);
                    }
                }
            }
            (job, reviews)
        };
        // Cancelling a proof or dropping a scheduler-owned review may release another
        // lock. Neither action is allowed beneath the exchange lifetime lock.
        if let Some(job) = job {
            job.cancel.cancel();
        }
        drop(reviews);
    }

    pub(crate) fn register_job(
        &self,
        cancel: state::SuffixPreparationCancellationV1,
    ) -> Result<JobRegistration<T>> {
        let outcome = (|| {
            let mut state = self.state.lock().map_err(|_| Failure::code(INTERNAL))?;
            if state.ended {
                return Err(Failure::code(CLOSED));
            }
            if state.job.is_some() {
                return Err(Failure::code(RESOURCE));
            }
            let id = state
                .next_job
                .checked_add(1)
                .ok_or_else(|| Failure::code(RESOURCE))?;
            cancel.check().map_err(Failure::from)?;
            state.next_job = id;
            state.job = Some(Job {
                id,
                cancel: cancel.clone(),
            });
            Ok(JobRegistration {
                lifetime: self.clone(),
                id,
            })
        })();
        // A rejected handoff must not leave the newly offered verification job running.
        // This handle belongs to the offered job, never the already registered job.
        if outcome.is_err() {
            cancel.cancel();
        }
        outcome
    }

    pub(crate) fn review(&self, review: T) -> Result<ReviewCell<T>> {
        let mut state = self.state.lock().map_err(|_| Failure::code(INTERNAL))?;
        if state.ended {
            return Err(Failure::code(CLOSED));
        }
        state.reviews.retain(|weak| weak.strong_count() != 0);
        if state.reviews.len() >= MAX_REVIEWS {
            return Err(Failure::code(RESOURCE));
        }
        let cell = Arc::new(Mutex::new(Some(review)));
        state.reviews.push(Arc::downgrade(&cell));
        Ok(ReviewCell {
            lifetime: self.clone(),
            cell,
        })
    }
}

/// A bridge call retains this registration while its exact bounded proof job runs.
pub(crate) struct JobRegistration<T> {
    lifetime: Lifetime<T>,
    id: u64,
}

impl<T> JobRegistration<T> {
    pub(crate) fn check(&self) -> Result<()> {
        let state = self
            .lifetime
            .state
            .lock()
            .map_err(|_| Failure::code(INTERNAL))?;
        if state.ended {
            return Err(Failure::code(CLOSED));
        }
        let job = state
            .job
            .as_ref()
            .filter(|job| job.id == self.id)
            .ok_or_else(|| Failure::code(CLOSED))?;
        job.cancel.check().map_err(Failure::from)
    }
}

impl<T> Drop for JobRegistration<T> {
    fn drop(&mut self) {
        let job = {
            let mut state = self
                .lifetime
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.job.as_ref().is_some_and(|job| job.id == self.id) {
                state.job.take()
            } else {
                None
            }
        };
        if let Some(job) = job {
            job.cancel.cancel();
        }
    }
}

/// Opaque one-use review. Projection DATA never enters this cell.
pub(crate) struct ReviewCell<T> {
    lifetime: Lifetime<T>,
    cell: Arc<Mutex<Option<T>>>,
}

impl<T> ReviewCell<T> {
    pub(crate) fn ended(&self) -> bool {
        self.lifetime.ended()
    }

    pub(crate) fn lifetime(&self) -> Lifetime<T> {
        self.lifetime.clone()
    }

    /// End and take serialize on the same lifetime lock. A successful take transfers the
    /// review to execution; the caller must register/check its exact proof job separately.
    pub(crate) fn take(self) -> Result<T> {
        let state = self
            .lifetime
            .state
            .lock()
            .map_err(|_| Failure::code(INTERNAL))?;
        if state.ended {
            return Err(Failure::code(CLOSED));
        }
        self.cell
            .lock()
            .map_err(|_| Failure::code(INTERNAL))?
            .take()
            .ok_or_else(|| Failure::code(INVALID))
    }
}

#[cfg(test)]
#[path = "lifetime/tests.rs"]
mod tests;
