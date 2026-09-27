# Native validation source refresh under concurrent State publication

This is local development evidence from `/Users/takemiyamakoto/dev/iroha` on
`optimizations`. It does not close the Sumeragi redesign or release gate.

The first candidate passed the four-validator same- and distinct-subject cases,
the nine-peer observer-recovery case, seven-validator restart and NPoS
stopped-leader rotation. Its silent-initial-author case failed after 218.38
seconds: one validator closed output with `Native source observation changed
before execution` while the network was preparing the height-2 carrier. The
State source reader had correctly detected a concurrent committed-view change,
but Native validation classified this read-only race as durable local recovery
failure. The retained candidate was stopped instead of refreshing its source.

The validation owner now retains the original proposal, recovered inputs and
carrier-shell reservation when source authentication or pre-execution candidate
preparation observes a changed State generation. It retries that same owner
after a typed local `ObservationChanged` refusal. Shell admission transfers only
after successful preparation; a refresh cannot publish a validation marker,
rerun economic execution, or close output for restart. A post-execution Apply
reporting this pre-execution refusal still fails closed.

Focused Core checks pass for the retained Validate dispatch's exact retry,
source preparation after an actual finalized-height advance, and ordinary
body-store retention. Core library checking and the current multilane binding
gate pass; the binding ledger and its expected source tokens were updated
together.

The finalized-height test establishes the State source's obsolete-observation
result, not end-to-end retirement of an already retained validation dispatch
when a different successor finalizes. That lifecycle cancellation path still
needs a production-level test and a nonfatal terminal owner; retrying a
permanently obsolete proposal cannot be the final design.

One matching captured Rust/manifest/fixture source built both the controlled
daemon and ordinary integration harness (`checkpoint-native-refresh-daemon116`,
`checkpoint-native-refresh-harness116`). On these unchanged binaries, one
real-network start attempt per run passed:

- `silent-author-refresh-local15`: four-validator silent initial Native author,
  94.02 seconds;
- `observer-pressure-refresh-local16`: four validators and five slow observers,
  175.38 seconds;
- `same-subject-refresh-local17`: locked reproposal under held messages,
  82.67 seconds;
- `distinct-subject-refresh-local18b`: competing PrepareQCs after causal
  release, 179.07 seconds;
- `seven-restart-refresh-local19`: two validator restarts in a seven-validator
  network, 249.12 seconds;
- `npos-leader-refresh-local20`: stopped-leader rotation, 174.28 seconds;
- `four-restart-refresh-local21`: validator restart in a four-validator network,
  128.96 seconds.

The first distinct-subject run on those **same binaries**,
`distinct-subject-refresh-local18`, failed after 137.06 seconds because one
peer did not finish its controlled FIFO heal-and-drain fence within the test's
20-second acknowledgement window. Three peers acknowledged; the delayed peer's
final ACK appeared at the boundary after the test stopped. A later identical
run passed. The controller's exact in-flight wait was not retained in that red
run, so the cause is unresolved. Two further unchanged-source monitored repeats,
`distinct-subject-capture-local22` and `distinct-subject-capture-local23`, pass;
their four peers each drain in 1.39–2.26 seconds while the local diagnostic
records every changing held/in-flight ACK. Those fast repeats do not explain
the earlier approximately 20-second stall or clear the red signal. The next
reproduction must retain the in-flight descriptor and downstream readiness
state at the timeout before changing the 20-second test bound.

The integration test now includes its exact pre-drain and current controller
ACK in a drain-timeout error. This is a diagnostic-only harness change; the
controlled daemon bytes remain SHA-256
`c0259f907e589935a95e0bd8ad363aace3a265061a859462d7f43d914cf51c8f`.
The rebuilt matching-source daemon/harness pair passes two more one-start
distinct-subject runs (`distinct-subject-diag-local24` and
`distinct-subject-diag-local25`, 185.51 and 185.45 seconds). Their monitored
ACKs also reach a drained revision-4 fence. The original red run remains open.

These are source-joined local network diagnostics, not a clean signed release,
the full loss/reordering/backpressure/final-transaction campaign, or a general
liveness proof. The original lifecycle simplification and complete memory
admission remain open.

## Superseded retained Validate owner

The later candidate distinguishes a stable finalized State head at or beyond
the old proposal height from a transient State-generation change. Before Native
execution, the former yields a typed supersession refusal. The lifecycle
Completion owner also checks the finalized head while an original first-input
recovery or source-refresh wait remains parked, so it does not require an
obsolete response to become runnable again.

That owner reattaches the original worker dispatch to its exact Waiting
Validate row and concrete body carrier, persists a `Cancelled` LedgerV1
successor, removes that carrier, then acknowledges the guarded worker
completion. A failed ledger write leaves both the waiting row and carrier
unchanged; no validation marker or invalid-body verdict is fabricated. The
same-height publication is now covered by a State-source regression. The
exact cancellation, write-failure, and driver-level guarded-completion cuts
pass three focused Core tests. The wider `superseded_` Core selection passes
21/21, and the Native preparation and physical Validate retry selections pass
1/1 and 3/3 on the same test executable.

The changed local-release daemon has SHA-256
`8c0440e36cc195c74b584982c55a7c1a35a27d6756f144237946edacbe68b20d`;
the final isolated network harness has SHA-256
`c49bdf2d722692f0b8907567ce82c6eca200e0e601bf66a358ddbae6074b4cf6`.
One four-validator silent-initial-author outage run passes in 97.82 seconds:
the other three finalize the finite input, and the restarted author agrees on
the committed result. One four-validator distinct-subject PrepareQC run passes
in 183.83 seconds after causal message release; all four authenticated message
controllers acknowledge the exact revision-4 drain fence with no held or
in-flight message. One seven-validator run with two validator restarts passes
in 219.66 seconds, including finality during the outage, recovery of both
identities, and the finite final transaction. One four-validator/five-observer
slow-reader relay run passes in 173.43 seconds, including exact successor
recovery and measured relay pressure. One four-validator NPoS stopped-leader
run passes in 174.28 seconds with bounded timeout-certificate rotation. The
four-validator same-subject locked-reproposal run passes in 65.35 seconds
after ordered quorum release. One four-validator restart run passes in 151.40
seconds, including continued finality during the outage and after recovery.
The retained logs are under
`dist/sumeragi-main-work/generation251-superseded-validate-network`.

The first attempted changed-daemon run stopped at Torii startup because the
transaction-history route was declared optional while its handler required a
canonical account signature. The route catalog and mount now agree on signed
account admission. The next run reached the block-2 read and received HTTP 401
because this harness used an unsigned raw GET for a canonical-authenticated
ledger carrier; the harness now signs that exact network request. Neither run
established a consensus outcome. The production Torii route tests pass 90/90.

The intermittent controlled-drain timeout above remains an independent open
signal. The harness wait now extends only when the exact active drain records
additional delivered or retired messages and has a fixed six-times hard cap;
this prevents a progressing drain from failing solely at the original 20-second
idle boundary without hiding a stalled controller. Its focused unit test passes;
the current multilane formal checker and Rust formatting check pass. These
network passes do not prove every superseded Validate cut was exercised or
close the four/seven-validator fault campaign or release gate.
