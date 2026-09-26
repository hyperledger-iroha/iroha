# Retained ownership and signer integration — 2026-09-26

Work remains on `/Users/takemiyamakoto/devstuff/iroha`, branch `optimizations`.
This is an implementation checkpoint. It is not a frozen candidate
or release qualification. The existing first-release requirements, software
custody policy and production cryptographic/resource ceilings remain in force.

## Integrated source

- Canonical proposal hashing borrows the original signed payload instead of
  deep-cloning the whole block and constructing an unused versioned copy.
  The sole V1 schema, frame header, signature set, resultless payload and hash
  remain authoritative. A follow-up hashes the version/header prefix and
  encoded payload as separate chunks, eliminating the additional full-frame
  copy. Payload serialization and its nested scratch still require funding.
- Retained journals and published Native candidates use checked borrowed
  resultless equality: real fixed-layout counting enforces both archive caps
  before comparing every signature and payload field. Either error rejects;
  two serialization errors can no longer match in the published phase.
  Whole-proposal comparison buffers are removed. Nested equality review found
  no reachable semantic/hash-only shortcut in the current encoded fields.
  Per-instruction equality buffers and numeric serialization scratch remain,
  including BigInt cloning and byte materialization during counting. Complete
  nested resource admission is still open; no allowance is reduced.
- `SignedBlockWire` owns only its canonical framed buffer. Its bare-buffer
  accessor and tuple-transfer API are removed, all repository callers migrate,
  and Kura no longer retains an unread bare copy in encoding scratch. Exact
  nested-codec comparison and tests rejecting bare storage/transport are kept.
  Neither accepted bytes nor wire/resource ceilings change.
- Owning resultless projection clones only the required signatures and proposal
  payload. It no longer clones the entire execution result and then drops that
  copy. It still allocates the required owning proposal; DataModel semantics
  pass locally, Core runtime validation is pending, and no allowance changes.
- Retained validation moves the decoded signed body into its existing candidate
  slot only while the original source/capture owner needs it. Full signed-body
  equality rejects a changed retry. Ready owners release that body before
  marker persistence; panic and successful publication retain the subject
  tombstone. The test asserts a stable nested signature allocation across the
  move, rather than the movable outer Rust value's address.
  Resume also takes the retained body into its consuming frame, so an unwind
  releases it with the execution owner. The reserved subject still refuses
  re-execution. Ordinary refusals restore the same allocation when needed.
- Native preparation moves its frozen source block into the recorder after
  global validation. All recorder callers use that consuming interface. The
  original State/generation checks and source/body authority checks remain.
  The initial source clone and full nested allocation admission remain open.
- Proposal preparation passes Kura's original shared parent block into its
  existing worker. Header/attachment preparation borrows the same owner;
  the worker no longer deep-clones the parent on the control thread. Parent
  validation, snapshot matching, the bounded worker and local refusals remain.
- Kura's finality and retained sidecars decode directly from their capped byte
  slices, preserving canonicality and cumulative Norito decoder limits. The
  removed input copies and proposal clone are removed from the named cold-read
  working-set charge. This does not fund the whole historical signer replay.
- The BFV eight-party roster requires canonical, distinct, nonzero Ed25519 keys.
  The unavailable private-share relation and independent production parameter/
  audit qualification still reject production use.
- Release-manifest role 13 participates in the exact signed Check binding.
  Its executed operation and daemon production source remain closed. Signer
  Check authentication verifies execution against the already authenticated
  lineage body, removing a second read/decode of that body. No key-use,
  signer-purpose or promotion gate opens.
- The X509 one-SHA diagnostic measures a conditional size reduction only; it
  neither removes certificate coverage nor admits an oversized production proof.
- Two unnecessary x86_64 `unsafe` wrappers in vendored `num-bigint` are removed
  for the pinned Rust toolchain; arithmetic is unchanged.

The pre-edit source files and patch identities are preserved in the ignored
`target/first-release-integration-20260926-before` directory. Source edits are
reviewed incremental cuts; F02, F03, F04, F06, F07, F09 and the release gates
remain open.

## Scoped validation and concurrent source change

All five Apple bridge targets built and the ABI-24 XCFramework passed artifact
validation. The matching native fingerprint was
`d7c089dfacd695ae5373321e8484931013ed8674f58b2cf5e6b99a209adc17fe`.
The subsequent focused Swift run passed 57 tests with no failures or skips:
19 Sumeragi wire, 20 Sumeragi Torii, two conviction/native golden and 16 App
Attest tests. A long expected-byte expression was split into ordered appends,
and the status fixture's stale epoch assertion was aligned with Rust's genesis
epoch zero. Both changes are test-only; pre/post native fingerprints matched.

That result belongs to the pre-merge source snapshot. Another task then began
reconciling `origin/optimizations`; this task preserved all eight conflicting
files and their three index stages before reviewing them. The user confirmed
the other task owns merge resolution. This task did not resolve or stage those
conflict files, create a branch/worktree, or make a commit. Current-source SDK
and native qualification must be repeated after integration settles.

The other task completed the merge at `44ac7ef9f1`. The first DataModel test
attempt stopped at a concurrent Rust merge marker. After resolution, the
borrowed proposal-hash selector passed all three tests, including exact
resultless/executed layouts, changed signatures, and attached outputs. The
test dependency graph enabled `transparent_api`; the separate
`--no-default-features --lib` check passed compilation with one existing
`SortitionRequestV1::validate_capacity_intent` dead-code warning. That check is
not a second runtime test result. Logs are
`target/first-release-borrowed-proposal-default-retry-20260926.log` and
`target/first-release-borrowed-proposal-no-default-check-20260926.log`.

A subsequent locked crypto test stopped because a concurrent SoraFS manifest
added `futures-timer` without its lockfile edge. Offline reconciliation added
only that dependency edge to the existing locked package. All seven BFV
eight-party tests passed, including canonical roster authentication, a
correctly signed non-Ed25519 adversary, bounded decode and the retained
private-share/production refusals. The log is
`target/first-release-bfv-eight-party-reconciled-20260926.log`.
The proposal chunk-hash follow-up passed all three equivalence tests in
`target/first-release-borrowed-proposal-chunks-20260926.log`, and the freshly
built crypto binary passed its incremental-hash concatenation control.
The first Core test build stopped at two import errors in concurrently
integrated SoraFS provider-admission and stream-token sources. Narrow import
repairs also remove the two reported unused imports. A retry includes those
repairs and the reviewed shared-parent handoff; its log is
`target/first-release-core-retained-validation-retry-20260926.log`.
That retry encountered a concurrent FASTPQ zeroization edit before its
dependency reached Cargo's resolved inputs. The manifest and lockfile now
both include `zeroize`; the next run is
`target/first-release-core-retained-validation-retry2-20260926.log`.
That build then exposed the new compact replay consumer importing a test-only
fixed zeroizing storage module. The module now compiles for its library
consumer; its test-only slice-copy constructor remains test-only. This is a
module-boundary repair, with no change to FASTPQ admission or qualification.
The next retry, `target/first-release-core-retained-validation-retry3-20260926.log`,
reported a missing `StateReadOnly` import in concurrently added provider-source
authorization and an unused stream-token fixture import. Both are repaired;
that build exited 101 without running tests. The reviewed checked-comparison
and single-buffer changes were then integrated with their formal source
bindings. The first DataModel run,
`target/first-release-comparison-buffer-datamodel-20260926.log`, exited 101 before
running tests: the new SoraFS genesis-admission initializer lacked Norito JSON
traits required by its existing roundtrip test. The missing derives are added;
a concurrent task separately scoped its capacity-record import to its test
helper. The reviewed resultless projection and three nested identity controls
were then integrated. Those controls distinguish account metadata, DA pin
request/signature authorization, and Native recovery hints even when lengths
or narrower semantic identities match. The combined retry passed all 45 tests
with zero failures or ignored tests in
`target/first-release-comparison-buffer-datamodel-retry-20260926.log`.
All 13 selector groups selected at least one test, including seven comparison
and three borrowed-hash tests; all six recorded input hashes remained unchanged.
No compiler warnings appeared. These are local integration results, not a full
candidate source seal. Core retry 4 exited 101 before running tests in
`target/first-release-core-retained-validation-retry4-20260926.log`: the
concurrent X509 cut lacked composite extension evaluators and one test closure
needed an explicit integer type. The closure type and an unused repair-source
test import are repaired; their preimages are preserved under
`target/first-release-core-retry4-small-repairs-20260926`. The X509 owner then
added the composite evaluators and small arithmetic forwarding tests. Core
retry 5 is pending in
`target/first-release-core-retained-validation-retry5-20260926.log`; the
focused runner now also selects both new evaluator tests. The Genesis wire
consumer passed its canonical digest roundtrip (one test, zero failures or
ignored tests), exit 0 in `target/first-release-single-buffer-genesis-20260926.log`.
None of the stopped builds executed the selected Core tests.
Core runtime validation remains pending; static review is not a test pass.
Historical archive verification also passed: 64,736 records and 67,311
occurrences, with the current root files within their 300-line limits.

The prior Python 3.12 readiness session no longer has retrievable terminal
output, so no full-suite pass is claimed from it. The fresh pinned 3.12.14 run passed all 468 tests in
8,787.16 seconds, exit 0, with zero JUnit failures, errors or skips and unchanged
recorded checker/test input hashes. Its durable log, JUnit XML and result are
under `target/sorafs-production-readiness-py312-20260926`. This local evidence
predates the subsequent receipt-identity grammar correction; it does not
qualify an immutable candidate or close production deployment/promotion gates.
The integrated correction aligns exact ASCII identity components with the native
signer contract. Its four focused Python suites passed 546 tests, including
337 new identity cases, with zero failures, errors or skips and unchanged
recorded inputs. Evidence is under
`target/sorafs-receipt-identity-validation-20260926`. The parser and regression
tests are the only changes in that cut; native authorization and promotion
gates are unchanged.

Queue-plan source selectors, adversarial mutations and shared JSON bindings
now follow borrowed proposal hashing and its separate prefix/payload chunks.
The final focused Python 3.12 suite passed 160 controls, including the current
production/ledger binding and reversed/omitted-chunk and copy regressions.
Native preparation source bindings now follow consuming source/body ownership
and resume-unwind cleanup. The full multilane structural checker passed, and
the focused Native suite passed all 118 cases in 3,279.65 seconds. Its nine
scoped source hashes were unchanged after execution. Its fixture authenticates
one source snapshot and gives every mutation independent files and a copied
ledger, with a separate isolation control. This evidence predates the staged
checked resultless-comparison and initial body-custody work.
The separate immutable checked-comparison snapshot then passed all 117 selected
cases in 2,494.87 seconds, including 31 new adversaries; all 1,854 sealed source
hashes remained unchanged. Its result is
`target/f02-resultless-comparison-formal-20260926/result.json`. The later
single-buffer, projection and nested parity additions need their separate
runtime evidence; that snapshot does not qualify them.
After integrating checked comparison and the single-buffer cut, all 15 touched
Rust files pass focused formatting checks and the no-legacy-codec guard passes
again. The current integrated full multilane structural checker also passes
(exit 0; `target/first-release-comparison-buffer-structural-20260926.log`).
This binds the current source/model relations; it is separate from the pending
runtime and mutation checks, required model-checking bounds, and full workspace
formatting gate. The earlier source-file budget check reported 256
findings, with no ceilings or baselines raised. These checks do not replace the required
TLC/Apalache/TLAPS/Verus bounds, four-validator lifecycle matrix, distributed
soaks, proof/hardware parity, independent audits or signed release evidence.

The subsequent direct-projection and nested-parity Rust files pass focused
formatting, and the no-legacy-codec guard passes again in
`target/first-release-resultless-followup-codec-guard-20260926.log`.
The follow-up structural check also passes (exit 0) in
`target/first-release-resultless-followup-structural-20260926.log`.
The DataModel retry passed 45 tests; Core runtime validation remains pending.
