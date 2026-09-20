# Canonical Queue cleanup on local selection release

Work location: `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`.
The Sumeragi liveness redesign remains active; this record covers one local
retirement obstruction, not complete runtime or release qualification.

## Ownership defects

State-authenticated replay cleanup cannot remove a globally bound claim while
selection, a popped transaction guard, or a selection attempt still owns it.
Previously that refusal discarded the cleanup obligation. Releasing the local
owner did not retry it. Subsequent candidate snapshots skipped the committed
transaction, while retirement still counted its Queue claim and route. The
production caller was post-Apply cleanup, so the Queue API had no release-driven
continuation and depended on another cleanup invocation.

The regressions reproduce this Queue interleaving with canonical State evidence
and the original local owners. They do not reproduce a live runner stall. In
particular, the current typed Apply dispatcher checks that runner Decision
cleanup has finished at both readiness and dispatch. Older same-batch comments
do not establish that Apply can precede this handoff.

A separate error path in `release_pending_autonomous_reservation_batches`
removed the entire pending map before fallible direct releases. A refusal then
dropped both the failed original batch and every unvisited batch, although their
Queue reservation owners remained. The adapter now borrows the first batch and
removes it only after its checked release succeeds; completed prefixes remain
completed and the untouched suffix retains its original allocations.

## Implemented ownership

The original durable claim now records its process-local custody: available,
autonomous, or canonically terminal while awaiting ordinary selection release.
Only the existing State-authenticated cleanup path can establish the terminal
state, after checking the byte-exact binding and excluding every reservation
or terminal barrier. Releasing or narrowing its actual selection lease resumes
that exact claim. The last selection attempt or popped guard also resumes
retained cleanup, outside Queue locks. A dirty hint avoids scanning claims when
no cleanup has been deferred; it grants no deletion authority.

Terminal claims cannot be selected again or rebound to another admission.
Cleanup uses the existing force-synced exact journal tombstone and only then
releases memory, capacity, routing and FIFO ownership. Existing retirement
notifications report actual predicate release. A durability failure retains
the in-memory owner, closes selection, and wakes recovery.

Autonomous custody survives Queue release and FIFO restoration: forgetting a
Queue release does not establish Kura Complete. Only the checked pre-Kura or
strict-absence direct release restores ordinary eligibility. Existing Kura
Complete-authorized cleanup remains responsible for actual autonomous
terminalization. Restart reconstructs custody behind the existing Queue
quarantine and authenticated State/Kura reconciliation.

The pre-Kura reset relies on the existing single mutable lane adapter: its live
retirement corridor finishes Kura Complete before another reservation can start.
Startup keeps reservations closed until the same reconciliation completes. The
planner alone does not establish that exclusion; a new concurrent caller must
preserve it. The current release path does not infer Complete from Queue FIFO
restoration.

## Validation

Build102 passed all seven selected package test builds, 383 Queue/Apply tests
and the 32 original review regressions with unchanged Rust inputs and test
binary. The nine new Queue regressions use real
QueuePlan and reservation journals, canonical State application evidence,
original selection leases and popped guards, and the actual retirement release
future. They require cleanup without a second Apply, retain live or released
autonomous claims without a Kura Complete authorization, and inject a journal
parent-sync failure before in-memory deletion. Receipts are in
`dist/sumeragi-main-work/build102-receipt.json`, `queue-release102/summary.json`
and `review-current102/summary.json` under that work directory.

Build103 passed after moving the new test modules into the existing
reviewed Queue test provider and fixing pending batch retention. Its additional
regression holds the actual Kura activation fence, requires refusal to retain
the original allocation, keys, journal and FIFO, then releases the fence and
requires the same batch to restore FIFO exactly once. The formal include gate
requires tracked providers; no index mutation or weaker source admission was
used to admit the new tests.

The unchanged build103 candidate passed:

- Seven selected package test builds in 179.6 seconds, including Core library
  tests and the selected daemon/network integration targets (`--no-run`).
- All 385 selected Queue, Apply and pre-Kura release tests, including all ten
  new ownership regressions and the 256-barrier durable restart test.
- All 32 original review regressions: sealed input helpers, complete carrier
  framing, single-copy authenticated gossip, and owned repair temporaries.
- Focused source/ledger controls, including mutations of release order,
  autonomous custody, retained batch ownership and strict release authority.
- Scoped Rust formatting and the no-legacy-codec guard.

The two runtime suites contain 417 distinct tests and use the same captured Core
binary (`0cd760387ae012ded9d05a2f237f236bcb39d9389fc0c3d5108ad04fc037f095`).
Their Rust inputs, branch, HEAD and index remained unchanged. Build, suite and
source receipts live under `dist/sumeragi-main-work`, respectively
`build103-receipt.json`, `queue-release103/summary.json`,
`review-current103/summary.json` and `replay-terminal103/controls-final/summary.json`.
The first full canonical source gate returned three stale startup-replay ledger
entries. That unchanged failed run is preserved in
`replay-terminal103/canonical/summary.json`; it is not passing evidence. The
new custody entries had replaced the startup declarations for shared owners.
The correction retains their complete union in both expected tables and the
reviewed ledger, with acceptance, equality and token-removal controls. The
earlier 51-control passing receipt remains in `replay-terminal103/controls`.
The follow-up full gate records
its separate result in `replay-terminal103/canonical-final/summary.json`.
These gates check source/model consistency, not live consensus or network
qualification.

Remaining work includes the original service-bound prepared Validate-to-Apply
handoff, complete resource admission, certified retirement publication,
production Native shared-reducer cutover, and unchanged four/seven-validator
fault/restart/final-transaction and workspace qualification.
