# Complete participant preparation

World, runtime, TriggerSet and carrier preparation now install every original
participant in an inert caller-owned slot before the first physical probe.
Preparation borrows those slots. A late refusal or caught panic cannot destroy a
callee-local acquisition while an enclosing sibling still owns a writer.
Normal recovery returns the exact original journals; unwind and terminal release
retain cleanup only. The carrier releases component writers and its original
State, Queue and Kura fences before payload destruction or release callbacks.

World normal recovery has a separate physical release pass before transferring
original field boxes. Terminal release cannot grant retry authority. An existing
abort-unwind control exposed poisoning of later writers when that first pass was
removed during integration; the restored two-phase recovery preserves all of its
original assertions.

## Captured execution evidence

`dist/sumeragi-main-work/generation156-core/aggregate-preparation-union-verification.json`
joins the final executable candidate:

- All 649 Core controls pass, retaining the previous 643 and adding four
  Runtime/TriggerSet controls and two World/carrier crash-boundary controls.
- The new World control panics after the last actual field prepares, verifies
  that all 278 original field owners remain held after the caught panic, and
  checks that terminal release cannot recover retry authority.
- The new carrier control checks both caught panic and outer unwind after that
  last World preparation. Its actual release callback independently probes 283
  World/runtime/membership owners plus hash, State, Queue and Kura custody. It
  finds no remaining busy owner; assertions run outside the callback.
- All 355 MV controls and all 10 documentation tests pass, including the original
  scoped Prepaid lifetime restrictions with the new cleanup-transfer API.
- Core, MV and documentation builds have no compiler diagnostics and identical
  7,536 Rust/build inputs and Git metadata. Every runtime result joins its
  captured compiler artifact. Core executable SHA-256 is
  `c165c512993172fd527d9782e0aab96099d2f367de230791407051c9c600b68c`.
- The canonical structural gate passes on unchanged 12,274 inputs in
  `generation156-formal/canonical4`, including those same Rust input hashes.
  This is source-binding evidence, not a production trace or network proof.
- Workspace formatting, codec-retirement checks, diff checks and historical
  archive verification pass in their recorded scopes.

The successful candidate is build3. The earlier build1 diagnostic reproduces
its abort-unwind regression; build2's two new crash controls pass but precede
that recovery correction. Those runs remain preserved. After the complete
execution run, one Rustdoc comment was corrected to say that borrowed refusal
retains writers until caller recovery/release. The exact comment-only delta is
recorded in `aggregate-preparation-rustdoc-delta.json`; it changes no executable
Rust. Runtime counts refer to the captured unchanged candidate above.
The subsequent production Core/Torii/daemon library and binary check passes
on 7,536 unchanged inputs. Its independent receipt,
`aggregate-preparation-consumer-verification.json`, verifies that the exact
Rustdoc correction above is its only Rust-input delta from the runtime candidate.
Compiler warnings are retained in that receipt; this check is not warning-free.

## Formal controls and earlier failures

The native preparation inventory preserves all 889 previous selectors and adds
112 controls covering the defining group, World, carrier and normal-recovery
owners. The 194-control union passes: run1 has 168 passes and 26 fixture-location
failures; the corrected 32-control follow-up passes those 26 plus six overlapping
baseline/representative controls. Both 1,693-input source captures stay unchanged.
The test locator now selects the exact reviewed impl before mutating a method
whose body can also occur in a sibling implementation. All executable-relation
and diagnostic assertions remain intact, and all 1,001 selector IDs are retained.
The independent report in
`generation156-formal/world-carrier-preparation/independent-verification.json`
verifies 770 binding/ledger rows and records the authority-fixture, locator-fixture
and comment-only deltas. Run1 remains failed; neither the whole 1,001-case suite
nor one unchanged 194-case rerun is claimed.

The subsequent canonical gate also passes on unchanged 12,275 inputs in
`generation156-formal/canonical5`, including the Rustdoc and authority-fixture
corrections. The later locator-only test correction has its own copied-source
follow-up above; it changes neither the checker nor its ledger.

The earlier immutable 1,292-case direct-State run completed with 1,279 passes,
five failures and eight fixture errors. Three native projection/destructuring
anchors had already been corrected in separately recorded follow-ups. The
remaining lifecycle anchor and manifest test callback now match the actual
borrowed argument and source-root keyword. The copied fixture also includes all
Queue startup owners. All 13 originally failing/error selectors pass on 1,713
unchanged copied inputs in `generation156-formal/direct-state-failures/run1`.
This follow-up does not turn the older whole run into a current-source pass.

## Remaining production boundary

Complete resource admission, pre-acquisition of State effect/index locks, and
the live retained Validate-to-Apply cutover remain open. Membership still inserts
into an unadmitted DashMap during publication; DA/lifecycle effects still acquire
additional locks and allocate inside publication. These need explicit ownership
and funding before production cutover. The unchanged four-/seven-validator
loss, restart and final-transaction campaigns remain required. L1–L6 stay open.
