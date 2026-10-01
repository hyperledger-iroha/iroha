//! Finite completion of progress observations under the original O-LIVE deadlines.

use super::World;
use crate::types::Millis;

/// One immutable height target; only an observed commit advances its existing live window.
struct ProgressWatch {
    replica: usize,
    target: u64,
    committed: u64,
    deadline: Millis,
}

impl ProgressWatch {
    fn observe(&mut self, world: &World) -> Result<bool, String> {
        let obs = &world.oracle.reps[self.replica];
        if obs.committed > self.committed {
            // A changed oracle window, restarted timer, or new precondition cannot forgive a
            // commit after the deadline already captured for this observation.
            if obs.last_commit > self.deadline {
                return Err(self.failure());
            }
            self.committed = obs.committed;
            self.deadline = obs.deadline;
        }
        if self.committed >= self.target {
            return Ok(true);
        }
        if world.now > self.deadline {
            return Err(self.failure());
        }
        Ok(false)
    }

    fn failure(&self) -> String {
        format!(
            "O-LIVE: progress observation of honest replica {} missed captured deadline {} at height {} (target {})",
            self.replica, self.deadline, self.committed, self.target,
        )
    }
}

impl World {
    /// Finish the outstanding finite height targets without granting any extra liveness time.
    /// `duration` remains the minimum observation window, restored before the final oracles.
    pub(super) fn complete_progress_observation(&mut self) {
        if !self.checks.liveness || self.failure.is_some() {
            return;
        }
        let mut pending = Vec::new();
        for r in self.honest() {
            let Some((base, expect)) = self.progress_obligation(r) else {
                continue;
            };
            let obs = &self.oracle.reps[r];
            if obs.committed.saturating_sub(base) >= expect {
                continue;
            }
            // This option does not manufacture a live window while its precondition is false.
            // Leave the unchanged final progress assertion to report that scenario mismatch.
            if !self.live_precondition(self.replicas[r].inst) || obs.deadline == 0 {
                return;
            }
            let Some(target) = base.checked_add(expect) else {
                return self.fail("progress observation target exceeds u64".into());
            };
            pending.push(ProgressWatch {
                replica: r,
                target,
                committed: obs.committed,
                deadline: obs.deadline,
            });
        }
        let minimum = self.duration;
        while self.failure.is_none() && !pending.is_empty() {
            pending.retain_mut(|watch| match watch.observe(self) {
                Ok(done) => !done,
                Err(error) => {
                    self.fail(error);
                    true
                }
            });
            if self.failure.is_some() || pending.is_empty() {
                break;
            }
            let deadline = pending.iter().map(|watch| watch.deadline).min().unwrap();
            let Some(end) = deadline.checked_add(1) else {
                self.fail("progress observation deadline exceeds Millis".into());
                break;
            };
            self.duration = end;
            if !self.step() {
                // Empty queues must not silently pass or hang: cross the same captured
                // deadline and let the next observation reject the missing commit.
                self.now = self.now.max(end);
            }
        }
        self.duration = minimum;
    }
}

#[cfg(test)]
mod tests;
