# Sumeragi v2 liveness contract and release gate

Sumeragi v2 does not promise unconditional termination. An unbounded network
partition, the absence of a responsive dual quorum, or local disk, signing,
validation, and application work which never completes can prevent progress.
The first-release target is therefore conditional:

> After GST, with a responsive dual quorum and terminating local work, every
> height eventually decides and every responsive validator eventually applies
> the decision and activates its successor height.

## Progress ownership

The reducer and its adapter enforce one structural rule at persistence and
view-change boundaries: volatile progress state may be cleared only while a
durable source retains a fairly scheduled reconstruction path. In particular,
a timeout certificate may clear a volatile vote pool, but it does not clear an
active PrepareQC lock or the corresponding durable Commit intent. Each
authenticated Commit vote which exactly matches that lock may cross semantic
delivery admission once in each reducer generation. Equivocation history is
height-bounded independently from this generation-scoped delivery record.

Reducer transitions also check executable progress witnesses. A durable locked
Commit intent must be represented by signing work, retransmittable outbound
state, the exact vote pool, recovery ownership, or a decision. A durable
decision awaiting application must retain its exact body pipeline and may not
refer to a body which deterministic validation marked invalid.

Those witnesses also cover recovery boundaries. WAL replay after durable
`LockAndCommit` restores the exact pending Commit signature and broadcast, and
a validated body retained under a lock survives leader loss. After leader
rotation, the retained body and exact old-round Commit intent can therefore
rebuild the old Commit quorum instead of leaving a lock with no executable
owner.

The serialized runtime and adapter each reserve completion and progress
capacity. Their cyclic `completion -> progress -> normal` service order keeps
FIFO order within a class and records eligible service skips only for the
oldest item in a non-selected class. Exact locked Commit votes authenticate
through the progress reserve even when ordinary ingress is saturated.

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
  activation work;
- retained semantic-admission occupancy and, separately, the live bounded
  transport-to-runner `network_ingress`, adapter, and runtime queue depth,
  capacity, oldest age, and service debt. Semantic-admission age is diagnostic
  history and is not treated as scheduler debt; network ingress currently
  publishes zero synthetic debt and uses its directly measured oldest age as
  the starvation witness;
- the last semantic transition, its age, the height no-progress age, and every
  reducer ignore-reason count.

The watchdog deadline is derived from the configured, view-aware round timeout
plus one retransmission interval. Timeout-vote admission and TC installation
update the last-transition diagnostic but do not reset the height-progress
clock; repeated view changes are not treated as height progress. After the
deadline, a snapshot classifies the current delay as exactly one of:

- `missing_proposal`
- `body_unavailable`
- `prepare_quorum_missing`
- `commit_quorum_missing`
- `timeout_certificate_missing`
- `scheduler_starvation`
- `application_pending`

The classification is diagnostic. It does not weaken reducer safety checks or
manufacture a progress event.

## Verified implementation evidence

The focused implementation corridor completed on 2026-07-15:

- all 248 `iroha_p2p` library tests, including bounded plaintext retention,
  cancellation-safe flush, read/write arbitration, and direct-post exhaustion;
- all 19 fair outer-ingress tests;
- three exact locked-Commit generation/readmission tests, one exact WAL replay
  witness test, and one locked-body leader-crash recovery test;
- all 59 formal-ledger checker tests and clean SANY syntax/semantic analysis;
  and
- one exact, no-retry four-validator genesis attempt using a freshly built
  `iroha3d`, which committed on every validator in 59.09 seconds. The daemon's
  SHA-256 was
  `0bff8b990a6f653b69b26b039ac26a4abdcba6cbb8f85c9fef252b81cdbab0df`.

This is implementation and regression evidence, not a release-completion
claim. The proof ledger still reports `machine_checked_completion: false`, and
the 100,000-height chaos run and fully pinned 24-hour Taira-profile soak remain
outstanding.

## Deterministic and production gates

The PR corridor runs four fixed seeds for the four-validator genesis, restart,
timeout-rotation, and divergent-PrepareQC scenarios, together with adversarial
reducer simulations and model-trace replay:

```bash
bash scripts/run_sumeragi_v2_release_gates.sh --pr
```

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

The runner clears inherited daemon/Kagami overrides and builds under a
SHA-256 checkout-manifest-addressed target. It first inventories the exact
ignored test and then accepts Cargo output only when exactly one test ran and
passed with networking required. The seed matrix also forces one fresh network
startup attempt, so a later harness retry cannot hide a protocol-induced
stall. The fixed durable summary records the Git
revision, checkout manifest, daemon, Kagami, test-binary and generated-config
digests, the complete profile, cadence accounting, and authoritative
initial/final Sumeragi status quorums. The localnet directory is retained, and
the evidence checker re-hashes those artifacts before the runner reports
success. Retained status snapshots preserve their original validator index and
must contain at least three distinct responsive validators, wire protocol 3,
no restart-required node, the complete liveness object, and bounded queue
evidence. Every retained no-progress interval is accepted only when its
canonical classification set exactly matches the blockers in its authoritative
status snapshots. A final checkout-manifest and proof-evidence check rejects
any source change during the long corridor:

```bash
bash scripts/run_sumeragi_v2_release_gates.sh --release
```

The release command is intentionally fail-closed while
`docs/formal/sumeragi_v2/proof_coverage.json` contains any
`specified_unproved` obligation or reports
`machine_checked_completion: false`. Bounded TLC searches and convincing paper
arguments do not upgrade that ledger state.
