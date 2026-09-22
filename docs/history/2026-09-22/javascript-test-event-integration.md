# Fixed JavaScript test-event integration — 2026-09-22

Work remains in `/Users/takemiyamakoto/devstuff/iroha` on `optimizations`,
HEAD `bdadfae6175a89e5fdb56292a5a64b93b44a231f` plus preserved local changes.
This adds one component of the pending installed JavaScript producer; all
fourteen overall release goals remain open.

`scripts/sorafs_javascript_test_events.mjs` consumes the fixed Node 24 structured
event relation. The existing original assertion-contract fixture supplies all
46 top-level and nine nested case names. It checks original parentage, IDs,
stable source coordinates, serial sibling execution, all five case events,
plans, counts and terminal observations. Refusal permanently invalidates the
owner. Complete observations grant no authority to execute, publish or promote.

Actual Node 24 mechanics show that a final `after` hook can emit `test:fail`
while the process exits zero and the summary still reports success with zero
failed cases. The consumer rejects every failed event. It also preserves valid
reporter buffering in which completion arrives before start; it does not impose
an invented chronological order on those buffered observations.

Independent review found three ordering bypasses in the first proposal:
swapped root execution blocks, overlapping next-root dequeue, and overlapping
nested registration/execution. The integrated revision requires original serial
sibling order and the preceding successful pass. Canonical integer checks also
reject negative-zero counters and nesting. Durations remain finite nonnegative
numbers, including zero and fractions, without integer coercion or an arbitrary
new duration ceiling.

The maintained controls live under `scripts/tests/`, outside the SDK unit
discovery used by the Node 20 privacy lane. No compatibility fallback, skip or
profile exclusion was introduced. The source contract and the 172 original
assertion expressions are unchanged.

Root execution on actual Node 24.21.0 passes **63 controls**, with zero failures,
skips, TODOs, cancellations, stderr or drift in the observed source/runtime
bytes. The independent reviewer repeats those controls, checks the captured
286-event/55-case trace, and tests ordering, numeric and reentrant mutations.
The repeated 63 cases are not additional coverage.

Evidence:

- `target/first-release-javascript-test-events-20260922/revision-2/`: exact
  integrated proposal, prior failed proposals and actual runtime mechanics.
- `target/first-release-javascript-test-events-review-20260922/`: separate
  review, reproductions and unchanged-input measurements.
- `target/first-release-javascript-events-integrated-20260922/identity.json`:
  root command, input hashes, counts and original log.

These tests execute generated inert callbacks. They do not import an SDK/addon
or run the original native assertions. The actual fixed child must retain the
original source/runtime/installed tree and native snapshot through genuine
stream EOF, authenticate process completion and feed the original-index join.

## CI integration

Both SoraFS release and SDK workflows explicitly watch the event owner and its
controls. The SDK workflow runs the complete tool suite immediately after its
existing Node 24 setup. No SDK profile, Node 20 unit selection, native job seal
or existing test body changed. The workflow validator rejects missing,
comment-only, duplicated, conditional, filtered or reordered invocation and
requires the real path triggers.

The integrated full workflow-control suites pass **1,246 Python tests** in
62.39 seconds. The actual Node 24 event/cache/assertion-structure/profile suites
pass **223 tests** in 1.11 seconds. There are zero failures, errors, skips,
cancellations or TODOs, and zero drift across 836 observed files. These 1,469
component controls include the preceding 63 event controls; overlapping runs
are not additional coverage.

Exact wiring and review are in
`target/first-release-javascript-event-wiring-20260922/`. Its first two overlay
test attempts failed because the copied contract fixture was absent; those
logs remain recorded. The corrected overlay uses the unchanged original
fixture. The canonical integrated commands, logs and source identities are in
`target/first-release-javascript-events-ci-validation-20260922/identity.json`.
This local execution does not claim a completed hosted CI run.

Installed execution, complete runtime/native qualification, signed aggregate
and matching-candidate release qualification remain open.
