//! Proposals (§6.2), execution requests and results (§6.3, §4.2) and the Prepare vote (§4.3).

use super::{Core, EvKey, ExecState, Held};
use crate::{
    api::{Action, ExecOutcome, LocalFault},
    crypto::verify_proposal_signature,
    message::{Block, Defect, Evidence, Proposal, VoteKind, WireMessage},
    pacemaker::exec_retry_delay,
    safety::RecordedVote,
    types::{Hash32, PublicKey},
};

/// §13.4 MS10a/MS10b: a TC-justified proposal is checked and executed like a fresh block (the
/// TC-rule branch of §6.2 step 6 and the binding of `Q.result` are gone). `false` unless mutated.
const NO_TC_RULE: bool = cfg!(any(
    sumeragi_mutation = "MS10a",
    sumeragi_mutation = "MS10b"
));

impl Core {
    /// §6.2 for a proposal of the current height (not awaiting).
    pub(super) fn on_proposal(&mut self, from: &PublicKey, p: Proposal) {
        let w = p.view;
        let bh = p.block_hash(&*self.crypto);
        let ad = p.att_digest(&*self.crypto);
        // Step 1 (after the byte-identical cheap reject): the signature of L(h, w).
        let exact = self.proposal.as_ref().is_some_and(|held| {
            held.p.view == w && held.bh == bh && held.ad == ad && held.p.sig == p.sig
        });
        if !exact {
            let leader = self.topo.leader(w);
            #[cfg(not(sumeragi_mutation = "MS18"))]
            let signed = verify_proposal_signature(
                &*self.crypto,
                &self.instance,
                &self.cfg.committee,
                leader,
                &p,
            )
            .is_ok();
            #[cfg(sumeragi_mutation = "MS18")]
            let signed = {
                let msg = crate::preimage::prop_preimage(&self.instance, self.height, w, &bh, &ad);
                p.instance == self.instance
                    && self.cfg.committee.get(leader).is_some()
                    && (self.cfg.committee.members().iter())
                        .any(|key| self.crypto.verify(key, &msg, &p.sig))
            };
            if !signed {
                return;
            }
        }
        // Step 2: duplicates and equivocation.
        if self.check_held(&p, bh, ad) {
            return;
        }
        // Step 3: justify (runs the TC handler, which may advance the view to w).
        let h0 = self.height;
        if let Some(defect) = self.check_justify(&p) {
            return self.signed_defect(p, defect);
        }
        if !self.same_height(h0) || self.check_held(&p, bh, ad) {
            return;
        }
        // Step 4: view.
        if w < self.view {
            if self.wants.contains_key(&bh)
                && let Some(block) = p.block()
            {
                self.on_body(block, false);
            }
            return;
        }
        if w > self.view {
            return;
        }
        // Steps 5–6: parent and header (signed content).
        if let Some(defect) = self.check_parent(&p).or_else(|| self.check_header(&p, bh)) {
            return self.signed_defect(p, defect);
        }
        self.accept_proposal(from, p, bh, ad);
    }

    /// Step 2: `true` if a proposal for `(h, p.view)` is already held (the duplicate may supply
    /// the missing body; a different one is equivocation evidence, and in the current view an
    /// early timeout: both proposals carry a valid signature of `L(h, p.view)`, so the leader is
    /// proven faulty and cannot be framed).
    fn check_held(&mut self, p: &Proposal, bh: Hash32, ad: Hash32) -> bool {
        let Some(held) = self.proposal.as_ref().filter(|held| held.p.view == p.view) else {
            return false;
        };
        if held.bh == bh && held.ad == ad {
            if !self.blocks.contains_key(&bh)
                && let Some(block) = p.block()
                && block.body_ok(&*self.crypto)
            {
                self.put_body(bh, block, false);
                self.after_body(bh);
            }
            return true;
        }
        let first = Box::new(held.p.clone());
        let mut second = Box::new(p.clone());
        second.payload = None;
        self.report(
            EvKey::Proposal(p.view),
            Evidence::ProposalEquivocation(first, second),
        );
        if p.view == self.view && !cfg!(sumeragi_mutation = "ML26") {
            self.sign_timeout(self.view);
        }
        true
    }

    /// Step 3. Returns the defect, or runs the TC handler on a valid justification.
    fn check_justify(&mut self, p: &Proposal) -> Option<Defect> {
        match (p.view, &p.justify) {
            (0, None) => None,
            (0, Some(_)) => Some(Defect::UnexpectedJustify),
            (_, None) => Some(Defect::MissingJustify),
            (w, Some(tc)) => {
                let valid = tc.instance == self.instance
                    && tc.height == self.height
                    && Some(tc.view) == w.checked_sub(1)
                    && self.verify_tc_cached(tc);
                if !valid {
                    return Some(Defect::InvalidJustify);
                }
                self.on_verified_tc(tc.clone(), true);
                None
            }
        }
    }

    /// Step 5: `parent_qc` is `None` iff `h == g + 1`, else a valid `CommitQC` of the tip.
    fn check_parent(&mut self, p: &Proposal) -> Option<Defect> {
        if cfg!(sumeragi_mutation = "MS19") {
            return None;
        }
        if self.height == self.genesis.saturating_add(1) {
            return p.parent_qc.is_some().then_some(Defect::UnexpectedParentQc);
        }
        let Some(qc) = &p.parent_qc else {
            return Some(Defect::MissingParentQc);
        };
        let shape_ok = qc.kind == VoteKind::Commit
            && Some(qc.height) == self.height.checked_sub(1)
            && qc.value() == (self.tip.block_hash, self.tip.result);
        let ok = shape_ok && (self.tip.commit_qc.as_ref() == Some(qc) || self.verify_qc_cached(qc));
        (!ok).then_some(Defect::InvalidParentQc)
    }

    /// Step 6: header checks, the TC rule (SR10) and the fresh-block rules.
    fn check_header(&self, p: &Proposal, bh: Hash32) -> Option<Defect> {
        let header = &p.header;
        let params = &self.cfg.params;
        let checks = [
            (header.instance != self.instance, Defect::HeaderInstance),
            (header.height != self.height, Defect::HeaderHeight),
            (
                header.parent_hash != self.tip.block_hash && !cfg!(sumeragi_mutation = "MS19"),
                Defect::ParentHash,
            ),
            (
                header.parent_result != self.tip.result && !cfg!(sumeragi_mutation = "MS19"),
                Defect::ParentResult,
            ),
            (
                header.payload_len > params.max_block_bytes,
                Defect::PayloadTooLarge,
            ),
        ];
        if let Some((_, defect)) = checks.into_iter().find(|(failed, _)| *failed) {
            return Some(defect);
        }
        let w = p.view;
        let certified = p
            .justify
            .as_ref()
            .and_then(|tc| tc.high_pqc.as_ref())
            .filter(|_| w > 0 && !NO_TC_RULE);
        if let Some(q) = certified {
            // Re-proposal: exactly Q's block; Q's honest signers checked the rest.
            return (bh != q.block_hash).then_some(Defect::TcRule);
        }
        let fresh = [
            (header.origin_view != w, Defect::OriginView),
            (header.proposer != self.topo.leader(w), Defect::Proposer),
            (
                header.skipped_leaders != self.topo.skipped_leader_keys(&self.cfg.committee, w),
                Defect::SkippedLeaders,
            ),
            (
                w >= params.empty_after_views
                    && header.payload_len != 0
                    && !cfg!(sumeragi_mutation = "ML13"),
                Defect::NonEmptyPayload,
            ),
            // §3.7 A1: `EMPTY` from `empty_after_views` on is never flagged (MA8 deletes it).
            (
                w >= params.empty_after_views && header.attest && !cfg!(sumeragi_mutation = "MA8"),
                Defect::FlaggedEmpty,
            ),
        ];
        fresh
            .into_iter()
            .find(|(failed, _)| *failed)
            .map(|(_, defect)| defect)
    }

    /// Step 7: evidence for a signed defect (payload stripped) and an early timeout if it is the
    /// current view's proposal (SR35).
    fn signed_defect(&mut self, mut p: Proposal, defect: Defect) {
        let w = p.view;
        p.payload = None;
        self.report(
            EvKey::Invalid(w),
            Evidence::InvalidProposal {
                proposal: Box::new(p),
                defect,
            },
        );
        if w == self.view {
            self.sign_timeout(w);
        }
    }

    /// Step 8: accept; take the body or want it; then step 9.
    fn accept_proposal(&mut self, from: &PublicKey, p: Proposal, bh: Hash32, ad: Hash32) {
        #[cfg(sumeragi_mutation = "MS35")]
        if p.block().is_some_and(|block| !block.body_ok(&*self.crypto)) {
            return self.signed_defect(p, Defect::PayloadTooLarge);
        }
        let block = p.block().filter(|block| block.body_ok(&*self.crypto));
        let q = p
            .justify
            .as_ref()
            .and_then(|tc| tc.high_pqc.clone())
            .filter(|_| p.view > 0 && !NO_TC_RULE);
        let mut stripped = p;
        stripped.payload = None;
        let leader = self.member_key(self.topo.leader(stripped.view));
        self.proposal = Some(Held {
            p: stripped,
            bh,
            ad,
            expected: q.as_ref().map(|q| q.result),
        });
        self.t_prop = Some(self.now);
        match block {
            #[cfg(not(sumeragi_mutation = "ML10"))]
            Some(block) => self.put_body(bh, block, false),
            #[cfg(sumeragi_mutation = "ML10")]
            Some(block) => self.put_body(bh, block, true),
            None if !self.blocks.contains_key(&bh) => {
                // SPEC: besides §6.2 step 8's sources, the relaying peer is asked too (it held
                // the proposal a moment ago; bodies are self-verifying, so this is harmless).
                // (Appendix E, E27)
                let mut sources: Vec<PublicKey> = leader.into_iter().collect();
                sources.push(from.clone());
                if let Some(q) = &q {
                    sources.extend(self.signer_keys(q));
                }
                self.want(bh, self.height, sources);
            }
            None => {}
        }
        self.maybe_execute();
    }

    /// Accept this node's own proposal (§6.10 rule 3, then §6.2 steps 8–9).
    pub(super) fn accept_own(&mut self, p: Proposal, bh: Hash32, ad: Hash32) {
        let expected = p
            .justify
            .as_ref()
            .and_then(|tc| tc.high_pqc.as_ref())
            .filter(|_| !NO_TC_RULE)
            .map(|q| q.result);
        let mut stripped = p;
        stripped.payload = None;
        self.proposal = Some(Held {
            p: stripped,
            bh,
            ad,
            expected,
        });
        self.t_prop = Some(self.now);
        self.maybe_execute();
    }

    /// Store a body in memory (`StoreBody` unless it came from the local stores). A held body
    /// is never replaced. Callers checked `body_ok` (SR20).
    pub(super) fn put_body(&mut self, bh: Hash32, block: Block, from_store: bool) {
        if self.blocks.contains_key(&bh) {
            return;
        }
        if !from_store {
            self.out.push(Action::StoreBody {
                block: block.clone(),
            });
        }
        self.wants.remove(&bh);
        self.blocks.insert(bh, block);
    }

    /// Step 9: execute the current proposal's block once its body and parent state are there.
    pub(super) fn maybe_execute(&mut self) {
        let Some(bh) = self.proposal.as_ref().map(|held| held.bh) else {
            return;
        };
        if !self.blocks.contains_key(&bh) {
            return;
        }
        // §9.2: the committing view's duration is measured from here, when this node first
        // holds the round's proposal together with its body (a leader cannot stretch it by
        // sending either late).
        if self.t_body.is_none() {
            self.t_body = Some(self.now);
        }
        let parent_ready = self.applied.saturating_add(1) >= self.height || self.tip.exec_ok;
        if !parent_ready {
            return;
        }
        match self.exec.get(&bh) {
            None => self.request_exec(bh, 0),
            Some(ExecState::Valid(_) | ExecState::Invalid) => self.on_outcome(bh),
            Some(ExecState::Pending { .. } | ExecState::RetryAt { .. }) => {}
        }
    }

    /// `request_exec(bh)` (§6.2): a fresh request id per request, never reused.
    pub(super) fn request_exec(&mut self, bh: Hash32, attempt: u32) {
        let Some(block) = self.blocks.get(&bh).cloned() else {
            self.exec.remove(&bh);
            return;
        };
        let req = self.next_req;
        self.next_req = self.next_req.saturating_add(1);
        self.exec.insert(
            bh,
            ExecState::Pending {
                since: self.now,
                req,
                attempt,
            },
        );
        self.out.push(Action::Execute { block, req });
    }

    /// `discard_exec(keep)` (§6.2): the only source of `DiscardExecution`. Every `Pending`
    /// execution it removes first counts for the start level with its elapsed time, a lower
    /// bound of an execution that outlasted its view (§9.2): its answer, if any, is ignored.
    pub(super) fn discard_exec(&mut self, mut keep: Vec<Hash32>) {
        keep.sort();
        keep.dedup();
        let pending: Vec<Hash32> = self
            .exec
            .keys()
            .filter(|bh| !keep.contains(bh))
            .copied()
            .collect();
        for bh in pending {
            self.record_pending_exec(bh);
        }
        #[cfg(not(sumeragi_mutation = "ML17"))]
        self.exec.retain(|bh, _| keep.contains(bh));
        self.out.push(Action::DiscardExecution {
            height: self.height,
            keep,
        });
    }

    /// §9.2: if the execution of `bh` is still `Pending`, record its elapsed time `now − since`
    /// (a lower bound of its duration) in `pm.last_exec_ms`. Called when the entry is discarded
    /// and, for the committed block, at the commit (§6.8 step 4).
    pub(super) fn record_pending_exec(&mut self, bh: Hash32) {
        if cfg!(sumeragi_mutation = "ML27") {
            return;
        }
        if let Some(ExecState::Pending { since, .. }) = self.exec.get(&bh) {
            let elapsed = self.now.saturating_sub(*since);
            self.pm.record_exec(elapsed);
        }
    }

    /// §6.3 on `Executed`.
    pub(super) fn on_executed(&mut self, bh: Hash32, req: u64, outcome: &ExecOutcome) {
        // Step 0: the execution of the committed block that was still pending at its commit.
        if bh == self.tip.block_hash && self.tip.exec_req == Some(req) {
            self.tip.exec_req = None;
            match outcome {
                ExecOutcome::Valid(result) if *result == self.tip.result => {
                    self.tip.exec_ok = true;
                    if !self.awaiting && self.halted.is_none() {
                        self.maybe_execute();
                    }
                }
                ExecOutcome::Valid(_) | ExecOutcome::Invalid => {
                    let view = self.tip.commit_qc.as_ref().map_or(0, |qc| qc.view);
                    self.local_fault(LocalFault::ExecutionMismatch {
                        height: self.tip.height,
                        view,
                    });
                }
                // The driver executes the block when it applies it (O3).
                ExecOutcome::Failed(_) | ExecOutcome::Cancelled => {}
            }
            return;
        }
        let Some(ExecState::Pending {
            since,
            req: expected,
            attempt,
        }) = self.exec.get(&bh).copied()
        else {
            return;
        };
        if expected != req {
            return;
        }
        self.pm.record_exec(self.now.saturating_sub(since));
        let state = match outcome {
            ExecOutcome::Valid(result) => ExecState::Valid(*result),
            ExecOutcome::Invalid => ExecState::Invalid,
            #[cfg(sumeragi_mutation = "MS36b")]
            ExecOutcome::Failed(_) => ExecState::Invalid,
            ExecOutcome::Failed(_) | ExecOutcome::Cancelled => {
                let delay = exec_retry_delay(attempt, self.local.rebroadcast_interval);
                self.exec.insert(
                    bh,
                    ExecState::RetryAt {
                        at: self.now.saturating_add(delay),
                        attempt: attempt.saturating_add(1),
                    },
                );
                if matches!(outcome, ExecOutcome::Failed(_)) {
                    self.local_fault(LocalFault::ExecutorFailed {
                        height: self.height,
                    });
                }
                return;
            }
        };
        self.exec.insert(bh, state);
        self.on_outcome(bh);
    }

    /// Results a held `PrepareQC` certifies for `bh` (a re-proposal's `Q`, or the lock).
    fn certified_results(&self, bh: Hash32) -> Vec<Hash32> {
        let mut out = Vec::new();
        if let Some(held) = self.proposal.as_ref().filter(|held| held.bh == bh) {
            out.extend(held.expected);
        }
        if let Some(q) = self.high_pqc.as_ref().filter(|q| q.block_hash == bh) {
            out.push(q.result);
        }
        out
    }

    /// §6.3 steps 3–4 for the current proposal's block with a recorded outcome.
    fn on_outcome(&mut self, bh: Hash32) {
        // The held proposal of the current view only (a view change clears it; checked here
        // too, like `try_prepare`'s condition 3, so a stale proposal never times a view out).
        // MS3 removes this guard with the other two.
        let stale = |held: &Held| held.p.view != self.view && !cfg!(sumeragi_mutation = "MS3");
        if (self.proposal.as_ref()).is_none_or(|held| held.bh != bh || stale(held)) {
            return;
        }
        let certified = self.certified_results(bh);
        let fault = LocalFault::ExecutionMismatch {
            height: self.height,
            view: self.view,
        };
        match self.exec.get(&bh) {
            Some(ExecState::Invalid) if certified.is_empty() => self.reject_payload(bh),
            #[cfg(sumeragi_mutation = "MS36a")]
            Some(ExecState::Invalid) => self.reject_payload(bh),
            #[cfg(sumeragi_mutation = "MS36a")]
            Some(ExecState::Valid(r)) if certified.iter().any(|e| e != r) => {
                self.reject_payload(bh)
            }
            // A certified block the local executor disagrees with: local only (SR36).
            Some(ExecState::Invalid) => self.local_fault(fault),
            Some(ExecState::Valid(r)) if certified.iter().any(|e| e != r) => {
                self.local_fault(fault);
            }
            Some(ExecState::Valid(_)) => {
                let h0 = self.height;
                self.try_prepare();
                // §3.7 A2: an attestor that answered `Pending` for a flagged lock needed this
                // execution; ask it again (MA12: only a stage raise does).
                let flagged = self.high_pqc.as_ref().is_some_and(|q| q.attest);
                if flagged && self.same_height(h0) && !cfg!(sumeragi_mutation = "MA12") {
                    self.try_commit();
                }
            }
            _ => {}
        }
    }

    /// Deterministic invalidity of an uncertified, `body_ok`, fresh block (SR35): no evidence
    /// (the leader does not execute before proposing, §3.6), the builder quarantines the culprit
    /// transactions, the view is abandoned at once.
    fn reject_payload(&mut self, bh: Hash32) {
        self.out.push(Action::PayloadRejected {
            height: self.height,
            view: self.view,
            block_hash: bh,
        });
        self.sign_timeout(self.view);
    }

    /// `try_prepare()`: the complete §4.3 predicate (S2), then persist, sign, route, pool.
    pub(super) fn try_prepare(&mut self) {
        let Some(me) = self.signer() else {
            return;
        };
        let view = self.view;
        // Condition 1: the timeout fence.
        #[cfg(not(sumeragi_mutation = "MS4"))]
        if self.timeout_view.is_some_and(|t| t >= view) {
            return;
        }
        // Condition 2: sign-once.
        if self.safety.as_ref().is_none_or(|r| {
            r.prepare.is_some_and(|v| v.view >= view) && !cfg!(sumeragi_mutation = "MS2")
        }) {
            return;
        }
        // Conditions 3–4: the held proposal of this view, its body, its valid result.
        let Some((bh, attest)) = self
            .proposal
            .as_ref()
            .filter(|held| held.p.view == view || cfg!(sumeragi_mutation = "MS3"))
            .map(|held| (held.bh, held.p.header.attest))
        else {
            return;
        };
        if !self.blocks.contains_key(&bh) {
            return;
        }
        let Some(ExecState::Valid(result)) = self.exec.get(&bh).copied() else {
            return;
        };
        if self.certified_results(bh).iter().any(|e| *e != result) {
            return;
        }
        if self.t_ready.is_none() {
            self.t_ready = Some(self.now);
        }
        // Condition 5: set A, or stage ≥ 1.
        if !self.rnd.in_set_a(me.index) && (self.stage < 1 || cfg!(sumeragi_mutation = "ML3")) {
            return;
        }
        #[cfg(not(sumeragi_mutation = "MS27"))]
        if let Some(record) = self.safety.as_mut() {
            record.prepare = Some(RecordedVote {
                view,
                block_hash: bh,
                result,
                attest,
            });
        }
        #[cfg(not(sumeragi_mutation = "MS23"))]
        self.persist();
        self.cast_vote(me, VoteKind::Prepare, (bh, result, attest), None);
        #[cfg(sumeragi_mutation = "MS23")]
        self.persist();
    }

    /// `on_block_request` (§6.9 rule 4): answer from memory, else ask the driver to answer from
    /// its body store or block store (any height).
    pub(super) fn on_block_request(&mut self, to: PublicKey, height: u64, bh: Hash32) {
        #[cfg(sumeragi_mutation = "ML11")]
        if height <= self.tip.height {
            return;
        }
        match self.blocks.get(&bh).filter(|b| b.header.height == height) {
            Some(block) => {
                let msg = WireMessage::BlockResponse(crate::message::BlockResponse {
                    instance: self.instance,
                    block: block.clone(),
                });
                self.send(to, msg);
            }
            None => self.out.push(Action::ServeBody {
                to,
                height,
                block_hash: bh,
            }),
        }
    }
}
