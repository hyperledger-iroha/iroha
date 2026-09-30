//! Discrete write attempts retain the original FIFO and persist-before-effect barrier.

use super::*;

impl World {
    /// Attempt one due write; a failed fake-driver write remains pending until a later event.
    pub(super) fn io_attempt(&mut self, r: usize, id: u64) {
        if !self.replicas[r].io.is_ready(id, self.now) {
            return;
        }
        let m = self.replicas[r].machine;
        let epoch = self.machines[m].epoch;
        let profile = self.machines[m].profile;
        if !self.replicas[r].host.owns_io() && self.rng.chance(profile.write_fail_ppm) {
            if let Some((id, due)) = self.replicas[r].io.retry(self.now, profile.write_retry) {
                self.schedule(due, Ev::IoDone { r, epoch, id });
            }
            return;
        }
        match self.io_kill_at(r) {
            Some(true) => self.io_kill(r),
            Some(false) => self.io_done(r, id, true),
            None => self.io_done(r, id, false),
        }
        // A previous retry may have made the next write's original event stale.
        // Keep only one replacement event per head, not one per queued successor.
        if self.alive(r, epoch)
            && let Some(&(id, due, _)) = self.replicas[r].io.pending.front()
        {
            self.schedule(due, Ev::IoDone { r, epoch, id });
        }
    }
}

#[cfg(test)]
mod tests;
