//! `Tick` (§6.11): due deadlines fire in the order propose/build, stage timers, vote
//! retransmits, view timeout, rebroadcast, `Status`, probe, sync retry, fetch retry, execution
//! retry. Every deadline that fires is consumed, so `next_wakeup()` always moves forward.

use super::{Core, ExecState};
use crate::{
    message::WireMessage,
    pacemaker::retransmit_spacing,
    types::{Hash32, Millis, PublicKey},
};

impl Core {
    /// The earliest pending deadline (§12.1); `Millis::MAX` when halted.
    pub fn next_wakeup(&self) -> Millis {
        if self.halted.is_some() {
            return Millis::MAX;
        }
        [
            self.build_deadline(),
            self.stage1_deadline(),
            self.stage2_deadline(),
            self.retx[0].map(|r| r.next),
            self.retx[1].map(|r| r.next),
            self.view_deadline(),
            Some(
                self.last_rebroadcast
                    .saturating_add(self.local.rebroadcast_interval),
            ),
            Some(self.status_deadline()),
            self.probe_deadline(),
            self.sync.deadline(),
            self.wants.values().map(|want| want.next_retry).min(),
            self.exec
                .values()
                .filter_map(|state| match state {
                    ExecState::RetryAt { at, .. } => Some(*at),
                    _ => None,
                })
                .min(),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(Millis::MAX)
    }

    /// Every deadline by name (tests).
    #[cfg(test)]
    pub(super) fn deadlines(&self) -> Vec<(&'static str, Option<Millis>)> {
        vec![
            ("build", self.build_deadline()),
            ("stage1", self.stage1_deadline()),
            ("stage2", self.stage2_deadline()),
            ("retx0", self.retx[0].map(|r| r.next)),
            ("retx1", self.retx[1].map(|r| r.next)),
            ("view", self.view_deadline()),
            (
                "rebroadcast",
                Some(
                    self.last_rebroadcast
                        .saturating_add(self.local.rebroadcast_interval),
                ),
            ),
            ("status", Some(self.status_deadline())),
            ("probe", self.probe_deadline()),
            ("sync", self.sync.deadline()),
            (
                "fetch",
                self.wants.values().map(|want| want.next_retry).min(),
            ),
            (
                "exec",
                self.exec
                    .values()
                    .filter_map(|state| match state {
                        ExecState::RetryAt { at, .. } => Some(*at),
                        _ => None,
                    })
                    .min(),
            ),
        ]
    }

    /// `on_tick` (§6.11).
    pub(super) fn on_tick(&mut self) {
        let h0 = self.height;
        self.build_tick();
        self.stage_tick();
        if self.same_height(h0) {
            self.retransmit_tick();
        }
        if self.view_deadline().is_some_and(|t| self.now >= t) {
            self.sign_timeout(self.view);
        }
        if self.now
            >= self
                .last_rebroadcast
                .saturating_add(self.local.rebroadcast_interval)
        {
            self.rebroadcast();
        }
        if self.now >= self.status_deadline() {
            self.broadcast_status();
        }
        if self.probe_deadline().is_some_and(|t| self.now >= t) {
            self.probe_tick();
        }
        self.sync_tick();
        self.fetch_tick();
        self.exec_retry_tick();
    }

    /// `t_s1 = min(t_ready + t_retx [no PrepareQC of the view], t_pqc + t_retx [no CommitQC of
    /// the height])` (§5.2 stage 1a; the second clause catches a member that Prepares but
    /// withholds its Commit). In the round no `CommitQC` of `h` is held by construction.
    pub(super) fn stage1_deadline(&self) -> Option<Millis> {
        if self.awaiting || self.stage >= 1 {
            return None;
        }
        let ready = self.t_ready.filter(|_| !self.has_pqc_of_view());
        let t_pqc = self.t_pqc.filter(|_| !cfg!(sumeragi_mutation = "ML22"));
        self.pm.stage1_deadline(ready, t_pqc, self.view)
    }

    /// `t_s2 = min(t_lastvote + 2·t_retx, anchor + φ·T)` (§5.2 stage 2a/2b).
    pub(super) fn stage2_deadline(&self) -> Option<Millis> {
        if self.awaiting || self.stage >= 2 {
            return None;
        }
        // The phase of the latest vote; its QC ends trigger (a). A set-B member that has not
        // voted counts from `t_ready + t_retx` in the Prepare phase.
        let last = if self.mine.commit.is_some() {
            self.t_lastvote
        } else if self.mine.prepare.is_some() {
            self.t_lastvote.filter(|_| !self.has_pqc_of_view())
        } else {
            let set_b = self.my_index().is_some_and(|me| self.rnd.in_set_b(me));
            self.t_ready
                .filter(|_| set_b && !self.has_pqc_of_view())
                .map(|t| t.saturating_add(self.pm.t_retx(self.view)))
        };
        #[cfg(sumeragi_mutation = "ML4")]
        let last = None;
        Some(self.pm.stage2_deadline(last, self.anchor(), self.view))
    }

    fn stage_tick(&mut self) {
        let h0 = self.height;
        if self.stage1_deadline().is_some_and(|t| self.now >= t) {
            self.raise_stage(1);
            if !self.same_height(h0) {
                return;
            }
        }
        if self.stage2_deadline().is_some_and(|t| self.now >= t) {
            self.raise_stage(2);
        }
    }

    /// Vote retransmission (§6.11): the only re-send path for votes, at
    /// `t_vote + t_retx·(2^k − 1)` until the phase's QC is held.
    fn retransmit_tick(&mut self) {
        let t_retx = self.pm.t_retx(self.view);
        let cap = self.local.rebroadcast_interval;
        for slot in 0..2 {
            let Some(mut retx) = self.retx[slot] else {
                continue;
            };
            if self.now < retx.next {
                continue;
            }
            let vote = if slot == 0 {
                (self.mine.prepare.clone()).filter(|_| !self.has_pqc_of_view())
            } else {
                self.mine.commit.clone()
            };
            let Some(vote) = vote else {
                self.retx[slot] = None;
                continue;
            };
            #[cfg(not(sumeragi_mutation = "ML5a"))]
            self.route(&vote);
            retx.k = retx.k.saturating_add(1);
            retx.next = self
                .now
                .saturating_add(retransmit_spacing(retx.k, t_retx, cap));
            self.retx[slot] = Some(retx);
        }
    }

    /// The view deadline `t_view = anchor + T(level(h, view))` while not timed out (§6.6).
    pub(super) fn view_deadline(&self) -> Option<Millis> {
        if self.signer().is_none() || self.timeout_view.is_some_and(|t| t >= self.view) {
            return None;
        }
        Some(self.pm.view_deadline(self.anchor(), self.view))
    }

    /// Rebroadcast (§6.11): the own timeout of the current view, and the leader's proposal
    /// re-push to members that evidently lack it.
    fn rebroadcast(&mut self) {
        self.last_rebroadcast = self.now;
        if self.awaiting {
            return;
        }
        #[cfg(not(sumeragi_mutation = "MS9"))]
        let resent = self.mine.timeout.clone();
        // MS9: the timeout of the view is re-signed with the current lock instead of re-sent.
        #[cfg(sumeragi_mutation = "MS9")]
        let resent = (self.mine.timeout.as_ref())
            .filter(|_| self.timeout_view == Some(self.view))
            .and_then(|_| self.signer())
            .and_then(|me| self.build_timeout(me, self.view, self.high_pqc.clone()));
        #[cfg(not(sumeragi_mutation = "ML2"))]
        if self.timeout_view == Some(self.view)
            && let Some(timeout) = resent
        {
            let to = self.members_except_me();
            self.broadcast(to, WireMessage::Timeout(Box::new(timeout)));
        }
        self.repush_proposal();
    }

    /// Re-send the own proposal (with payload) once per member per view to members whose
    /// latest `Status`, received at least `rebroadcast_interval` after the proposal was sent,
    /// is for `(h, view)` without it.
    fn repush_proposal(&mut self) {
        let (Some(p), Some(sent)) = (self.mine.proposal.clone(), self.proposal_sent_at) else {
            return;
        };
        let Some(block) = self
            .proposal
            .as_ref()
            .and_then(|held| self.blocks.get(&held.bh))
        else {
            return;
        };
        let bh = block.hash(&*self.crypto);
        let payload = block.payload.clone();
        let threshold = sent.saturating_add(self.local.rebroadcast_interval);
        let targets: Vec<(u32, PublicKey)> = self
            .peers
            .iter()
            .filter(|(_, peer)| {
                peer.height == self.height
                    && peer.view == self.view
                    && peer.status_at.is_some_and(|at| at >= threshold)
                    && peer.proposal_hash != Some(bh)
            })
            .filter_map(|(key, _)| Some((self.cfg.committee.index_of(key)?, key.clone())))
            .filter(|(index, _)| !self.repushed.contains(index))
            .collect();
        for (index, key) in targets {
            self.repushed.insert(index);
            let mut full = p.clone();
            full.payload = Some(payload.clone());
            self.send(key, WireMessage::Proposal(Box::new(full)));
        }
    }

    /// Unsettled (§6.11): timed out in the current view, `view > 0`, awaiting, a verified sync
    /// target above `h` (unverified hints never count), a late entry without a proposal of
    /// `(h, view)`, or stage 2 in the current round.
    // SPEC: §6.11 has no stage-2 clause. A withholding proxy tail can deliver the PrepareQC to
    // some honest members only; at stage 2 those no longer re-send their Prepare (its phase QC
    // is held), so the others cannot form the PrepareQC from broadcast votes, and the lock
    // reached them only through a keepalive `Status` (found by the simulator: F3 seed 4,
    // `Deliver::Half`, gaps above P3 after the revision-4 removal of the "no commit in
    // 2·rebroadcast_interval" clause). A node at stage 2 sends its `Status` (which carries its
    // lock) at the unsettled cadence (Appendix E, E5).
    fn unsettled(&self) -> bool {
        #[cfg(not(sumeragi_mutation = "MR-unverified-target"))]
        let target = self.sync.verified_target();
        #[cfg(sumeragi_mutation = "MR-unverified-target")]
        let target = self.sync.target();
        self.timeout_view == Some(self.view)
            || self.view > 0
            || self.awaiting
            || target > self.height
            || (self.late_entry && self.proposal.is_none())
            || (self.stage >= 2 && !cfg!(sumeragi_mutation = "ME5"))
    }

    fn status_deadline(&self) -> Millis {
        let interval = if self.unsettled() && !cfg!(sumeragi_mutation = "ML15") {
            self.local.rebroadcast_interval
        } else {
            self.local.status_keepalive
        };
        self.last_status.map_or(0, |t| t.saturating_add(interval))
    }

    /// `Broadcast(Status)` to `C_h ∪ C_{h+1}` (except self).
    fn broadcast_status(&mut self) {
        self.last_status = Some(self.now);
        let mut to = self.members_except_me();
        if let Some(next) = self.configs.get(&self.height.saturating_add(1)) {
            for key in next.committee.members() {
                if !to.contains(key) && !self.is_local_key(key) {
                    to.push(key.clone());
                }
            }
        }
        let status = self.status_message();
        self.broadcast(to, WireMessage::Status(Box::new(status)));
    }

    /// The probe deadline (§6.11): every `rebroadcast_interval` while some key is unanchored and
    /// `C_{tip.height+2}` is known.
    fn probe_deadline(&self) -> Option<Millis> {
        if !self.any_unanchored()
            || !self
                .configs
                .contains_key(&self.tip.height.saturating_add(2))
        {
            return None;
        }
        Some(
            self.last_probe
                .saturating_add(self.local.rebroadcast_interval),
        )
    }

    /// Probe (§6.11, §7.4 R2): `Send(Status{probe: Some(nonce)})` to each member of
    /// `C_{tip.height+2}` other than this node.
    fn probe_tick(&mut self) {
        self.last_probe = self.now;
        let Some(next) = self.configs.get(&self.tip.height.saturating_add(2)) else {
            return;
        };
        let to: Vec<PublicKey> = next
            .committee
            .members()
            .iter()
            .filter(|key| !self.is_local_key(key))
            .cloned()
            .collect();
        let status = self.status_message();
        self.broadcast(to, WireMessage::Status(Box::new(status)));
    }

    /// Execution retries that are due (§4.2), while the block is still needed.
    fn exec_retry_tick(&mut self) {
        let due: Vec<(Hash32, u32)> = self
            .exec
            .iter()
            .filter_map(|(bh, state)| match state {
                ExecState::RetryAt { at, attempt } if *at <= self.now => Some((*bh, *attempt)),
                _ => None,
            })
            .collect();
        for (bh, attempt) in due {
            self.request_exec(bh, attempt);
        }
    }
}
