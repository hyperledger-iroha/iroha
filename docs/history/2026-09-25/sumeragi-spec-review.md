# Sumeragi specification review logs (revisions 2 to 4.1)

These are the logs of the adversarial reviews of the Sumeragi rewrite specification on 2026-09-25
and 2026-09-26, one per revision (2, 3, 4 and 4.1). They were Appendices A–D of
`crates/iroha_sumeragi/spec.md` and are kept here verbatim. The specification is
[`specs/sumeragi.md`](../../../specs/sumeragi.md). Section references (§) below use its numbering;
its Appendix E records the later as-built reconciliation.

---

## Appendix A. Review log (revision 2)

Every finding of the adversarial review, with its disposition. "Fixed" means the protocol text
changed; the section says where. Parts of findings that were not adopted are explained below the
table. Dispositions marked *rev. 3* were changed by the decisions recorded in Appendix B.

| # | Lens | Finding | Disposition | Where |
|---|---|---|---|---|
| 1 | safety | Bodies accepted after checking `block_hash` only | Fixed: `body_ok` at every intake, wanted-only acceptance, no replacement of held bodies, `payload_len` in header | §3.2, §6.2, §6.9, MS20 |
| 2 | safety | Local certificate carries own undurable Commit out via `CommitBlock`/serving | Fixed: O2 is a total barrier for every externally visible effect | §7.4, §12.3 O2, MS24 |
| 3 | safety | Unsigned `justify`/payload tamperable; evidence against honest leaders | Fixed: signature covers `justify` and `parent_qc` (`ad`); payload defects are silent drops; duplicates by `(bh, ad)` | §3.3, §6.2, MS35 |
| 4 | safety | Missing record treated as a fresh start | Fixed: R2 abstains at `≤ t+2` unless `Fresh`; R5 failure halts; R6 abstains (*rev. 4*: R2 is a peer probe, `Fresh` removed, record provenance rules; Appendix C #0) | §7.4, MS31, MS32 |
| 5 | safety | Timeout/proposal rebuilt for re-send | Fixed: exact stored messages; record stores the carried QC and the proposal's `justify`; a restarted leader never regenerates | §6.6, §6.10, §7.4, MS9, MS29 |
| 6 | safety | Key rotation vs single signer and R1 | Fixed: per-key records, signer chosen by membership | §7.4, §10.3, MS33 |
| 7 | safety | How G learns `C_{Di,h}` (stale committee certifies forged AMX) | Fixed: epoch-start-only changes + sequential handoff proofs | §11.7 |
| 8 | safety | Executor failures cached as `Invalid` | Fixed: `Failed` outcome; certified mismatch is a local fault | §4.2, §6.3, MS36 |
| 9 | safety | Restart/persistence mutations missing | Fixed: one mutation per safety rule, each with a named deterministic test | §7.1, §13.4 |
| 10 | safety | Signer intersection wrongly treated as guilty across views | Fixed: attribution only for the same `(I,h,v,kind)` | §3.6, §7.6 |
| 11 | safety | Monitor window vs committee pruning | Fixed: monitor `{tip−1, tip}` only | §7.6 |
| 12 | safety | Fallback restates the vote predicate incompletely | Fixed: stage entry calls `try_prepare()`/`try_commit()` only | §5.2 |
| 13 | liveness | Unsigned proposal fields let a relay force early timeouts | Fixed (as #3) | §3.3, §6.2 |
| 14 | liveness | Fetched bodies never checked against `payload_hash` | Fixed (as #1) | §3.2, §6.9 |
| 15 | liveness | Uncommitted bodies not durable | Fixed: `StoreBody` before any signature about the body; retained until applied; local-first fetch | §7.4, §6.9, ML10 |
| 16 | liveness | Entry TC not persisted → restart deadlock | Fixed: `high_tc` in every record; L1 re-proved | §7.4, §8.2 L1, ML9 |
| 17 | liveness | `pending_apply` fetch rejected after peers moved on | Fixed: `BlockRequest` served at any height (`ServeBody`); wants accept any height | §6.9, ML11 |
| 18 | liveness | Local executor failures treated as invalidity | Fixed (as #8) | §4.2 |
| 19 | liveness | Validation weaker than `T_req`; apply on the Prepare path | Fixed: one formula with `a_max` and fetch term; speculative certified parent | §8.2 L3, §9.4, §4.1 |
| 20 | liveness | Block size limit is local | Fixed: chain parameter, header `payload_len`, frame limit O10 | §3.2, §6.2, §12.3 |
| 21 | liveness | No feedback from `Invalid` to the builder | Fixed: `PayloadRejected`; O-TXP | §4.2, §12.1, ML14 |
| 22 | liveness | `block_time > idle_block_interval` not rejected | Fixed | §9.4 |
| 23 | liveness | Early-proposal buffer evictable | Fixed: buffer removed; proposals carry `parent_qc` | §3.3, §6.2 step 0 |
| 24 | liveness | Leader's proposal not persisted | Fixed (as #5 and #15) | §6.10, §7.4 |
| 25 | liveness | Superseded executions never cancelled | Fixed: `DiscardExecution` on view change with keep list; newest first (*rev. 3*: entries removed at every discard, every request answered, B3) | §6.12, O4 |
| 26 | topology | Previous proposer can grind the topology seed | Fixed: per-committee permutation, round-robin; O-CQ (*rev. 3*: slot substitution, B1) | §2.1 |
| 27 | topology | Set A has no slack; fallback is O(n²) | Fixed: stage ladder (retransmit, set B via P, broadcast), sticky mode removed | §5.2, §5.3 |
| 28 | topology | Re-chaining only for skipped leaders; `W` does not scale | *rev. 3* (B1): absence-based demotion removed again; skipped leaders only, `|D| ≤ f`, `W = 128`; silent set-A members handled by the stage ladder and its hint | §2.1, §5.2 |
| 29 | topology | Fallback timer from round entry, level-scaled, no contagion | Fixed: readiness-based stage timers, `f+1` contagion, anchor on proposal receipt | §5.2, §9.1 |
| 30 | topology | View-0 timers shorter than the execution budget | Fixed: `exec_budget` hint, anchor on proposal, `P(0)`/validation rules (held-proposal bit not adopted, below) | §9.1, §9.4 |
| 31 | topology | No parent CommitQC in proposals; P(h) = L(h+1) | Fixed: `parent_qc`, buffer removed (P/L coupling not adopted, below) | §3.3, §6.2 |
| 32 | topology | Demoted members re-enter via the proxy-tail slot; `|D|` unbounded | Fixed: `|D| ≤ f`; views rotate over non-demoted members only (*rev. 3*: heights anchored at permutation slots, slot substitution, B1) | §2.1 |
| 33 | topology | Status all-to-all with full certificates, no verify-once | Fixed: cheap rejects, certificate cache, per-peer rate, adaptive cadence (digest pull not adopted, below) | §6.1, §6.11 |
| 34 | topology | Leader re-push races Status; amplification | Fixed: once per member per view after an interval; pull via wants | §6.11 |
| 35 | topology | Sticky broadcast renewal ambiguous | Fixed: sticky mode removed | §5.2 |
| 36 | topology | 1 s idle heartbeat too eager; no wake-up | Fixed: `PayloadReady`, 5 s default, `t_tx` anchor, sync overlapping fetch and apply (*rev. 3*: one outstanding request, B4; *rev. 4*: `t_tx` removed, Appendix C #6) | §6.10, §9, §6.9 |
| 37 | topology | Set B's payload copies compete with set A's | Fixed: `Broadcast` sends set A first (relay through set A not adopted, below) | §6.0, §12.1 |
| 38 | topology | Latency claims wrong (5 hops, not 4) | Fixed | §2.2, §5.1 |
| 39 | topology | Verify-before-pool forbids optimistic aggregate verification | *rev. 3* (B2): withdrawn; every vote is verified before pooling; batch verification is a future optimisation (§14.11) | §6.1, §6.4 |
| 40 | topology | Executing `h+1` waits for durable apply of `h` | Fixed: speculative certified parent | §4.1, §6.2 step 9 |
| 41 | topology | No push path to observers | Partly: proposals and CommitQCs also go to the next committee's joiners; observer subscription is a driver goal | §6.0, §14.9 |
| 42 | integration | Locked/committed bodies not durable (cluster restart) | Fixed (as #15) | §7.4, F32 |
| 43 | integration | Body fetch reply dropped; sync re-download; fetch ownership undefined | Fixed: wants as sole owner, any-height `BlockResponse`, sync from `max(h, buffered+1)` with buffer gating | §6.9 |
| 44 | integration | No traffic classes / event priority / isolation; verify before dedup | Fixed: O5, O6, O8, O9; cheap rejects and cache in the core; CPU model in the simulator | §12.3, §6.1, ML12 |
| 45 | integration | Verification covers only the core | Fixed: generic production driver run in the simulator, conformance oracles, soak gate | §13.5 |
| 46 | integration | Mandatory mutations missing; only seed sweeps | Fixed (as #9) | §13.4 |
| 47 | integration | Liveness oracle too loose | Fixed: O-PERF with P1–P6 and pacemaker mutations | §8.2, §13.2, ML1–ML18 |
| 48 | integration | AMX abort path, escrow bound, cross-committee proofs | Fixed: `Begin` registration, abort by default, no unilateral release, handoff chain, idempotence sets, O-AMX | §11 |
| 49 | integration | Size limits local or missing | Fixed (as #20) plus byte-capped sync responses accepted as prefixes | §3.5, §6.9 |
| 50 | integration | Chain parameters and key fixed at `Core::new` | Fixed: `HeightConfig` with lag 2; per-key records | §10.1, §7.4 |
| 51 | integration | Benign conditions cause fail-stop or false evidence | Fixed: R6 abstain-and-resync; `Failed`; certified mismatch local | §7.4, §4.2 |
| 52 | integration | Proposals cannot be re-sent after restart | Fixed (as #5) | §6.10 |
| 53 | integration | Intake filter drops service requests and `h+1` votes silently | Fixed: service messages never height-filtered; `Status` reply to `h+1` senders | §6.1 |
| 54 | integration | `Broadcast` has no recipient set | Fixed: `Broadcast{to}` resolved by the core, incl. `C_{h+1}` joiners | §12.1, §6.0 |
| 55 | integration | Monitor window vs retention | Fixed (as #11) | §7.6 |
| 56 | integration | Validation formula weaker than `T_req`; `A_max` not a parameter | Fixed (as #19) | §9.4 |
| 57 | integration | Defaults lose throughput at n≈20 | Fixed: `W = 128`, vote retransmit, re-push rule (*rev. 3*: absence demotion removed, stage-1 hint instead, B1) | §2.1, §5, §6.11 |
| 58 | integration | DS quorum loss unrecoverable and unstated | Fixed: stated; recovery outside the core; no core override | §10.8, §14.10 |
| 59 | integration | Storage durability on the Prepare path | Fixed (as #40) | §4.1 |
| 60 | integration | Smaller ambiguities (scheme, key bytes, signer, tables, buffer, awaiting) | Fixed: BlsNormal min-pk and `kb`, local non-blocking signer, bounded LRU, buffer removed, `awaiting` state | §1.6, §3.1, §12.1, §6.8 |

**Parts not adopted, with rationale.**

- *#1 "Have `R` commit to transaction hashes."* Unnecessary: `block_hash` binds `payload_hash` and
  `payload_len`, and `body_ok` is enforced at every intake (§3.2), so a certified `bh` fixes the
  payload bytes; two honest nodes cannot commit different bodies under one `(bh, R)`.
- *#30 "Timeouts carry a signed held-proposal bit to exempt leaders from demotion."* Not adopted:
  the header could only verify the bit for the last view (earlier views' TCs are not in the
  proposal), and the problem it addresses — honest heavy blocks failing views — is removed at the
  source by `exec_budget` and the proposal-anchored view timer. A leader wrongly demoted loses only
  its turns for `W` heights (liveness-neutral: `|D| ≤ f`).
- *#31 "Place `L(h+1,0)` at the proxy-tail slot of `h`."* Not adopted: it couples two roles to one
  node, so a single slow or Byzantine member would cost both the CommitQC of `h` and the proposal of
  `h+1`. The `parent_qc` in every proposal already gives the merge the finding wanted (one fewer
  hop for most members under load) without the coupling.
- *#33 "Status carries digests; certificates are pulled."* Not adopted: it adds a request/response
  round to the one mechanism that must work under heavy loss. The same CPU and bandwidth bounds are
  obtained by cheap rejects (a certificate is verified only if it would change state), the verified-
  certificate cache, one processed `Status` per peer per half interval, and the settled keepalive
  cadence.
- *#37 "Set-A members relay the payload to set B (B-Chain style)."* Not adopted for the first
  release: it adds a hop and a relay failure mode to set B's readiness for stage 1. Set B pulls
  through its wants if the leader is slow. Tracked as §14.7.
- *#38 option (b) "Set A sends Commit votes to everyone."* Not adopted: it costs `q(n−1)` messages
  per height to save one hop, which is exactly the trade the star exists to avoid. The
  `parent_qc` merge already recovers that hop under load.
- *Seed grinding: "unique-signature seed per height."* Not adopted in favour of the per-committee
  round-robin, which is fair (slot fairness since revision 3, §2.1) and removes grinding
  completely; the predictability it introduces is recorded as §15.3.

---

## Appendix B. Review log (revision 3)

The skeptical review of revision 2 raised the issues below; the decisions were taken by the
orchestrator and are binding. This appendix records what changed and where.

| # | Issue / decision | Change | Where |
|---|---|---|---|
| B1 | Re-chaining too complex: absence records in headers, `f + 1`-distinct-proxy attribution, and a rotation modulo `a_h` that re-indexes every member's role whenever `D_h` changes. **Decision:** remove absence-based demotion; keep skipped-leader demotion (`|D_h| ≤ f`, window `W`); slot substitution. | Rule (b) of §2.1, header fields `absent` / `parent_proxy`, their `block_hash` encoding, the fresh-header checks, `absent_set`, ML16 (old), its golden vectors and the F6 proxy-omission clause deleted. §2.1 now anchors view 0 of height `h` at permutation slot `h mod n`; a demoted slot passes to the next non-demoted member; views rotate over non-demoted members; the order is the stably partitioned rotation starting at the leader. Fairness, locality, `f + 1` distinct leaders and set-A entry stated precisely. Stage-1 hint added so silent set-A members need no demotion; P2–P4 and the throughput paragraph rewritten; ML7 retargeted, ML16 (hint) and ML18 (view rotation) added. | §2.1, §2.2, §3.1, §3.2, §5.2, §5.3, §6.2, §6.8, §6.10, §6.12, §8.2, §8.3, §13.3, §13.4, §15.4 |
| B2 | Deferred (optimistic) vote verification lets unverified votes influence pools and triggers. **Decision:** remove it. | Every vote is verified before it enters a pool; cheap rejects compare only against verified entries; stage triggers, contagion and formation count only pool entries. SR38/MS38 added (local formation skips re-verification, so pooling unverified votes would be a safety bug). | §5.2, §6.1, §6.4, §7.1, §13.4 |
| B3 | Cancelled execution (skeptic new problem 1): a `Pending` entry whose work the driver cancelled on `DiscardExecution` was never answered, so a later re-proposal of that block (hidden PrepareQC) was never executed or voted on; `executed: true` could name a post-state the driver had dropped. **Decision:** remove entries on every discard, the driver answers every `Execute`, `exec_ok` only while the post-state is held, otherwise re-execute. | `discard_exec(keep)` removes every entry not kept; `Execute`/`Executed` carry a request id `req`, so stale answers (including `Cancelled`, and a `Valid` finished before the discard) never match a live entry; new outcome `Cancelled`; O3/O4 require an answer to every `Execute`, dropping discarded post-states, and recomputation (never `Failed`, `Invalid` or `ApplyDiverged`) when a needed post-state is missing. The lock is now updated before `advance_to` on a higher PrepareQC, so that view change keeps the locked block's execution. Deterministic test `det_l17_hidden_pqc_reexecutes` (three answer orders), mutation ML17, scenario F33, oracle O-FAULT, conformance checks for O3/O4, fake executor with eviction. | §4.1, §4.2, §6.0, §6.2, §6.3, §6.5, §6.8, §6.12, §6.13, §8.4, §12.1, §12.3, §13.1–§13.5 |
| B4 | General simplicity pass (core ≤ 8K lines). | Removed or moved to §14.11, listed below; §12.6 adds a per-module line budget enforced by CI. | §6.9, §6.11, §7.6, §9.3, §9.4, §12.4, §12.6, §14 |
| B5 | Consistency fixes found while applying B1–B4. | `Init` also carries the configuration of the tip height `t` (needed at `t + 1` to verify `parent_qc` and compute the hint); `qc_lat_ewma` sampling defined so hinted rounds still sample; the stage-1 hint is recomputed at every round entry. | §6.8, §6.12, §7.4, §9.1, §12.1 |

**Deviation from the literal slot-substitution formula (B1).** The decision text defined
`base_{h,v}[i] = perm_C[(h + v + i) mod n]` and asked to re-check that `f + 1` consecutive views
still have `f + 1` distinct leaders. With that formula they do not: for `n = 4`, `perm = [A, B, C,
D]`, `D_h = {B}` and `h mod 4 = 1`, views 0 and 1 both have leader C (slot 1 passes to C, slot 2
is C's own), so a Byzantine C leads `f + 1 = 2` consecutive views. Revision 3 therefore keeps
slot substitution across *heights* (view 0 of `h` is anchored at slot `h mod n`) but rotates
*views* over the non-demoted list (`L(h, v) = nd_h[(k0(h) + v) mod a_h]`). With `D_h = ∅` the
two are identical; with demoted members they differ only for views `> 0` and in the rotation of
the demoted tail. ML18 kills the literal formula.

**Removed or moved (simplicity pass).**
1. Deferred (optimistic) vote verification — removed (B2); batch verification is §14.11a.
2. Absence-based demotion, header fields `absent` / `parent_proxy`, `absent_set`, the
   `f + 1`-distinct-proxy rule, old ML16 and their vectors — removed (B1); §14.11b.
3. Rotation modulo `a_h` over the active list — replaced by slot substitution (B1).
4. Multi-request sync pipelining (`sync_inflight`, per-range deadlines) — replaced by one
   outstanding request whose successor is sent when a response is checked (fetch still overlaps
   apply); §14.11c. `B_live`, §8.4, §9.3, §9.4 and `LocalParams` updated.
5. Duplicate vote re-send path — votes were re-sent both by the retransmit schedule and by every
   rebroadcast; now only by the retransmit schedule (via `route()`), rebroadcast re-sends only the
   timeout (and the leader's proposal re-push).
6. Same-view conflicting-certificate evidence at the current height (§7.6 second bullet) —
   removed: cheap rejects meant it was never reached; forensics are §14.6.

**Kept after review, with the reason** (each is needed for safety, for P1–P6 or for a user
decision, except where noted): QC answer by `P` and vote retransmit (P1 at `n = 22` under 1 % loss); leader re-push
(awaiting nodes drop the next proposal; P1 max-gap); contagion and the readiness timer (P3);
start-level adaptation and `exec_budget` (slow executors; ML1, ML6); `t_tx` anchor (*removed in
revision 4*, Appendix C #6: it let a node's own queue move its view timer) (not needed
for the P4 bound, which uses `P(0)`, but one timestamp makes a crashed leader under load cost
`≈ block_time + build_timeout + T` instead of `idle_block_interval + build_timeout + T`, 3.2 s
instead of 7.2 s at the defaults, and without absence demotion every crashed member still takes
one leader turn per `W` heights); certificate cache
(timeouts carry the same PrepareQC; view change CPU at `n = 22`); TC unicast to the next leader
(stateless); stage-1 hint (new, stateless; makes P2 hold without demotion).


---

## Appendix C. Review log (revision 4)

The second adversarial review of revision 3 produced 41 findings (numbered as in the review, 0–40);
the critical/high ones were independently confirmed. The orchestrator's decisions (binding) were:
remove the `t_tx` anchor; a store-independent R2 with normative record provenance; a targeted,
bounded re-push for late entrants; `P` broadcasts its CommitQC; `BlockApplied` carries the applied
header; exact code-level mutations whose named tests can fail; and a disposition for every
medium/low finding. "Fixed" means the protocol text changed; "Rejected" gives the reason.

| # | Sev. | Finding | Disposition | Where |
|---|---|---|---|---|
| 0 | high | Rolled-back or re-created record bypasses R2/R3 (record + Kura tail lost; key reinstalled) → fork with one Byzantine member | Fixed. R2 no longer uses the local tip: an `Absent` key is unanchored until fresh (nonce-echoing) `Status` replies from `q` members of `C_{t'+2}` other than itself, counted per authenticated sender, each report a committed height `≤ t'`; it then abstains through `t' + 2` (`= max(t', H) + 2`). Proved by Lemma 0 without any assumption on the block store or on committee changes; the probe uses `C_{t'+2}` rather than "the current committee" because with a committee change at `t' + 2` a probe of `C_{t'+1}` proves nothing. Nodes with an unanchored or abstaining key (retired keys included) do not answer probes. Normative record provenance (no backup/copy/restore of records; installation log; initial record only at the installation event; reinstall → `Absent`); `Fresh` removed. The finding's "second copy of the high-water mark outside the record" was not adopted: provenance plus the probe make it unnecessary. | §1.3, §1.4, §3.5, §6.0, §6.11, §7.1 SR31/SR33, §7.4, §7.5 Lemma 0/2, §12.1, §12.2, §13.1, §13.3 F24/F27, §13.4 MS31–MS31d, §14.5 |
| 1 | medium | Initial record per (instance, key) undefined: new DS stalls or R2 stops protecting DS keys | Fixed: installation event per `(instance, key)` in a log kept outside the record files; keys generated on the node get initial records for new instances automatically; imported keys get `Absent` unless the operator asserts "never signed for `I`" | §7.4 record provenance, §13.3 F24 |
| 2 | low | R5 "extends the tip" cannot be evaluated at Init | Fixed with #31: R5 only verifies the CommitQC at Init; linkage is checked when the body arrives | §7.4 step 2, §6.9 rule 6 |
| 3 | low | Per-key restart outcomes not composed; rotation can re-enter a committed height | Fixed: `restore` runs R1 for all keys, then R5 once, then classifies every key against the new tip; the round comes from the single record at `t + 1`; MS33c | §7.4 Restart, §7.5, §13.4 |
| 4 | low | `ExecutionInvalid` evidence against honest leaders (poison transactions) | Fixed: execution verdicts never produce evidence; early timeout and `PayloadRejected` remain | §3.6, §4.2, §6.3, §7.1 SR35, §12.5 |
| 5 | high | Late entrants (awaiting apply, joiners, just synced) lose the proposal; only the gated re-push recovers it | Fixed as decided: `Status.want_proposal`; a late entry (from `awaiting`, sync, a `Status` CommitQC, or restart) without a proposal unicasts it to the leader at once and makes the node unsettled; the leader re-pushes on receipt, no interval gate, once per member per view. The finding's buffer of unverified proposals and its Status-on-every-entry were not adopted (evictable buffer, Appendix A #23; payload amplification, Appendix A #34). ML19a/b, F34 | §3.5, §5.3, §6.0, §6.2, §6.8, §6.11, §6.13, §8.2 L4/P1, §8.3, §13.3, §13.4 |
| 6 | high | `t_tx` makes timeouts depend on local mempool contents | Fixed as decided: `t_tx` removed; `anchor = min(t_enter + P(v), t_prop)`; `PayloadReady{req}` only ends the view-0 leader's heartbeat wait; ML21, F35; the deterministic parent-based alternative is recorded as §15.2 | §6.0, §6.8, §6.10, §9.1, §9.3, §12.1, §13.3, §13.4, §15.2, App. B |
| 7 | medium | Commit before own execution finishes discards the result; next height waits E + A + E | Fixed: the committed block's pending execution is kept (`tip.exec_req`); its `Valid(R = tip.result)` sets `exec_ok` and starts the next execution; `CommitBlock` lost its `executed` flag and the driver reuses a cached or in-flight execution (O3); ML23 | §6.0, §6.2, §6.3 step 0, §6.8, §12.1, §12.3 O3/O4, §13.4 |
| 8 | medium | `D_h` truncation lets an honest skip reinstate a crashed member; Byzantine share rises | Partly fixed: P4 counts one extra crashed-leader gap per honest skip in the window; O-CQ is measured only from `W` heights after the last honest skip. Rejected: ranking by skip count — in the finding's own sequence the counts tie and recency decides, so it does not stop the ping-pong; the cost is bounded and now stated | §8.2 P4, §13.2 O-CQ |
| 9 | medium | `KeyConflict` + make-before-break rotation deadlocks a 4-node chain | Fixed normatively: the application MUST NOT schedule two keys of one validator (rotation replaces the key at one height); the core's `KeyConflict` guard stays. Rejected: signing with both keys as independent members — it turns one node into two members (one crash = two faults) and doubles every signing path | §7.4 Keys, §10.3 |
| 10 | medium | `PayloadRejected` lets a Byzantine leader quarantine honest transactions; poison demotes honest leaders and ratchets start levels | Fixed: the builder quarantines only transactions that make a block `Invalid` on their own; early timeouts no longer set `failed_with_proposal`. Rejected: exempting that view's leader from `skipped_leaders` — not expressible in the signed header; the one-time demotion is stated in P4/§8.3 | §4.2, §6.6, §9.2, §8.2, §8.3, §12.2 |
| 11 | medium | Stage-ladder gaps: Commit-only withholder; `t_retx` blind to skew and inflatable; P2/P3 scale with it | Fixed: stage 1 also on an overdue CommitQC (`t_pqc + t_retx`, ML22); every round sampled, each sample capped at `2×` the estimate; P2/P3 use `t̂ = φ·T(level)/2` (the clamp) | §5.2, §6.0, §6.5, §9.1, §8.2, §8.3, §13.4 |
| 12 | medium | `exec_budget` from the leader's local level overshoots voters' timers | Fixed: `exec_budget = min(e_max, φ·T_base/2)`, level-independent; unit stated | §9.1, §8.3 |
| 13 | medium | Oracle bounds not derivable (P5, `B_view`, sync term, idle `G_norm`, P4 stage-2 cost) | Fixed: P5 restricted to all-honest, no-restart scenarios with a sum over failing levels plus `P(0)`; `T_max_eff` in `B_view`; `E_max + A_max` per synced block; idle `X` includes `build_timeout`; P4 adds `2·t̂ + Δ` when `a_h = q` | §8.2 |
| 14 | low | `T_req` ignores serialized fan-out; capped timers need a known `Δ` | Fixed (text): `Δ` covers the leader's `n − 1` copies; liveness assumes post-GST `T_req ≤ T_max_eff`; §9.4 relates `max_block_bytes` to uplink. Rejected: an uplink parameter inside `T_req` — the core cannot know the real rate; a SHOULD on `max_block_bytes` suffices | §8.1, §9.4 |
| 15 | low | Sync pinned to a Byzantine source; `sync_retry` vs byte cap | Fixed: rotate the source on every request; responses processed whenever they arrive; entries below `h` skipped. Rejected: deriving `sync_retry` from the byte cap — a slow response now costs only a duplicate download | §6.9 rules 2–3 |
| 16 | low | (1) idle chains never settled; (2) contagion rationale wrong; (3) fetch sources not cycled; (4) rate limit drops solicited replies | (1) Fixed with #27; (2) fixed (text); (3) fixed (sources cycle); (4) fixed for probe replies (`echo` exempt). Rejected for other replies: the periodic `Status` just processed carries the same state | §5.2, §6.9 rule 5, §6.11 |
| 17 | high | `P` never broadcasts the CommitQC it forms | Fixed: §6.8 step 1a broadcasts before `CommitBlock` (O2 holds it until `P`'s Commit record is durable); the unreachable Commit branch of §6.4 step 2 removed; §5.1 says a late Commit retransmit is answered by `Status`; ML20 | §5.1, §5.2, §6.4, §6.5, §6.8, §13.4 |
| 18 | high | No source for the header appended to `recent_headers` | Fixed: `BlockApplied.header`; the core checks `a == applied + 1`, `header.height == a`, `block_hash(header) == block_hash`, else `Halt(DriverAnomaly)`; §2.1 states that `J_h` is always complete | §2.1, §6.13, §12.1, §12.2, §12.3 O3, §12.5 |
| 19 | high | Several named mutation tests cannot fail (guards mask them) | Fixed: every mutation is a named function plus change (§6.0 names); MS2 killed by a restart test; MS3 is the two-guard mutation with a TC that keeps B; MS5 driven through stage entry; MS7 through a nested PQC; MS6/MS28 removed as equivalent mutants together with the redundant record field `commit`; CI meta-check; a kill map | §6.0, §6.5, §7.1, §7.4, §13.4 |
| 20 | medium | R4 "maximum of every view in the record" ambiguous | Fixed: exact list (`proposal`, `prepare`, `timeout`, `lock` views, `high_tc.view + 1`), excluding `parent_commit_qc` and nested certificates | §7.4 step 4 |
| 21 | medium | A divergent executor produces evidence and early timeouts against honest leaders | Fixed with #4 (no execution evidence); §4.4.5 corrected: its early timeout is one faulty node's timeout | §3.6, §4.4 |
| 22 | medium | `Status` of an awaiting node undefined | Fixed: height `tip + 1`, view 0, `committed_qc = tip.commit_qc`, no lock/TC/proposal hash, `want_proposal = false`; the proposal reaches it through its late-entry `Status` after it enters | §3.5, §6.2, §6.8 |
| 23 | medium | Unverifiable sync hint can never be dropped | Fixed: `SyncResponse` may be empty ("nothing at `from_height`"); unverified targets never make a node unsettled | §3.5, §6.9 rules 1, 3, 4, §6.11 |
| 24 | medium | `CommitBlock` out of height order while `pending_apply` is non-empty | Fixed: a commit is appended to `pending_apply` unless it is empty and the body is held; flush strictly from the front | §6.8 step 3, §6.9 rule 6 |
| 25 | medium | Whether own votes/timeouts run QC/TC formation | Fixed: `pool_insert` and `timeout_insert` for received and own messages alike; `route` never delivers locally | §5.2, §6.0, §6.3, §6.4, §6.5, §6.6, §6.7 |
| 26 | medium | `blocks` never pruned on a view change | Fixed: `advance_to` keeps only the bodies of `high_pqc`, `high_tc.high_pqc` and `pending_apply`; §8.4 states the body store's per-view growth | §6.12, §8.4 |
| 27 | medium | "Settled" cadence inverted (idle spam, loaded starvation) | Fixed: the "no commit in `2·rebroadcast_interval`" clause removed; late entrants are unsettled and use `want_proposal`. Rejected: re-push when a member's vote is missing at `P` — the late-entry flag and the stage ladder cover it | §5.3, §6.11 |
| 28 | medium | `t_tx` never fires under continuous load | Fixed by #6 (removed); `PayloadReady` redefined per build request | §6.10, §12.1 |
| 29 | medium | `skipped_leaders` misses failed views after `origin_view` | Fixed (text): §5.2 and P4 state the exception. Rejected: a signed `skipped_since_origin` list — new signed content for a liveness nicety | §5.2, §8.2 P4 |
| 30 | medium | Changing `W` via chain parameters desynchronises `D_h` | Fixed: `W` is a genesis constant (`Init.demotion_window`), not a chain parameter | §2.1, §9.3, §9.4, §10.1, §12.1, §12.4 |
| 31 | medium | R5's "extends the tip" cannot run in `Core::new`; failure undefined | Fixed: R5 verifies the CommitQC at Init; the flush of `pending_apply` checks parent linkage for every entry and halts with `SafetyRecordInconsistent` on mismatch; MS32c | §7.4, §6.9 rule 6, §12.5, §13.4 |
| 32 | medium | Stage-1 hint latches on in all-honest runs | Rejected (mechanism), text fixed: the latch occurs only when a set-B vote beats a set-A vote, costs `2f` messages per phase and never latency; the proposed set-A-absence hint latches the same way because `P` forms from the first `q` votes | §5.2, §5.3 |
| 33 | low | Restarted leader underspecified | Fixed: signatures deterministic (`Signer`); recorded proposals re-signed; R4 leader rules for view 0 and for `view > 0` via `high_tc` | §6.0, §6.10, §7.4 step 4, §12.1 |
| 34 | low | Payload requests lack ids; heartbeat's two requests ambiguous | Fixed: `req` on `BuildPayload`/`PayloadBuilt`/`PayloadReady`; the builder only peeks | §6.0, §6.10, §12.1, §12.2 |
| 35 | low | Two vote re-send rules; `t_vote` undefined | Fixed: stage entry re-sends once and restarts the schedule; `t_vote` defined | §5.2, §6.0, §6.11 |
| 36 | low | API enums violate `variant_size_differences` | Fixed: large variants are boxed in code (encodings unchanged) | §12.1 |
| 37 | low | TC handler enters `Q.view`, Commits there, leaves | Fixed: from a TC only §6.5 2a and 2c run | §6.5, §6.7 |
| 38 | low | Execution running at commit time is lost | Fixed with #7 | §6.3 step 0, §6.8 |
| 39 | low | Contagion rationale counts signers, claims senders | Fixed (text) | §5.2 |
| 40 | low | Evidence dedup set, `cert_cache` eviction, `RetryAt` attempts, config of `t − 1` | Fixed: `reported` set in §6.0; `cert_cache` LRU; `RetryAt{t, attempt}`; the monitor covers `tip.height − 1` only when its configuration is retained, and `tip.prev` keeps its value | §6.0, §6.1, §6.3, §7.6, §8.4 |

**Implementation delta from revision 3** (what an implementation of revision 3 must change):
1. Record: delete the `commit` field; `try_commit` checks `mine.commit`; R4 takes `view` from the
   exact list of #20; restart composition (#3); R5 split (#31); `RecordState::Fresh` removed;
   retired keys passed in `Init` and restored like others without signing.
2. R2 probe: `Init.nonce`, `Status.probe`/`echo`, the `probe` table, anchoring on `C_{t'+2}`,
   `abstain_below = t' + 3`, the probe-answer rule, the `echo` rate-limit exemption.
3. Pacemaker: delete `t_tx` and its `PayloadReady` handling; `exec_budget` from `T_base`;
   `qc_lat_ewma` samples every round with a `2×` cap; start level not raised by early timeouts
   (`sign_timeout` gets a cause).
4. Late entrants: `late_entry`, `Status.want_proposal`, the unicast `Status` at the end of a
   late-entry `handle` call, the immediate leader re-push; the "no commit in `2·ri`" unsettled
   clause removed; verified-only sync targets count as unsettled.
5. `P` broadcasts its CommitQC in §6.8 step 1a; §6.4 step 2 answers Prepares only.
6. `BlockApplied.header` with its three checks and `Halt(DriverAnomaly)`; `recent_headers` from it.
7. Execution: `tip.exec_req` and §6.3 step 0; `CommitBlock` without `executed`; driver O3 reuses
   cached or in-flight executions.
8. Stage 1 on an overdue CommitQC (`t_pqc`); stage-entry re-send once plus schedule restart.
9. `pool_insert`/`timeout_insert` for own messages; `route` never local; TC handler runs only §6.5
   2a/2c; `advance_to` prunes `blocks`; `CommitBlock` ordering through `pending_apply` with
   parent fields and the flush linkage check; §6.1 rule 5 cheap rejects apply to top-level
   objects only (a timeout carrying an older PrepareQC is still stored and counted).
10. Evidence only from signed defects (no `ExecutionInvalid`); `reported` dedup set; `cert_cache`
    LRU; `RetryAt{t, attempt}`; `tip.prev`.
11. Sync: rotate source every request, empty responses, late responses processed, entries below
    `h` skipped; fetch sources cycle.
12. API: `req` on `BuildPayload`/`PayloadBuilt`/`PayloadReady`; `Init.demotion_window` (removed from
    `ChainParams`); deterministic `Signer`; boxed variants; `status()` reports the level.
13. Simulator and tests: builder with request ids, peek and per-transaction quarantine; record
    store with installation log; F24/F27/F28 variants, F34, F35; O-SIGN lock clause; O-CQ window;
    the §13.4 table (new and renamed tests, MS6/MS28 removed) and the CI meta-check.

---

## Appendix D. Review log (revision 4.1)

The verification of revision 4 produced eight refinements. The orchestrator's decisions are
binding; nothing else changed except the consistency edits each item needs, which are listed in
its row.

| # | Refinement (decision) | Change | Where |
|---|---|---|---|
| D1 | Installation-log rollback: a key store restored from a backup older than a dataspace instance made a key generated on the node look fresh for that instance, so the driver could write an initial record where the key had already signed | "Generated on this node" is bound to the record store: every log entry carries a freshly drawn random store id, which is also written next to the record files (never backed up or restored). At start, a mismatch with the newest entry (or a missing id) makes the driver mark every key imported, so no initial record is written without an operator assertion and every missing record is `Absent`. For consistency, the installation event never replaces an existing record file. New F24 variant (key store restored from an older backup, with the instance's record deleted, and onto a new, empty record store); MS33d trusts a rolled-back log | §7.4 record provenance rules 2–4, §7.1 SR33, §12.2, §13.1, §13.3 F24, §13.4 MS33d, §14.5 |
| D2 | R2 anchoring threshold is `2f + 1` fresh replies, not `q` | R2, SR31 and Lemma 0 count `2f + 1` member keys of `C_{t'+2}`, the smallest count that meets the honest signers of every CommitQC for every `n` (`(2f + 1) + (q − f − 1) − (n − 1) = 1`); §1.2 names it as the only count other than `q`. Consequence: with `n ∈ {2, 3}` one reply anchors, and only `n = 1` can never anchor. A node with an unanchored key counts as faulty until it anchors (and a key that abstains at `h` as faulty at `h`), stated in §1 Liveness, §8.1, §8.3 and O-LIVE. F24 scripts never pair a deletion with `f` other members that ignore probes | §1.2, §1 Properties, §7.1 SR31, §7.4 R2, §7.5 Lemma 0, §8.1, §8.3, §13.2 O-LIVE, §13.3 F24, §13.4 MS31 |
| D3 | Probes vs rate limits | While any key is unanchored, every `Status` the node sends (broadcast, reply, proposal request, probe) carries `probe: Some(nonce)`. For echo replies only the probe-table update is exempt from the per-peer `Status` rate limit; the rest of that `Status` is processed under the normal limit (revision 4 exempted the whole `Status`) | §3.5, §6.11 (probe bullet, on-`Status` steps 1 and 3), §7.4 R2 |
| D4 | `want_proposal` delivery | A `Status` with `want_proposal` for the leader's `(h, view)` runs the late-entrant re-push decision even when the rest of that `Status` is rate-limited; the re-push stays capped at once per member per view. `det_l19` pins C's `Status` timer phase so that its request arrives inside the leader's rate-limit window; ML19c (re-push decision after the rate limit) added | §6.11 (re-push bullet, on-`Status` step 2), §8.2 L4, §8.3, §13.4 ML19a–c |
| D5 | Lost proposal copy at an on-time member | New helper `request_proposal` at the end of every `handle` call: a member that is not awaiting, holds no proposal for `(h, view)`, and either entered late or holds evidence that a proposal exists (a verified vote or PrepareQC of `(h, view)`, or a peer `Status` reporting `proposal_hash` for `(h, view)`) sets `want_proposal` and unicasts one `Status` to `L(h, view)` per view (new state bit `asked`). The late-entry request of §6.8 step 5 now goes through the same helper (ML19a's site moves there). Det test `det_l24_lost_proposal_copy` (n = 4, D crashed, the proposal copy to C dropped once → view 0 commits) with mutation ML24 | §5.3, §6.0, §6.8 step 5, §6.11, §6.12, §8.2 L4, §8.3, §13.4 ML19a, ML24 |
| D6 | Retired keys: never drop their records | The rule "the driver may drop it once its record is classified R3" is removed: with the `(I, K)` log entry present, a dropped record would be `Absent` at every restart and stop the node from answering probes. Size bound: one record (≤ ~3 KB at n = 31) per `(instance, key)` ever installed, i.e. per instance ≤ ~3 KB × (1 + key rotations) | §7.4 Keys, §7.1 SR33, §8.4, §10.3, §12.2, §14.5 |
| D7 | `BlockApplied` names the committed block | Besides `a == applied + 1` and the header checks, the core requires `block_hash == tip.block_hash` when `a == tip.height` and `block_hash == tip.prev.0` when `a == tip.height − 1` (any other `a` fails), else `Halt(DriverAnomaly)`. Added to the §13.5 O3 conformance checks | §6.13, §12.5, §13.5 |
| D8 | Probe echo authenticity | Echo replies are signed with the replier's consensus key over `echo_preimage(nonce, height) = TAG_SIG ‖ 0x05 ‖ I ‖ be64(nonce) ‖ be64(height)` (`KIND_ECHO`) and verified under the named member key of `C_{t'+2}`; relayed replies therefore count, for their signer, and the core no longer relies on channel authentication for safety. Consistency: the probe table is keyed by member key and pruned to `C_{t'+2}` at height entry; echo signatures carry no sign-once obligation and are excluded from O-SIGN and O-PBS; `Status` size updated. Golden vector for the preimage; mutation MS31e (echo signature not verified) with `det_s31e_echo_signature` | §1.4, §3.1, §3.3, §3.5, §6.0, §6.8, §6.11, §7.4 O-PBS and R2, §7.5 Lemma 0, §8.4, §13.2, §13.3 F24, §13.4 MS31d, MS31e, golden vectors |

**Implementation delta from revision 4** (appended to the list in Appendix C):
14. Wire: `KIND_ECHO = 0x05` and `echo_preimage` (golden vector); `Status.echo` becomes
    `Option<Echo>` with `Echo { nonce, key, sig }`.
15. State: `probe` keyed by member key of `C_{tip.height+2}` and pruned at height entry; new
    `asked` bit reset at height entry and in `advance_to`.
16. `on_status` reordered: echo handling (filters, signature check, probe-table update, anchoring
    check) and the `want_proposal` re-push decision run before the per-peer rate limit, the rest
    after it; the probe answer carries a signed `Echo` (signing key at `h`, else the first
    configured key; no answer without a configured key).
17. Every outgoing `Status` carries `probe: Some(nonce)` while a key is unanchored.
18. `request_proposal` at the end of every `handle` call and of `Core::new` (late entry or
    evidence; once per round via `asked`), replacing the late-entry-only unicast; `want_proposal`
    follows the same condition.
19. R2 anchoring counts `2f + 1` member keys (from `f`, not `quorum`).
20. `BlockApplied`: `block_hash` checked against `tip.block_hash` / `tip.prev.0`.
21. Driver: record-store id with every log entry, the start-up comparison and imported marking,
    no initial record over an existing file, retired records and log entries never dropped.
22. Simulator and tests: fake record store with store id, key-store snapshots and replacement;
    echo-aware provenance (excluded from O-SIGN/O-PBS); O-LIVE and F24 fault accounting; F24
    variants; MS31e, MS33d, ML19c, ML24 with their named tests; pinned `Status` phase in
    `det_l19`; O3 conformance for `BlockApplied` hashes.
