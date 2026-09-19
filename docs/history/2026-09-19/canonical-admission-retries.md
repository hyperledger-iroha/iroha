# Canonical QueuePlan submission retries — 2026-09-19

All work uses `/Users/takemiyamakoto/dev/iroha` on `optimizations`. This task makes
no branch, worktree, index, commit or sibling repository changes. A concurrent
editor committed and staged changes in the same checkout during validation;
those changes are preserved and have separate qualification requirements.

A replica with an empty local Queue previously applied fresh routing and queue
capacity checks before consulting canonical admission custody. That could refuse
an already-carried input after its original route closed. Public submission now
consults the coherent State registry before those checks. It returns the existing
public signed submission receipt, or the requested minimal acknowledgement,
without fabricating current routing headers or creating a local claim.

Authenticated peer retries require the original certificate. After checking the
request identity, transaction, binding and route hint, the receiver reads the
exact first carrier identified by the canonical registry. The State reader joins
the immutable registry position and committed carrier hash, releases State guards
before Kura I/O, verifies historical finality and complete input, then rejoins the
same registry and history. Missing, malformed, changed or evicted evidence cannot
fall back to fresh admission. The original authenticated certificate is returned;
no new availability vote or local journal record is created.

The historical carrier may be much larger than the retried transaction. Its
complete read workspace belongs to the existing dedicated proxy memory slot.
Synchronous blocking I/O retains that reservation through physical completion;
the response body retains it through certificate delivery. Cold metadata reads
avoid implicit live-body cache materialization. Canonical block and metadata
authentication count serialized size before constructing comparison buffers,
while preserving exact byte equality checks. The wire format is unchanged.

The component fixtures publish admission-only State controls, a real canonical
block body, and separately authenticated finality. They do not stand in for a
real-peer consensus campaign. Controls cover pending/applied ownership, an actual
lane close, bad rank or claim, orphan state, damaged or foreign finality, eviction,
State-guard release and post-I/O replacement. Torii controls cover both public
entrypoints, signed/minimal receipts, empty/full Queue, no local peer or service,
forged requests, original certificate delivery and memory ownership.

Build61 passed. Its selected runtime run passed 115/134 on unchanged 7,060 inputs
and unchanged emitted binaries. Two new Core fixtures failed: one required the
existing explicit State test-stack harness, and one supplied DA/proof policy to
an intentionally admission-only staging helper. The fixture corrections retain
all production guards and assertions. All four new Torii retry controls passed.
The remaining 17 failures have three distinct causes. Fourteen single-submit
fixtures use ordinary Log transactions and receive the current QueuePlan intent
rejection before their intended queue, rate or fee branch. One batch fixture lacks
local committee ownership. Two batch token tests expect partial token consumption,
although the current limiter rejects an oversized batch atomically and preserves
the bucket. Their passing historical baseline has not been established; they
remain open. Migrating these tests must preserve their intended assertions and
explicitly resolve changed contracts, rather than weakening ingress or the limiter.
Their exact names and logs are retained in
`dist/sumeragi-main-work/validation61.json` and
`local-validation-controls61/summary.json`.

Build62 passed in 13m44s. Its emitted binaries passed all 1,692 selected tests
(1,215 Core, 62 Torii, 415 data-model block tests). An incorrect routed-memory
selector was corrected in a separate exact run, which also passed on the same
unchanged binary. A stale harness assertion expected the old 1,256-test inventory;
the following harness instead checks its actual selected inventory. Both harness
errors remain in the original logs. More importantly, the source seal failed:
150 captured Rust/configuration inputs changed, HEAD moved from `87e0e45d17`
to `166c455c06`, and external index changes appeared. These passing binaries do
not qualify that combined checkout. `dist/sumeragi-main-work/validation62.json`
records the source mismatch, complete test result and harness diagnostics.

Review of build62 identified that a single-block cumulative decoder allowance
does not cover every independently bounded read in the complete operation. Both
State observations, repeated Kura metadata authentication, SCCP projections and
selected-input validation need named allowances. The physical-read completion
also needs the original execution deadline rechecked before certificate output.
The following correction sums the exact bounded Kura reads and both State
observations with checked arithmetic, while charging a body-only proposal clone.
It preserves the concurrently introduced guarded Kura reader. The Torii helper
receives the original monotonic execution deadline, including its egress reserve,
and rechecks it after physical I/O before constructing the certificate response.
Its new control proves that late delivery does not consume the canonical owner
and that a subsequent exact retry still returns the original certificate.

TODO: Record final build, runtime, source-binding and static receipts after the
budget/deadline corrections are validated on an unchanged source.

Canonical retry is not yet fully independent of fresh admission policy: ingress
still evaluates current TTL, NTS health, cryptography and transaction limits
before the canonical branch. Separate authentication of an exact old submission
from fresh policy without admitting forged signatures. The 17 older handler
fixtures also need migration to the current first-release admission contract.
The generic batch endpoint itself permits ordinary application batches while
rejecting QueuePlanSynced and bypassing the threshold-key lifecycle exception
used by single submission. Resolve that production admission inconsistency before
treating the batch success expectation as the desired contract.
Exact terminal Queue cleanup, off-chain receipt carry/closure, complete capture
admission, original prepared Validate-to-Apply ownership and the production shared
lane reducer/publication cutover remain open. Full workspace and unchanged real
four/seven-validator fault/restart/final-transaction qualification remain open.
No liveness goal is complete.
