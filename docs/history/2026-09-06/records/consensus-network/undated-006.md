# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-ccdde6451ecd2eb52e38982c9e76d00fa6a2ea28913c8b8b610f5a7b50b6750c"></a>

<!-- Original context: Roadmap / Consensus, Performance, and Operations -->
## Consensus, Performance, and Operations

**Status:** active first-release verification and release validation.

Unless tied to one immutable candidate, Cargo, TLAPS, Verus, and network
pass/count statements below are historical mutable-tree checkpoints; none
attests the current working tree.

The release target is the single serialized Sumeragi v2 reducer and wire
revision 4. Permissioned and NPoS contexts use the same Prepare/Commit state
machine, dual count-and-power quorums, durable grouped timeout certificates,
mandatory DA, exact receipt-backed height transitions, and one frozen
height-context identity. Mixed-version operation, rolling protocol upgrades,
global RBC, collectors, adaptive pacing, missing-QC shortcuts, and runtime
mode flips are not first-release architecture and must not return.

The checked-in ledger precommits the post-Decision timeout frontier and its
dependent Core action, certified-request, effective-lock, runner-preservation,
and async-type obligations. Historical strict runs support those entries, but
fresh same-candidate proof validation is still pending and the aggregate proof
checker remains under validation. Remaining formal work proceeds through
production effective-lock refinement, progress ownership, protected service
ranks and starvation freedom, then timeout/view/leader, application,
successor, and indexed-height composition.

The Apply boundary now requires the exact durable current-context Commit
Decision in both execution and readiness; it no longer permits an old-context
Decision to retire a non-voter's historical-recovery target. The paired
deductive assertions, source-fidelity mutations, and bounded repaired/bug model
are authored. Before this slice can be promoted to machine-checked evidence,
run the pinned SANY and strict TLAPS checks for the changed modules, execute
`scripts/formal/run_sumeragi_v2_apply_authority_mutation.sh`, and archive the
green repaired trace plus the expected missing-guard counterexample.

The exact-output corridor now freezes the height-roster Safety/Lane/Bulk
reservation set, adds a distinct shared budget, and deterministically matches
at most one unique reserve to each retained fanout. This closes the discovered
non-roster identity-churn starvation path. During partial progress, an inactive
source parks only its connection-tenure writer state while retaining the exact
reservation, FIFO age, immutable payload, and current cursor. Reconnect reuses
that retained ownership to retry the current item; only an exact
`CertifiedSidecarChunk` item may reinstall its one-message local writer slot.
A generically completed source remains terminal and cannot reacquire capacity
or reset its cursor.
Reply delivery now also uses bounded per-source ownership: actor-global delivery
ordinals are separate from connection tenure, semantic duplicates attach one
attempt per authenticated source, reconnect preserves source-local message and
chunk cursors (including generic completion), and only its tenure-bound ticket
is replaced. Fair ingress,
lane-work, worker output, sidecar materialization, runner dispatch, and daemon
Hold/Release retain the exact route capability. The focused P2P, worker,
sidecar, and lane regressions are green. In particular, the sidecar now stores
terminal completion separately from pending chunk zero, so exact duplicates,
later deliveries, and reconnects cannot replay a completed source while a
sibling progresses. An incomplete parked cursor now also survives the former
server-gate TTL after shared byte/session budget is released; only terminal
no-outbound tombstones age out. Both focused TTL-boundary regressions passed
1/1, and the complete sidecar module at that checkpoint passed 50/50. The
route capability now also carries an immutable minting-tenure binding, so
bounded retired-source tombstone churn cannot admit an equal-ordinal capability
from another tenure. Centralized normal-exit/`Drop` teardown retires every
actor-owned route and cancels only that actor's waiters. Route-attempt capacity
is the effective `network.max_total_connections` value `R`; fair-ingress
authenticated non-validator lane capacity is the independent
`sumeragi.queues.authenticated_non_validator_sources` value `H`, and
configuration rejects `H > R`. Root validation resolves the effective network
lane profile before deriving `R`; in particular, an omitted
`max_total_connections` under `lane_profile = "home"` uses `R=32`. The 465-test checkpoint across 30 modules and
53 pre-network legs is followed by exact authenticated-non-validator-cap and
alternate-route-before-lane-cap regressions, yielding the 467-test
checkpoint without adding a module or leg. Three daemon Hold/Release controller
regressions, one layered daemon ownership regression, and two root configuration
geometry regressions yield the 473-test checkpoint, still across 30
modules and 53 pre-network legs. One configuration-fingerprint, two historical-
recovery-kernel, and one shared authenticated source-credit regression yield
the historical 477-test checkpoint with the same module and leg counts.
Mechanical reconciliation adds 16 non-ignored Kura, successor/refinement,
CommitQC-admission, recovery, runner, watchdog, P2P-geometry, and daemon-genesis
tests; the lane-relay saturation test is a rename in place, not a fifth module
test. A subsequent adversarial sidecar regression proves a same-tenure route
redelivery cannot re-emit an in-flight chunk before its writer flush. Three worker
regressions additionally retain pending and flushed-but-unapplied ownership,
preserve a terminal zero-reservation route beside live siblings, and retry an
unflushed current item on reconnect without resetting its cursor or charging a
second same-source reservation.
The subsequent source-authority, immutable-sidecar, runner-race,
daemon-corridor, shared-byte-budget, cached-Arc admission, and executable-refinement closure adds 22
exact regressions and moves two peer tests to their actual owning module.
The former terminal-reconnect test is renamed to require this current-item
retry after a writer closes without `Flushed`. It retains
the exact runner route bridges and missing-route rejections for
durable lane certificates and certified sidecar chunks, plus the four-gate,
two-session, 16-MiB per-source sidecar cap and same-hub overflow isolation
tests. The added authenticated-source limit and pre-cap alternate-route
regressions are not covered by that historical 50/50 receipt. The current
closure journals requester and responder lifecycle state under Kura, preserves
authenticated-source budgets and cursors across restart/height rollover, and
fail-stops request, queued-chunk, timeout, Close, and CloseAck output when that
journal cannot advance. Outstanding release work for this slice is to execute
those complete source-sealed suites, add a
four-validator signed-observer slow-reader flood during view change/body
recovery, discharge the remaining formal obligations, and run the sealed
network, chaos, and 24-hour soak gates.
Decision cleanup additionally retains only the exact decided carrier's
immutable lane-body binding, so a late atomic certificate can finish without
reviving any losing carrier. The touched Core modules are green in isolation;
the full Core suite still requires a completed source-sealed run because the
latest broader attempt stalled in an unrelated Kura regression.

First-release voting validators are explicitly scoped to the Linux/macOS
storage contract; non-Unix nodes are restricted to uncertified observer or
development use and fail voting startup.
Windows validator support remains blocked on a unified handle-relative
filesystem layer covering progress-temp replacement, no-clobber lane-geometry
moves, recursive authenticated archive deletion, 128-bit ReFS identities, and
durable namespace barriers, together with NTFS/ReFS crash and substitution
tests. Compile success alone must not remove the validator startup gate.

The append planner no longer repeats a full historical-index allocation after
descriptor-bound recovery, but the recovery pass itself still validates the
complete index before an ordinary write. Keep that full scan for startup and
ambiguous crash repair, then add an authenticated tail checkpoint or equivalent
bounded steady-state attestation before treating the 100,000-height result as a
storage-throughput gate. The optimized path must retain exact rollback,
non-overlap, range, namespace, and durability checks.

The production boundary now preserves a durable locked Commit intent across
TC-driven volatile resets through generation-scoped exact-vote delivery,
executable reducer/WAL progress witnesses, retained locked-body recovery, and
protected service across per-source count/byte ingress quotas and bounded peer
send pools. Timeout/safety capacity remains isolated; cancellation-safe flush,
unified reliable read/write arbitration, and finite direct-post bursts protect
the local actor-to-writer attempt. A local flush is not a remote application or
relay-final-target acknowledgement, so those later custody boundaries remain
explicit proof debt.
The canonical status payload exposes exact partial dual quorums, outbound
intent/work ownership, queue age/debt, transition age, ignore counts, and a
view-aware blocker.

The serialized production runner now actively polls that diagnostic contract
on every turn, including retained-output and completion retry paths. The
watchdog rebuilds fully overlaid snapshots only at the retained semantic
deadline, before the first deadline is known, or when height ownership changes;
it emits one structured warning for a new or changed blocker and one recovery
notice only after the height-wide semantic high-water advances. View/TC churn
cannot clear an alert, and successor activation or status clear resets it
without claiming recovery. Exact quorum, queue, and local-work context is
included in each edge-triggered record.

Reliable transport ownership now covers the bounded local seams in both
directions. For outer ingress, the exact non-empty-roster count geometry is
`5N+3H`: each validator transport hop owns generic, ordinary Progress,
certified-fence-escape, TimeoutVote, and TransportCompletion positions; each of
the at most `H` simultaneously materialized authenticated non-validator lanes
owns generic, certified-fence-escape, and TransportCompletion positions.
Identityless ingress owns no capacity partition; the no-roster diagnostic
minimum is `3H`. Semantic origin remains
available to the protocol, but authenticated `via` owns count, bytes, fairness,
and reservations, so relay identity churn cannot multiply capacity or borrow a
validator reserve. Semantic duplicate/alternate-route attachment precedes the
new-lane `H` gate, and empty authenticated non-validator lanes are removed.
For outbound reliable consensus, the original actor item retains its byte
lease and per-target attempt until every selected peer writer reports a
complete local write and flush. Cancellation resumes flush without rewriting
bytes, replacement/reconnection retries unfinished direct targets, and one
non-cloneable authenticated-source credit follows inbound bytes through the
actor-owned queues. Old-generation dispatch workers now drain accepted
reliable work before normal teardown, and a closed subscriber returns its
actor-side pending reliable backlog for FIFO replay to its replacement.
Only a canonically identical retry in the same membership tenure coalesces with
an existing broadcast child. Distinct payloads and direct/broadcast cross-kind
collisions retain exact per-target FIFO tickets, while responsive targets
continue independently without a class-wide parent. Explicit topology removal
cancels only the old broadcast tenure, remove/re-add creates a new generation,
and direct-post ownership survives. Already-channelled subscriber items,
direct-post ownership for a removed target, relay second-hop failure, and final
application receipt still need a durable final-consumer acknowledgement or a
proved reconstruction handoff.

The exact Sumeragi output scheduler is now per-target FIFO and round-robin
across fan-outs, so an actor-backpressured target does not suppress completion
service or later responsive targets. After local application, an exact Kura
receipt and matching finality artifact authorize an all-or-nothing retirement
only after every retained network-message hash and typed creation-scope claim
revalidates. Historical CommitQC, certified-body, and lane-certificate claims
also independently reread the exact Kura finality artifact, canonical body, or
certified lane artifact and reject source, responder, target, or payload
substitution. Current-height global V2 payloads bind to the exact applied
artifact. Winning lane payloads require the exact durable Kura certificate and
application receipt; alternate proof variants are revalidated, while
structurally valid same-height non-winning outputs are explicitly superseded.
Native AMX claims bind scope, embedded round, and message hash; merge-share
claims bind scope and share hash. Certified sidecar request/chunk claims bind
scope, requester/responder target roles, transfer identity, and exact payload
hash. Finalized-sidecar pruning leaves winning data in the committed merge log
and explicitly supersedes losing pending work before handoff. Manual or
otherwise untyped `Exact` output remains owned and fails closed.
The source-bound seam and synthesized-finality runner regression do not replace
the real QC-to-body-to-store-to-validate-to-apply and catch-up corridor. When a
broadcast target already has a child owner, only an identical canonical retry
in the same membership tenure coalesces with it. A distinct same-class payload
or direct/broadcast cross-kind collision keeps exact per-target FIFO ownership
with the caller, while other targets proceed without a class-wide parent.
Removing the target cancels only the old broadcast tenure, remove/re-add creates
a new generation, and direct-post ownership survives. True target-geometry
exhaustion, direct-post removal, relay/final-application acknowledgement, and
broadcast starvation freedom remain open.

Production transport geometry is implemented and is no longer an open design
item. Frozen-context activation computes exact bare manifest, maximal proposal,
payload-completion, recovery-request, and Commit-certificate-response bounds;
then lifts them through the complete `NetworkMessage`, direct P2P relay,
header-framed data message, AEAD, and outbound high-queue layers. The
feature-independent 8,258-byte raw public-key ceiling covers non-roster
observers and rotated responders. Configure and open both fail closed when any
count, byte partition, topic frame, global encrypted frame, or queue owner is
undersized. Shipping defaults are 17 MiB for global/consensus/block-sync,
2 MiB for control, `H=2`, 161 ingress entries, a 34 MiB source partition, and
1,122 MiB maximum-roster aggregate body ownership. Kagami localnet scales aggregate body bytes
by `N + H` and rejects validator rosters above the protocol maximum of 31. The
independent P2P wire-prefix boundary is also closed: the
wire body has the inclusive `u32::MAX` ceiling, while runtime configuration is
limited to a deterministic 2,147,483,643-byte body so prefix plus body remains
representable in one buffer on 32-bit and 64-bit hosts. Daemon and network
startup reject larger caps before binding, and the sender uses checked length
derivation before encryption. Prefix-inclusive queue charging and peer queue
counters reject arithmetic overflow explicitly, outbound serialization is
counted before frame materialization, and stream readers grow incrementally
after checked prefix arithmetic. Remaining consensus work is evidence and
mechanization: this
closure does not make the strict TLAPS runner, clean release corridor, or
100,000-height chaos receipt complete, and
the checked-in `machine_checked_completion: true` value remains a precommit,
not a current aggregate proof result or release receipt.

Every Commit vote now requires the exact active durable lock, including votes
from the current view. A pre-lock current Commit is a recoverable ignore. Once
the matching `LockAndCommit` acknowledgement applies the lock, the adapter
advances the locked-Commit consumer epoch so that exact vote may enter once; the
reducer then prunes the superseded historical Commit pool before releasing the
current Commit signature. The old pool remains intact if persistence fails.
Outbound retransmission is a peer-delivery source, not a sufficient local
progress witness because broadcast excludes the sender.

The deferred Progress reserve is now partitioned into non-displacing
locked-Commit, TimeoutVote, PrepareQC, CommitQC, and TC ownership classes, with
one signer-injective locked-Commit slot and one TimeoutVote slot per frozen
validator. The TimeoutVote slot is shared across the bounded current and
adjacent-future rounds. Exact retransmissions coalesce, while distinct
same-owner traffic cannot displace admitted progress and retries after fair
service. Current/adjacent TimeoutVote keys retain two signer-bounded
semantic-admission sets beyond the ordinary history budget, preventing
pre-queue starvation when that table is saturated. The pinned adapter boundary
realizes the exact `1024 + 3N` live bound and crosses TC installation to prove
out-of-window retirement, adjacent-share preservation, and non-poisoning
current-view retry. Durable Decision
retransmission reconstructs store, validation, and application work from its
recorded body stage. The height watchdog advances only on bounded semantic or
Prepare/Commit count-and-power high-water, while candidate status reflects
actual ownership, validation and Apply sidecar waits remain distinct, and the
bounded worker completion handoff reports depth, capacity, age, and service
debt. Successor construction is visible as runner-owned `Running` work and
defers the successor reducer status and emits `SuccessorHeightActivated` only
after adapter/runtime/service startup succeeds, live clocks are armed, and
authenticated ingress is open. The marker is bound to the successor height and
generation and retained by the live adapter. Complete-tip and audited-snapshot
recovery reconstruct the same deferred boundary. Height/context-tagged effect,
completion, and ingress overlays cannot cross-contaminate the predecessor;
constructor failure retains its `Running` snapshot with `restart_required` and
exits fail-closed.

Decision is now an explicit terminal cleanup boundary: losing proposal, body,
candidate, outbound, lane, and retransmission ownership is retired before the
decided-body recovery effect reserves capacity, while the exact decided
pipeline remains protected through application. Nonzero-view proposal replay
also joins the safety-WAL intent with the independently fsynced body-store
validation commitment before startup signing, and a production-path regression
continues from that restart through broadcast and the exact Prepare vote.

Certified lane-block and application-receipt progress sidecars always enforce
data, index, immediate-directory, and bottom-up ancestor barriers through the
authenticated Kura root, independent of ordinary batched `fsync`. Authoritative
reads re-attest the full binding and fail closed because page-cache readability
does not prove durability. Pending alternate QCs and proofs of possession are
fully authenticated and validated before same-proposal retirement.

Body-pipeline completions now have one evidence-preserving ownership domain
across runtime ingress and the adapter's Busy-deferred queues. Exact retries
compare the complete manifest and durable/validated receipts, including
validation polarity; conflicting evidence and multiple owners fail closed
before any proposal-pruning side effect. A production Busy-transfer regression
pins the cleanup seam which the four-validator genesis reproduction exposed.
`LocalProposalReady` now uses the Completion reservation in both domains rather
than competing with Normal ingress. Individual signed Votes cannot establish
execution-commitment authority; they require an exact binding from local
validation, verified WAL replay, or quorum-authenticated QC evidence and remain
recoverable while unbound. Commitment conflicts are rejected before serialized
runtime ownership.

Authenticated consensus messages share one generic production ownership
bridge across runtime ingress and Busy-deferred state. It compares the complete
canonical envelope with the retained `authenticated_wire_identity` and repeats
that equality after authentication. CommitQC-specific lookup wrappers remain
test-only regression conveniences; the remaining refinement debt must be
discharged against the generic full-envelope path rather than a
reducer-event-only QC exception.

Body-availability rebind now requires the installed destination tag and
transactionally preflights source plus destination ownership. One exact source
moves or coalesces into one exact destination; an uninstalled destination is a
recoverable non-mutating caller rejection, while conflicts or duplicate owners
fail closed before mutation. Pipeline and Decision retirements also preflight
before removing any owner. Certified lock/view/Decision cleanup preflights both
signed-request indexes and commits their retirement together only after service
acknowledgement; second-callback failure, mixed decided-local work, corrupt
indexes, and post-commit status failure are pinned by adversarial tests.
Certified-body request conflicts remain nonfatal, and conflicting
Commit-certificate responses preserve outstanding discovery for retry.
Ordinary pending-work exhaustion is the general retryable executor boundary;
certified-request pressure is a separate Fetch-only retry whose atomic
preflight allocates neither work nor request ownership on failure. An exact
transport-only `CertifiedBodyResponse` with a still-live matching logical
request registration may cross retained reducer-effect debt and release that
request capacity, while `CommitCertificateResponse` remains
reducer-ordered. The largest flattened persistence macro-step has four effects
against the eight-effect adapter/reducer bound. Runtime scheduling services one
deferred adapter step per turn before timers and newer ingress once older
WAL/signature work is serviceable; a subsequent unexpected `Busy` is terminal,
and terminal readiness checks all three deferred queues. A production-capacity
regression saturates certified-request, Normal, and Progress ownership while
preserving the Completion reserve, then uses durable reducer retransmission to
reconstruct the blocked Fetch after the exact authenticated response with a
still-live matching logical request registration releases capacity. At that
historical checkpoint, the gate pinned 477 required tests across 30 modules,
including exact composite
replay-FIFO ordering, its source-linked refinement projection, and
recovery-derived successor identity plus sequential missing-height discovery
and catch-up. A four-validator restart diagnostic showed that the recovering
validator paid the complete 20-second quiet-round deadline at each missing
height before starting CommitQC discovery. Recovery now seeds immediate
discovery from durable v2 startup or an interrupted applied tip, carries that
urgency only when an authenticated Commit-certificate response yields a
discovered CommitQC which is admitted to, or coalesced with, serialized reducer
ownership, and clears it after ordinary live finality. The outstanding
request's `Some`-to-`None` transition proves only that ownership handoff, not
reducer execution, Decision, durability, or historical-Kura provenance. This
retains the existing request authentication, exact
frozen-context and certificate checks, reducer ordering, and ordinary-height
deadline; it does not add permanent normal-height fanout. The existing
four-validator restart regression owns this sequential catch-up seam. Its
fresh exact run,
`sumeragi_v2_runner::authoritative_v2_finalizes_through_validator_restart`,
passed 1/1 in 79.82 seconds; all four peers shut down gracefully with empty
stderr. This focused result does not promote a formal obligation or complete
the broader release matrix. Post-decision regressions reject new durable
timeout and TC formation after a local decision. Capacity regressions
bind the real serialized-runtime Proposal-A/distinct-PrepareQC-B/TimeoutVote
trace, bounded causal effect dispatch, deterministic reconstructible-Fetch
preemption, full-capacity Fetch reconstruction, Decision filtering, and the
canonical `EffectDispatch`
watchdog lane. The timeout watchdog regression distinguishes a remote partial timeout
pool, a durable local current-view timeout path, and exact same-/older-view
locked Commit recovery. Before the three-corridor expansion, all
twelve then-owning modules were green on the then-current unsealed source:
56/56 reducer/core, 10/10 refinement, 9/9 reducer source-link, 57/57 adapter,
26/26 apply, 115/115 effects, 60/60 lane work, 40/40 runtime, 29/29 recovery,
23/23 runner, 66/66 worker, and 19/19 watchdog tests (510 passed, 0 failed, 0
ignored). Cargo discovery found every one of the
146 previously required names among 6,742 tests, with no missing or ignored
entry. The latest expanded unsealed discovery found all 162 then-required names
among 6,744 tests with none missing or ignored, and the authoritative-ingress
module is green at 30/30. Two newly pinned replay-FIFO names plus their two
source-linked refinement witnesses raised the inventory to 166, and the two
effective-lock acquisition regressions raised it to 168. Two same-runtime-step
Decision reconciliation regressions now raise it to 170. The new
refinement witnesses are green at 2/2, the complete source-link module at
11/11, and the isolated reducer library at 96/96. The preceding mutable-source
discovery found all 168 then-required names among 6,750 library tests with none
missing or ignored, and direct exact execution was green at 168/168. Fresh
mutable-tree discovery found all 170 then-required names exactly once among
6,756 library tests, with no overlap with the four ignored tests. The new 171st
classifier is green exactly 1/1, in the watchdog module at 21/21, and in the
complete status module at 29/29. Eleven Kura progress-witness durability
regressions, two lane-geometry durability regressions, and one lane-work
alternate-certificate retirement regression raised the inventory to 185 names
across 16 modules. Six exact lane-retirement recovery/substitution regressions
and the lane-work plus runner platform-role gates raised the inventory to 193.
The isolated-runner view-zero deadline/view-one observation contract raised the
inventory to 194 across 17 modules. Seven bounded effect-dispatch/executor
regressions and the `EffectDispatch` watchdog regression raised it to 202. Two
exact post-decision timeout/TC quiescence regressions raised it to 204. Fourteen
scheduler/adapter/effect regressions raised it to 218, and four final
post-WAL, terminal-readiness, real-adapter-ordering, and production-capacity
adversarial regressions raised the preceding inventory to 222. Six outer
TransportCompletion-corridor regressions raise it to 228; explicitly pinning
the five-per-validator plus two shared relay-lane owners (`5N+2` total)
capacity-negative raises it to
229; and the four-validator exact PrepareQC count-and-power quorum regression
raised the preceding inventory to 230. Atomic lane-certificate, authenticated-
via, historical-recovery, active-watchdog, peer flush/source-credit, network
actor-retry, and daemon relay-fairness regressions raised the preceding
inventory to 264. Thirty-seven positive additions—10 exact-output/typed-rollover,
2 peer-generation/flush, 20 ticket/topology/broadcast/subscriber-network
regressions, 1 runtime/Busy-deferred exact CommitQC coalescing regression, and
4 Nexus lane-relay ownership/fairness regressions—are offset by removal of the
obsolete adapter cursor alias and two superseded broadcast-residual tests,
raising that inventory by a net 34 to 298; the current route/reservation slice
adds another net 13 tests, and nine P2P reply-route tests replace one phantom
historical name for a further net 8 and a total of 319. The rollover slice covers historical
Kura CommitQC, body, and lane-certificate rereads; current global V2; lane
proof/supersession, Native AMX, merge-share, certified-sidecar, and untyped
fail-closed boundaries. The network slice pins identical-retry coalescing and
exact per-target FIFO ownership for distinct/cross-kind collisions.
Certified-sidecar completion now also retains a byte-free identity of the
current source chunk and consumes one actor-minted, clone-shared writer claim
only after exact projection and payload validation. Equal ticket/delivery
fields reconstructed with another claim cannot cross worker handoff, and a
consumed late receipt cannot advance a later byte-identical rematerialization.
The four
integration names execute as one module-filtered leg; adding
the peer, network, daemon, and Nexus lane-relay modules expands the totals to
21 modules and 41
pre-network legs. Fresh full discovery and serial execution are pending for
that inventory; the preceding 170-name
corridor was green with one passing row per name. The committed, detached,
source-sealed serial rerun remains pending. A fresh exact isolated-network run after the status-registry
isolation change also passed the authoritative four-validator genesis test 1/1
in 266.01 seconds with networking required, one startup attempt, and the exact
deterministic scenario seed; all voters applied genesis and exposed the common
awaiting-proposal successor height. It remains mutable-tree diagnostic evidence
and does not replace the four-seed PR or 32-seed sealed release matrices. The
one-shot token binds successor identity to the exact
verified successor `HeightContextId`, and the adversarial rejection table
retains the applied predecessor on a same-height foreign-context snapshot. A
directly pinned adversarial umbrella test exhaustively covers the four
source-linked body/scheduler kernels, and a pinned runtime regression exercises
every selector ready-mask through production `pop_next`. The clean committed,
detached, source-sealed serial release leg remains pending; the unsealed module
runs are not release evidence. The earlier exact one-attempt four-validator
genesis rerun passed on all validators in 456.76 seconds, with retained logs at
`target/sumeragi-v2-genesis-final-hardening-20260716/irohad_test_network_7sJJmM`.

A newer current-working-tree diagnostic ran the exact authoritative
four-validator genesis regression with networking required, network-start
retries disabled,
and its exact deterministic seed. Its single permitted startup attempt passed
1/1. Live snapshots crossed the original reset boundary through view 10 while
retaining the exact view-9 locked Commit intent; the regression then observed
genesis applied on every validator and a common awaiting-proposal successor
height. All four peers exited with status 0, and libtest completed in 1192.57
seconds including cold network binary preparation. Its temporary log
is `/tmp/iroha-root-genesis.1T17yY/run.log`, and its temporary localnet is
`/tmp/iroha-root-genesis.1T17yY/irohad_test_network_FbEl6A`. This mutable-source
diagnostic is not a clean signed, checkout-manifest-bound source-attested
release receipt and therefore does not reduce the outstanding four-seed PR,
32-seed release, strict-proof, clean chaos-attestation, or 24-hour soak gates.

After the locked-round, exact-output, historical Kura handoff, and P2P
request-identity hardening settled, a final current-tree one-attempt rerun of
`authoritative_v2_genesis_commits_on_every_validator` also passed 1/1. It used
four voting validators with DA/RBC enabled, network skips disabled, and the
exact deterministic scenario seed; all four validators applied genesis,
activated the common awaiting-proposal successor height, and exited cleanly.
Libtest completed in 656.79 seconds including daemon binary preparation, and
retained the localnet under
`/tmp/iroha-sumeragi-v2-genesis-final-20260719b/irohad_test_network_04E5W5`.
This supersedes the earlier mutable-tree diagnostic for the current
implementation checkout, but it remains non-release evidence and does not
reduce the outstanding multi-seed, proof, chaos-attestation, or soak gates.

The exact current-tree restart and timeout-rotation scenarios are separately
green: `authoritative_v2_finalizes_through_validator_restart` passed 1/1 in
165.05 seconds after replaying heights 1 through 4, and
`taira_npos_leader_timeout_commits_within_rotation_bound` passed 1/1 in 90.00
seconds. The historical distinct-subject PrepareQC attempt remains red. It
split the four validators 2/2 across QC views 1 and 0, but both QCs named the
same block subject because the view-zero QC reached every validator and the
safe-value rule correctly forced an exact-body re-proposal. The retained
network is
`/tmp/iroha-divergent-prepareqc-retained-20260718/irohad_test_network_M4CsaG`;
that result remains diagnostic only. A replacement schedule is implemented: it
isolates A at the actual view-zero leader, advances the remaining three using
no-high-QC timeout votes, forms B at two receivers, installs stale A at a
fourth, and then drains one FIFO fence. Static checks pass, but compilation and
the exact real-network run remain required before this scenario can reduce
matrix debt.

Ten arbitrary-context Core safety obligations are TLAPS-proved: durable-vote
uniqueness, lock monotonicity, external validity, certified-body availability,
certificate uniqueness, agreement, conflicting-CommitQC exclusion, and crash
recovery, together with historical TC-lock Commit authorization and the
dependent full timeout-protection wrapper. The receipt-backed chain-prefix and
epoch-boundary obligations and the narrower grouped-timeout kernel for Commit
intents already present at timeout are also proved.

The focused P2P, fair-ingress, locked-Commit, WAL, leader-crash, Decision
cleanup, and nonzero-view restart corridors are green. On 2026-07-15, a
freshly built, digest-pinned `iroha3d` also passed
one exact no-retry four-validator genesis attempt on every validator in 59.09
seconds. The checked formal baseline and SANY analysis are recorded in
`status.md`. The complete
timeout-envelope DOMAIN/adapter boundary is TLAPS-proved, and the
deferred-owner replacement mutation now pins scheduler-wide exact-envelope
coalescing. The proof ledger reports `machine_checked_completion: true`, but
fresh strict proof and revision-4 TLC evidence remain pending; post-GST height
liveness remains a conditional protocol claim until that release corridor
passes. The PR gate inventories 477 production-liveness tests across 30
Rust modules before network startup. Exact regressions cover
completion coalescing, conflicting evidence, production Busy transfer,
transactional cross-queue retirement/duplicate rejection,
installed/destination rebind, certified cleanup plan/commit boundaries, and
post-decision timeout/TC quiescence.
It also inventories the Rust
cross-SDK fixture authority and exact JavaScript/Python status-parser tests.
The exact-evidence four-validator reproduction passed its first attempt in
351.76 seconds, with retained logs under
`target/sumeragi-v2-genesis-recheck-exact-evidence-20260715/irohad_test_network_3XUobQ`.
The earlier eleven-module wrapper and final exact genesis rerun are green, but
the complete PR corridor still needs a recorded green run. A standalone
focused 100,000-height permissioned/NPoS chaos run also preserved both
50,000-height chain prefixes in 52.97 seconds. The four-seed PR and 32-seed
release real-network matrices remain unexecuted;
the release profile must also reproduce the chaos result under its
checkout-manifest-bound evidence root. The current schema-v2 gate additionally
queues local work, rotates restart across five persistence/application
boundaries every 64th height, rejects stale-generation completions, and binds
exact duplicate, reordered, and count-only/power-only negative counters. Its
post-hardening 320-height smoke and exact 100,000-height harness run are green;
the prior run completed in 57.52 seconds with every schema-v2 counter matched,
and a fresh mutable-source rerun completed all 100,000 heights in 171.97
seconds under concurrent build/proof load with every exact counter matched.
The checkout-manifest-bound rerun remains outstanding until these changes are
available as a signed clean commit.

Production release execution is now structurally isolated from mutable source
and authenticated before candidate code runs. Active Git operations are
rejected, the candidate must be one clean committed HEAD/index/worktree,
including one regular tracked `Cargo.lock`, and an operator-authenticated out-of-tree
bootstrap must run under a protected `python3 -I -S`. The bootstrap digest-binds
and archives its protected Python, Git, OpenSSH `ssh-keygen`, Bash, manifest and
identity helpers, one-entry unbounded SSH allowed-signers policy, and revocation
policy; it authenticates the exact signed candidate and bound runner before
launch. Direct production-runner entry fails before another candidate helper.
The bootstrap neither times out nor output-captures the runner and never
signals its process group; an escaped internal deadline stays visibly
incomplete.
Its outer `PATH` consists only of archived protected tools plus a digest-pinned
runner-tool closure. Before any candidate build child starts, the outer runner
copies the named reviewed language/tool and exact shell-utility runtime closure
to new private inodes and binds their source and destination inventories; the
child receives only those private paths. There is no `/usr/bin:/bin` execution
fallback, and an unlisted external command fails closed. Tool sources and all
ancestors must be trusted-owner and non-writable.
Runner stdout/stderr inherit owner-private regular files, so a
blocked bootstrap stream cannot stall the runner; normal exit seals them
read-only, while bootstrap-only interruption preserves active logs without an
external completion marker. The successful runner retains its sealed source
and identity for independent replay.
The complete corridor then re-enters from a unique independent clone made with
`--no-local`, `--no-hardlinks`, and no alternates, sealed read-only,
with external outputs. The chmod seal is a cooperative guard against ordinary
writes, while detached committed source, the external bootstrap marker, and
repeated identity checkpoints remain authoritative. The manifest binds modes
of enumerated file/symlink entries, while the seal walk checks directories and
rejects escaping or writable-output symlinks plus hard-linked source files.

The original checkout manifest and sealed manifest are both retained; every
child completion uses the latter. One canonical aggregate receipt binds
original HEAD/tree/`Cargo.lock`, all 83 pre-network legs and their exact
866-test inventory plus the separate exact 522-test G-UNIT inventory, the
formal harness lock/toolchain, matrix, chaos, and soak
evidence. The formal leg archives a tee-captured all-legs log plus
`proof_coverage.json` and `proof_evidence.json`; receipt publication reruns the
official proof checker. Every matrix summary row hashes its exact Cargo log,
and receipt publication revalidates all 160 scenario and exact-seed libtest
markers. Each retained localnet has a canonical relative-path/size/SHA-256
manifest, and the schema-v2 matrix completion binds all 160 manifest paths and
digests plus their aggregate index; receipt replay recomputes every manifest
and rejects legacy, symlinked, special-file, escaping, or changed evidence. The
100,000-height launcher requires an explicit 50,000-per-mode
completion marker and publishes its source-bound log receipt, and the soak
promotes an invocation-local `.partial` JSON only after validation and final
identity checks, with exact HEAD/tree/`Cargo.lock` plus the canonical JSON and
full Cargo/libtest log hashes in its completion. Cargo/rustc resolve to the
repository's pinned 1.93.1 toolchain after Git/Rust semantic-environment cleanup
and run with an isolated configuration-free `CARGO_HOME`; exact tool paths,
versions, and hashes are receipt-bound.

The current release-support suites collect 367 aggregate release-receipt and
258 protected-bootstrap tests without selector inflation. The source-bound
release gate now requires exactly 5,513 proof-fidelity cases and 27
formal-launcher cases. The source-derived proof-fidelity composition is 5,410 ledger/checker
cases, 29 pinned-Verus evidence cases, 15 TLC-normalizer cases, eight
reviewed-Rust closure cases, 31 Native/passive multilane source-contract cases,
and twenty selected layout/wire cases. It includes
the new fail-closed mutations for exact-request coalescing, exact-output
lifecycle source seals, and durable Apply-tombstone monotonicity. Fresh
settled-tree collection and execution of both
preflights remain pending; the derived totals and prior runs are not promoted
as final release evidence.

The receipt is published with mode `0400`, one link, an exclusive fully synced
stage, and one no-replace rename. Publication revalidates and synchronizes every bound
artifact, then every evidence directory bottom-up, and repeats the transitive
closure with the receipt included; any file, directory, or `fsync` failure is
fail-closed. The protected outer runner invokes the separately archived receipt
validator in exact `--verify-existing` mode; that validator alone publishes the
no-clobber acknowledgment. Only then may the protected runtime helper prune
disposable runtime/cache/target state and publish the retained result and exact
inventory. The bootstrap authenticates that acknowledgment, result, inventory,
source identity, and sealed runner logs before external completion. There is no
mutable pointer.
Malformed, incomplete, cross-source, semantically mismatched, and
digest-mismatched evidence is rejected. This authenticates the
signed candidate and runner relative to operator-protected inputs, but does not
attest the host image, pre-Python dynamic loader, same-UID processes, trusted
ancestor owners, or storage which violates `fsync`. The remaining work is to
produce the fresh proof evidence below and then execute the hardened gates,
not to accept evidence from the mutable caller.

In a historical pre-precommit proof wave, runner scheduler preservation and
the dependent async type invariant were recorded as `tlaps_proved`.
Hash-guarded strict TLAPS slices exited 0 for transport/runner closure (186/186
and 204/204), the recovery execution hierarchy (305/305), its strong caller
and bracket (63/63), the exact type obligation (16/16), and the named
always-strong wrapper (10/10).

The 54-entry top-level ledger contains 44 `tlaps_proved`, 3
`cross_tool_proved`, 6 `trusted_contract`, and 1 `out_of_scope` entries, with
no `specified_unproved` rows and machine-checked completion true. Sixteen
source-bound decomposition
leaves are checked transitively through those reviewed consumers rather than
being additional release claims. Historical recovery now accounts for exactly
three promoted temporal targets—authority acquisition, certificate-rank
progress, and Decision/body rank progress—with source proof bodies, plus one
proved Decision-stage ownership safety leaf. Composition derives that
ownership property from `IndexedChainSpec`; it is not a fourth temporal
premise. The three temporal
leaves and top-level `height-liveness` require fresh strict release evidence;
their promoted statuses do not substitute for that evidence. In particular,
`AdequateLeaderExactClosureResidualObligation` and
`ExactDecisionOffSchedulerResidualConvergenceObligation` have explicit source
proof bodies; neither may support release without fresh strict evidence or
through assumption, circular reuse, or a vacuous wrapper.
Outstanding release work:

The async deadlock decomposition now scopes local runner-service debt to the
exact active responsive and historical-recovery owners. Keep that deadline as
per-validator ghost bookkeeping for the existing `runtime-after-gst` trusted
contract. The source-bound serialized loop, bounded service turns, watchdog
poll, and finite idle waits are necessary structural evidence, but they must
not be promoted into a proof of host scheduling or operation latency. The
sealed release/soak evidence must still demonstrate that operator-provided
runtime premise on the final signed source.


