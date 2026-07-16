# Sumeragi v2 liveness contract and release gate

Sumeragi v2 does not promise unconditional termination. An unbounded network
partition, the absence of a responsive dual quorum, or local disk, signing,
validation, and application work which never completes can prevent progress.
The first-release target is therefore conditional:

> After GST, with a responsive dual quorum and terminating local work, every
> height eventually decides and every responsive validator eventually applies
> the decision and activates its successor height.

This remains the conditional protocol target and paper argument until the
proof ledger reports `machine_checked_completion: true`.

The formal liveness contract makes both quantifier boundaries explicit. An
undecided execution must either decide before another rotation is needed or
reach a view in which the responsive honest scheduled leader itself is active;
that leader state must then lead to decisions at all responsive validators.
Application is per validator: once GST holds and one responsive validator has
a durable decision, that validator must eventually recover, validate, and
apply the certified body even if another responsive validator has not yet
decided. The aggregate all-decided-to-all-applied clause is retained only as a
composition target for successor-height activation.

## Progress ownership

The reducer and its adapter enforce one structural rule at persistence and
view-change boundaries: volatile progress state may be cleared only while a
durable source retains a fairly scheduled reconstruction path. In particular,
a timeout certificate may clear a volatile vote pool, but it does not clear an
active PrepareQC lock or an existing corresponding durable Commit intent. A TC
may also promote its selected highest PrepareQC to the active durable lock at a
node which has no Commit intent for that round. That node does not sign merely
because the TC was installed: it recovers, durably stores, and deterministically
validates the exact locked body, then appends the matching historical
`LockAndCommit`. Only the successful WAL acknowledgement releases the Commit
signature.

This is a narrow exception to the timeout fence. It authorizes only the exact
round and subject of the active TC-promoted lock; arbitrary old-round Commit
votes and all old-round Prepare votes remain fenced. A higher local Prepare
intent or known PrepareQC blocks the exception only when it names a different
subject; a higher same-subject reproposal is non-conflicting and does not
suppress reconstruction of the old-round Commit. Replay admits the
historical `LockAndCommit` only after an earlier durable `InstallTimeout` has
advanced the view while leaving that exact PrepareQC as the active lock. A
missing or mismatched lock fails closed instead of reviving an unrelated old
round. Each authenticated Commit vote which exactly matches the active lock may
cross semantic delivery admission once in each active consumer epoch. A TC
generation change or a newly acknowledged exact lock advances that consumer;
equivocation fingerprints are tracked independently from delivery records and
retained for one roster rotation (plus the active exact lock exception).

The exact-lock rule also applies to current-view Commit votes. A vote delivered
before the node acknowledges the matching `LockAndCommit` is ignored
recoverably rather than creating an authority-free pool. The acknowledgement
first applies the new durable lock, then retires the superseded historical
Commit pool, and only then releases the current Commit signature. That ordering
keeps the old reconstruction pool intact if persistence fails. The adapter's
locked-Commit consumer epoch changes at this boundary, so the previously
ignored exact vote may cross admission once after the lock exists without
weakening the independently tracked, rotation-bounded equivocation fingerprint
history. Complete authenticated conflict-pair persistence for penalties remains
separate future work.

Reducer transitions also check executable progress witnesses. A durable locked
Commit intent must be represented by signing work, the exact local vote pool,
recovery ownership, or a decision. Retained outbound control is a peer-delivery
source, but it is not a sufficient local witness because broadcast excludes the
sender. A TC-promoted lock without an intent must retain exact body-recovery
ownership until validation; once validated, the pending historical
`LockAndCommit` append is the required witness. A durable decision awaiting
application must retain its exact body pipeline and may not refer to a body
which deterministic validation marked invalid.

Decision retransmission reconstructs the next missing owner from the durable
body stage instead of assuming a fetch is always sufficient. A missing body
restarts recovery, an available body restarts `StoreBody`, a durable body
restarts `ValidateBody`, and a validated body restarts `Apply`. Dropping any
one volatile owner therefore leaves a deterministic reconstruction path from
the durable Decision and body-stage record.

Decision installation is also a terminal ownership boundary. Before its
`FetchBody` effect can reserve capacity, the executor retires every competing
fetch, store, validation, signing, proposal, candidate, outbound, lane, and
retransmission owner. It preserves only the exact decided body pipeline and its
exact merge-sidecar deferral until application starts; application then drains
that remaining volatile work. Late I/O completions cannot recreate proposal or
lane work after this tombstone.

Those witnesses also cover recovery boundaries. WAL replay after durable
`LockAndCommit` restores the exact pending Commit signature and broadcast. For
a historical record, replay first requires the same exact lock to be active
after the preceding TC installation; `InstallTimeout` alone never synthesizes
a signature. A validated body retained under a lock survives leader loss.
After leader rotation, the retained body and the existing or reconstructed
exact old-round Commit intent can therefore rebuild the old Commit quorum
instead of leaving a lock with no executable owner.

Proposal replay joins two independently fsynced authorities before startup
effects run: the safety WAL supplies the exact proposal intent and the body
store supplies its canonical body plus deterministic execution commitment.
Startup rejects any identity or commitment drift, otherwise restores the
commitment into the replayed registry so a re-signed proposal can proceed
directly to its Prepare vote even when the durable intent belongs to a nonzero
view.

The serialized runtime and adapter each reserve completion and progress
capacity. Their cyclic `completion -> progress -> normal` service order keeps
FIFO order within a class and records eligible service skips only for the
oldest item in a non-selected class. Exact locked Commit votes authenticate
through the progress reserve even when ordinary ingress is saturated.

Body-pipeline completion ownership spans both runtime ingress and the
adapter's Busy-deferred queues. The deferred owner retains the full manifest
and non-forgeable durable or validation receipt, including validation
success/failure polarity. A retry coalesces only when that complete evidence
is equal; conflicting evidence or more than one owner fails closed. The check
runs before a `BodyAvailable` completion can prune conflicting queued
proposals, so a non-exact retry cannot mutate consumer state and then hide as
a duplicate.

`LocalProposalReady` is trusted completion work, not ordinary proposal ingress.
It uses the runtime Completion reservation and, if the reducer reports Busy,
the adapter's Busy-deferred Completion reservation. Saturating Normal ingress
therefore cannot strand a locally built, durably stored, validated proposal.

Cryptographic authentication also checks commitment-bearing consensus evidence
before the envelope receives serialized runtime ownership. An individual
signed Vote cannot establish execution-commitment authority: its exact round,
subject, and commitment must already be bound by a local validated receipt,
verified safety-WAL replay, or quorum-authenticated QC evidence. An unbound Vote
is rejected without closing the runtime and may be retried after an authoritative
binding arrives. QCs, TCs, proposal justifications, and certificate transport
are checked for conflicts both against existing authority and within the one
authenticated envelope.

`BodyAvailable` rebind is authorized only after the reducer has installed its
destination tag. Before mutation, the runtime inventories the exact source and
destination owners across ingress and Busy-deferred state. One exact source may
move into an empty destination or coalesce into one exact destination owner;
an uninstalled destination is a recoverable caller-contract rejection with no
mutation. Conflicting destination evidence or duplicate source/destination
ownership fails closed without changing either side. Body-pipeline and Decision
retirement use the same transactional preflight: every owner count and evidence
invariant is checked before any queue entry is removed.

Higher-lock, Decision, and certified-view cleanup also treat an external
cancellation failure as a restart boundary, never as a successful retirement.
The executor keeps the exact fetch/store/validation owner until all required
service cancellations acknowledge; a failure latches the process-wide
restart-required guard while the safety WAL or durable Decision remains the
reconstruction source. Adversarial tests inject cancellation failure at each
of these three cleanup boundaries while an exact certified fetch is pending
and require that fetch owner, signed-request hash, and accounting projection to
remain exact. Cleanup preflights both certified-request indexes before runtime
or service mutation and commits their removal as one infallible serialized
step. Separate corrupt-index tests require lock, view, and Decision cleanup to
fail closed before issuing even the first external cancellation. Direct
locked-body reconciliation closes itself on a service/runtime cleanup failure
instead of relying solely on the outer runner to terminate.
Multi-fetch view cleanup also completes every ordered service callback before
committing any executor or certified-request retirement; failure on the second
callback therefore leaves the complete executor projection unchanged and
forces restart recovery.
Decision cleanup similarly defers detaching an exact decided local
store/validation consumer until all losing service owners acknowledge
cancellation, so a failed losing fetch cannot orphan the decided local
pipeline.
If the final status publication fails after cleanup commits, the replacement
lock and retired certified indexes remain the authoritative executor state and
the node still fails closed; retries cannot resurrect the superseded owner.
Across later view-cleanup categories, an already acknowledged signature,
store, or validation cancellation is not rolled back if a subsequent callback
fails. The fail-closed process must restart; WAL replay restores the TC, lock,
and Commit intent, while the locked QC recreates any required body fetch.
Unprotected stale work is not a progress witness in the installed view.

Remote certificate conflicts at the body-recovery transport boundary are
nonfatal. A certified-body request whose QC conflicts with local authority is
rejected without closing the runtime, and a conflicting Commit-certificate
response leaves discovery outstanding so another authenticated peer can be
retried.

The adapter's deferred Progress reserve is partitioned by consumer ownership:
one locked-Commit slot per frozen validator and one independent slot for each
PrepareQC, CommitQC, and TC class. Exact retransmissions coalesce before this
capacity check. Once a progress item is admitted, later equal- or higher-ranked
traffic cannot displace it; a full class rejects the new item while preserving
the already admitted reconstruction or certificate owner.

The roster-aware transport ingress also prevents auxiliary I/O backpressure
from becoming a per-validator head-of-line stall. On each source's fair turn it
removes the oldest message which the downstream consumer can currently admit;
earlier blocked messages remain owned in their original order. A certified-body
request waiting for auxiliary I/O capacity therefore cannot hide a later
proposal, QC, TC, body response, or payload chunk from the same authenticated
validator. The source still consumes only one turn, and the retained head is
selected first as soon as it becomes admissible.

An empty validator lane reserves both a first-message slot and a later Progress
slot; a non-empty lane without Progress retains the latter, and a sole Progress
item retains the continuation slot needed to restore both empty-lane
reservations after service. Outer Progress is deliberately narrower than
ordinary traffic: Commit votes, QCs, TCs, payload chunks, certified-body
responses, and Commit-certificate responses qualify; proposals, Prepare votes,
timeout votes, manifests, and requests do not consume that reservation. Pending
exact retransmissions coalesce only for the same transport sender and canonical
envelope hash, and the coalescing authority ends when the consumer removes the
queued occurrence.

Count bounds are paired with canonical-wire byte bounds. The
`sumeragi.queues.body_source_bytes` quota isolates each frozen-roster validator
and the shared untrusted lane, while `sumeragi.queues.body_bytes` bounds their
aggregate ownership. Roster installation fails closed if the aggregate cannot
provide every source partition. Each authenticated source also retains an
isolated timeout-vote byte reserve, so an ordinary maximum-size body cannot
consume the capacity required to advance its view. These count, byte, Progress,
and timeout reservations prevent one authenticated source from consuming
another validator's recovery capacity or turning the count-bounded queue into
multi-gigabyte memory ownership.

The peer sender extends that ownership boundary through encrypted stream I/O.
It retains at most one bounded plaintext retry in each safety, ordinary-high,
and low pool when encrypted-frame capacity is full; the safety pool has
independent frame capacity. A write that is cancelled after its bytes reach the
stream but before flush completion retains the non-empty batch as a pending
flush witness, resumes the flush before staging later work, and never writes
the batch twice. One read/write arbiter polls both reliable streams and both
outbound senders, alternating equally ready high/low work. Direct post intake
has a finite burst; on exhaustion a non-cancelable checkpoint gives reliable
stream I/O first refusal before intake reopens, so continuously ready
best-effort datagrams cannot starve consensus traffic.

## Liveness snapshot

`GET /v1/sumeragi/status` and `/v1/sumeragi/status/sse` expose the same required
`liveness` object in the canonical `SumeragiV2Status` payload. It contains:

- the reducer generation and exact Prepare, Commit, and timeout partial pools,
  including distinct signer count and signed/total voting power;
- durable outbound proposal, vote, QC, timeout-vote, and TC intents, with their
  persistence, signature, queue, or sent stage;
- candidate, body recovery/store, validation, application, and successor
  activation work. Candidate work reflects an actually owned candidate load or
  artifact and is never inferred merely from the local validator being leader;
- retained semantic-admission occupancy and, separately, the live bounded
  transport-to-runner `network_ingress`, adapter, and runtime queue depth,
  capacity, oldest age, and service debt. Semantic-admission age is diagnostic
  history and is not treated as scheduler debt. Queue age remains diagnostic
  context and never proves scheduler starvation by itself. Queue-based
  starvation requires eligible-skip debt covering a complete three-class
  service rotation. Network ingress publishes zero synthetic debt and instead
  maintains a private service clock: while the queue is non-empty, a completed
  admission scan resets that clock even if every item remains blocked. Only a
  watchdog interval with no scan classifies network-ingress starvation;
- the bounded worker-to-executor `EffectCompletion` handoff, including its
  depth, capacity, oldest retained age, and service debt, so completed local I/O
  cannot remain invisible behind a full runtime completion lane;
- the last semantic transition, its age, the height no-progress age, and every
  reducer ignore-reason count.

The watchdog deadline is derived from the configured, view-aware round timeout
plus one retransmission interval. Its height-wide progress rank is bounded by
the greatest semantic stage reached and the maximum Prepare/Commit signer count
and signed power observed at that height. View and reducer generation are not
rank components. Timeout-vote admission, TC installation, and reconstruction of
the same locked Commit pool update current diagnostics but cannot reset the
height-progress clock; repeated view/generation cycles are not treated as
height progress. After the deadline, a snapshot classifies the current delay as
exactly one of:

- `missing_proposal`
- `body_unavailable`
- `prepare_quorum_missing`
- `commit_quorum_missing`
- `timeout_certificate_missing`
- `scheduler_starvation`
- `application_pending`
- `local_control_pending`

`local_control_pending` distinguishes a reducer blocked on safety-WAL
persistence or consensus signing from scheduler starvation. Queued outbound
work and formed-but-unconsumed QC or TC transitions remain structural scheduler
witnesses even when no queue-debt sample is available.

Merge-sidecar ownership is classified by its actual consumer. Validation work
waiting for the exact sidecar remains validation/body-unavailable work, whereas
a durable `Apply` waiting for that sidecar is `application_pending`; the
aggregate retained-work counter is not used to collapse these two stages.

Successor activation is likewise tied to runner ownership rather than inferred
from application alone. After application finalizes the height, the runner
publishes successor construction as `Running` before building the verified
context. It publishes `Complete` and emits `SuccessorHeightActivated` only
after verified successor construction succeeds, leaving a stalled handoff
visible at the finalized height.

The classification is diagnostic. It does not weaken reducer safety checks or
manufacture a progress event.

## Implementation evidence and pending inventory

The following focused checks were recorded through 2026-07-16:

- all 248 `iroha_p2p` library tests, including bounded plaintext retention,
  cancellation-safe flush, read/write arbitration, and direct-post exhaustion;
- all 19 fair outer-ingress tests;
- three exact locked-Commit generation/readmission tests, one exact WAL replay
  witness test, and one locked-body leader-crash recovery test;
- terminal Decision cleanup across executor, runtime, runner, lane, candidate,
  outbound, and completion ownership, plus a real nonzero-view WAL/body-store
  proposal replay through production signing and Prepare continuation;
- all 55 serial `sumeragi::v2_lane_work::tests`, including the production
  Native-AMX committee, PrepareQC-retention, Decision cleanup, and lane-session
  ownership boundaries;
- the status contract accepting all 12 distinct ignore reasons and rejecting a
  thirteenth entry, with tag-11 `unsafe_proposal` parity in the maintained SDKs;
- a fresh exact four-validator genesis run with networking required and one
  permitted startup attempt, which committed and activated the successor height
  on all validators in 458.47 seconds including the separate source-bound
  daemon build;
- the then-current formal-ledger checker suite and clean SANY
  syntax/semantic analysis;
- one exact, no-retry four-validator genesis attempt using a freshly built
  `iroha3d`, which committed on every validator in 59.09 seconds. The daemon's
  SHA-256 was
  `0bff8b990a6f653b69b26b039ac26a4abdcba6cbb8f85c9fef252b81cdbab0df`;
- an exact-evidence four-validator genesis reproduction which passed its first
  attempt in 351.76 seconds, with retained logs under
  `target/sumeragi-v2-genesis-recheck-exact-evidence-20260715/irohad_test_network_3XUobQ`;
- the complete eleven production-liveness modules at 56/56 reducer/core,
  10/10 refinement, 9/9 reducer source-link, 55/55 adapter, 26/26 apply,
  115/115 effects, 55/55 lane work, 38/38 runtime, 19/19 runner, 66/66 worker,
  and 14/14 watchdog tests (463 passed, 0 failed, 0 ignored). The broader
  serialized Sumeragi v2 namespace passed 582/582 tests, and release discovery
  confirmed all 124 explicitly required production tests are present and
  non-ignored.

The focused source-bound additions above are green. The gate names nine
completion-ownership regressions: exact ingress/Busy-deferred
coalescing, conflicting manifest and receipt evidence, conflicting local and
validated receipts, production Busy transfer, cross-queue retirement,
transactional duplicate-owner failure, and three installed/destination-rebind
cases. It also names the two production certificate-transport conflict tests
and the runtime unbound-Vote authority test, plus the reducer exact-lock and
adapter consumer-epoch tests. The final-source exact one-attempt four-validator
genesis rerun passed on all validators in 456.76 seconds, with evidence retained
under
`target/sumeragi-v2-genesis-final-hardening-20260716/irohad_test_network_7sJJmM`.

This is implementation and regression evidence, not a release-completion
claim. The proof ledger still reports `machine_checked_completion: false`, and
14 obligations remain `specified_unproved`. They include the historical
TC-lock authorization and the dependent timeout theorem which now requires
either direct protection by the target TC or a non-strict honest
timeout/Commit intersection witness with durable installed-TC authorization.
A standalone final-source 100,000-height permissioned/NPoS chaos run preserved
both 50,000-height chain prefixes in 52.97 seconds; the release profile must
still reproduce it under the checkout-manifest-bound evidence root. The fully
pinned 24-hour Taira-profile soak also remains outstanding. The complete PR
corridor is not claimed green here until its source-bound run finishes.

## Deterministic and production gates

The PR corridor runs four fixed seeds for the four-validator genesis, restart,
timeout-rotation, and divergent-PrepareQC scenarios, together with adversarial
reducer simulations and model-trace replay:

```bash
bash scripts/run_sumeragi_v2_release_gates.sh --pr
```

Before those longer scenarios, the PR gate inventories 103 exact production
liveness tests and executes all ten owning Rust modules serially. The inventory
is the prior 101 named tests plus the reducer exact-lock and adapter
consumer-epoch regressions. Nine tests pin the completion-ownership seam
described above. It then inventories and executes the exact Rust
positive/negative cross-SDK wire-fixture tests and
the maintained JavaScript and Python authoritative-status parser tests. The
parser inventory pins normalization, `local_control_pending`,
`unsafe_proposal`, and the full 12-reason ignore bound; a missing or ignored
Rust fixture test or a missing named SDK parser test fails the gate. The
inventory and serial ten-module execution are green; this is still not evidence
that the complete source-bound PR corridor has passed.

The same pre-network gate inventories and executes four exact, non-ignored
Taira release-profile validators plus the Rust summary-JSON schema contract.
It then requires exactly 38 passing mocked soak launcher/evidence tests. Those
tests reject profile drift, zero-test success, concurrent evidence ownership,
source or artifact mismatch, malformed JSON, weakened acceptance bounds,
inconsistent counters, and forged status classifications. They validate the
release machinery; they are not substitutes for the 24-hour validator soak.

Both profiles compute the canonical tracked/untracked checkout manifest before
their first Cargo command, bind the ignored workspace `Cargo.lock` as an
explicit build input, reject unresolved Git index entries, export the digest to
the network matrix, and use it as the build-root identity. The seed runner
compares that parent digest after test inventory, before and after every
scenario, and on both sides of completion publication; the PR profile also
recomputes it after the formal harness. Any drift leaves only partial evidence
and fails the corridor.

Before any network attempt, the release gate also requires all six exact
source-manifest contract tests to pass. They cover content and ordering,
deletions, symlinks, executable modes, ignored `Cargo.lock` drift, unmerged
index parsing, and fail-closed rejection of unresolved entries.

Seed evidence is isolated per invocation beneath a manifest-addressed root. An
atomic directory lock rejects a concurrent writer to the same root, and a
unique invocation directory preserves earlier and failed runs instead of
deleting them. `invocation.tsv` records the source identity, fixed run count,
and internal harness deadlines; `summary.tsv` binds every command row to that
source digest. Only an exact complete matrix receives an atomically published
`COMPLETED.tsv`, which includes the SHA-256 of the summary. A zero-test result,
ambiguous libtest output, command failure, source change, or interrupted/stuck
invocation has no completion attestation. A hard-killed runner deliberately
leaves its evidence lock for operator inspection.

The matrix pins the integration harness's bounded build, subprocess, network
permit, startup, synchronization, status, and shutdown waits. Repository
process-safety policy forbids an outer watchdog from signalling Cargo, rustc,
or validator process groups, so it cannot promise a wall-clock bound if an
execution path escapes all of those internal deadlines. Such a pathological
run may remain active, but it remains fail-closed evidence because it cannot
publish `COMPLETED.tsv`; it must be inspected and resolved before retrying.

The production corridor raises the real-network matrix to 32 seeds per
scenario, requires fresh source-bound TLAPS/TLC/Verus and trace-replay evidence,
runs the 100,000-height permissioned/NPoS chain-prefix chaos test, and pins the
Taira-profile seed, load, packet loss, churn cadence, acceptance bounds, and
86,400-second duration. Load acceptance and commit rates use the full elapsed
wall time, not a denominator with churn work removed. Churn deadlines stay
anchored to the original schedule; at least 90% of the expected process and
membership cycles must execute, a cleanup leave does not count as a scheduled
cycle, churn work may consume at most 25% of elapsed time, and an in-flight
bounded churn action may overrun the workload deadline by at most 15 minutes.
The strict formal release gate runs before the 32-seed matrix, so an incomplete
proof ledger or stale backend evidence fails before 128 real-network attempts.
The PR profile retains its four-seed matrix followed by the fast formal harness.

The production soak runner clears inherited daemon/Kagami overrides, pins
release-profile binaries, `CARGO_NET_OFFLINE=true`, and `RUST_LOG=info`, and
builds under a SHA-256 checkout-manifest-addressed target. When invoked by the
release corridor, it rejects a checkout digest that differs from the parent's
manifest before inventory or compilation, rather than spending 24 hours on a
candidate the parent must later reject. An atomic lock gives
that source root exactly one soak-evidence writer; a hard-killed run leaves the
lock for explicit operator inspection. Before any real matrix, the release
gate inventories and executes the five exact Rust release-profile/schema tests
and requires all 38 mocked soak launcher/evidence adversaries to pass. The soak
runner then inventories the exact ignored test and accepts Cargo output only
when exactly one test ran and passed with networking required. The seed matrix
also forces one fresh network startup attempt, so a later harness retry cannot
hide a protocol-induced stall. The fixed durable summary records the Git
revision, checkout manifest, daemon, Kagami, test-binary and generated-config
digests, the complete profile, cadence accounting, and authoritative
initial/final Sumeragi status quorums. The localnet directory is retained, and
the evidence checker independently recomputes the canonical workspace manifest,
requires every binary to remain beneath its release-profile source-bound
directory, and re-hashes those artifacts before the runner reports success.
Duplicate object keys and non-finite numbers, including finite-syntax exponent
overflow such as `1e10000`, are rejected recursively before schema validation.
Retained status snapshots preserve their original validator
index and must contain at least three distinct responsive validators, wire
protocol 3, no restart-required node, the complete liveness object, and bounded
queue evidence. Every retained no-progress interval is accepted only when its
canonical classification set exactly matches the blockers in its authoritative
status snapshots. A checkout-manifest checkpoint immediately after the
100,000-height chaos run rejects drift before starting the 24-hour soak. A final
checkout-manifest and proof-evidence check rejects any later source change
during the long corridor:

```bash
bash scripts/run_sumeragi_v2_release_gates.sh --release
```

The release command is intentionally fail-closed while
`docs/formal/sumeragi_v2/proof_coverage.json` contains any
`specified_unproved` obligation or reports
`machine_checked_completion: false`. Bounded TLC searches and convincing paper
arguments do not upgrade that ledger state.
