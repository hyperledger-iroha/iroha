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

Build63 failed at three linker invocations because the system-selected Xcode
launcher required license acceptance. No Rust type error was reported, but this
was not a successful build and no partial runtime selection qualified. Its
source also changed during compilation. Build64 used the already installed
Command Line Tools through a command-local `DEVELOPER_DIR`; no system setting or
license was changed. It completed the combined six-package build in 7m15s.

Build64 passed 1,707 of 1,709 selected runtime controls: 1,228 Core, all 64 Torii
and all 415 data-model block controls passed. All 7,134 captured build inputs,
HEAD/index and emitted binary hashes stayed unchanged through that observation.
The 17 earlier ordinary/batch failures remain explicitly excluded and open, not
counted as passes. Scoped Rustfmt, retired-codec guard, historical archive
verification and unstaged whitespace checks passed. Externally staged vendored
files retained four whitespace diagnostics and were not edited. Full receipts
are in `dist/sumeragi-main-work/validation64.json`.

Two failures prevented a green runtime qualification. The physical Validate
assertion exercised a historical test-only event-transition oracle after
production had switched to the actual retained-owner projection. Its correction
moves the invariant to the real parked/requeued Validate fixture and preserves
the original ordinal, registry, wake, yield, Runtime and Ready restrictions.
The nonzero-view Proposal restart overflowed an ordinary stack. LLDB identified
an 827,152-byte production control-sign startup frame retained through recursive
ledger decoding; the test's own frame was small. Build65's initial extraction of
row repair/readback compiled, but only moved the overflow to registry replay
authentication while retaining an 824,272-byte assembly frame. The broad recovery
selection correctly did not run after that first critical failure. The two
migrated actual-owner tests passed separately. That failed extraction is retained
in the diagnostic artifacts. Build66 then boxed only the completed private
assembly result; it compiled in 3m02s but left an 827,200-byte startup frame and
the same overflow. Both unsuccessful changes are removed from production source.
The correction heap-owns the opaque adapter startup state itself. Its large
adapter and pending continuations no longer move inline through every fallible
recovery join. Borrowed access retains the same owner, and consuming access moves
its exact state once. Build67 compiled in 3m24s and reduced the startup frame to
671,120 bytes, but that frame still overlapped registry installation and the same
critical test overflowed. The broad selection again did not start. LLDB receipts
for each failed attempt remain under `dist/sumeragi-main-work/`.

Build68 separates complete storage preparation from registry installation using a
private move-only prepared cut. The thin outer owner constructor invokes these
phases sequentially, so storage decoding no longer occurs inside the registry
assembly frame. It compiled in 2m52s. The original nonzero-view restart and both
actual Validate-owner regressions passed on unchanged source and binary. Its
expanded 563-test recovery selection passed 550 and failed 13: six additional
control-output census stack overflows, five CompleteTip fixtures missing the
mandatory geometry journal, one wrong-network fixture failing prematurely while
rebinding physical Kura storage, and one obsolete source-contract token. These
failures were not dropped from the next selection.

The next correction separates replay preparation from recursive census assembly
as well, with each private phase consuming the exact prior cut. The CompleteTip
fixture now authenticates the configured primary geometry before genesis
publication. The network-negative fixture constructs a State before physical lane
attachment, preserving the exact Kura and wrong network for the factory's own
rejection check. The source asset follows the pending-Apply owner and binds its
exact Decision/body comparison and publication ordering. All original
authentication/publication steps remain intact; no stack size or validation
assertion is relaxed.

The formal binding migration removed all 84 incoming diagnostics at its initial
full canonical gate. Its broader 779-control run then exposed three ineffective
mutation checks: substring matches admitted substituted provider/reputation
receivers, and a second identical finality call hid substitution of the first.
The checks now bind each full defining branch and original receipt separately,
with the negative controls retained and an independent second-receipt mutation
added. That long run completed naturally with 712 passes, six failures and 61 setup
errors after ten source changes; it cannot qualify the final source. The setup
errors paired an earlier imported checker with a later ledger. Fresh focused
captures cover the corrected owners separately. An external merge
advanced HEAD to `482a02fa70` before build65; the task preserved that checkout.

Build65 passed in 2m52s; `validation65.json` records its critical failure and two
separate actual-owner passes. The fresh 369-control formal selection passed,
including all 179 ingress/capacity controls and paired binding/ledger negatives;
its exact one-file source drift was the subsequent startup boxing correction.
The last old P01 mutation now targets the actual complete-history pending branch,
and P02/C09 diagnostics retain the suite's precise contract prefix. A final
canonical gate and affected-owner capture qualify those changes separately.

Formal66 then passed its canonical gate and all 70 affected controls on unchanged
8,212 inputs and unchanged HEAD/index. This qualifies the final binding repairs,
including the actual pending-history mutation. The subsequent private startup
representation has a separate final gate/owner capture.

Formal67 passed the canonical multilane gate on unchanged captured inputs, but
six of nine supplementary startup/proof/source positives failed. Their 51
reported entries include duplicate reports from two checker layers and point to
older source-owner migrations: retained local Validate, typed status/storage
preflight, descriptor-bound Serve storage, registry includes, factory fixtures
and loop projections. These are separate from the canonical gate and remain
open until their repaired checker/ledger and adversarial controls pass together.
The first repaired durable-Validate scope passed all 36 controls, including four
new owner-substitution negatives; that does not qualify the remaining groups.

Build69 compiled in 2m41s. Eight of the 16 critical controls passed, including
three of the six additional overflows, the factory network negative and the
repaired source asset. Five CompleteTip cases still failed because blank Kura
has no authenticated configured baseline; three retained-output cases still
overflowed. The broad selection did not start; the remaining critical cases ran
separately on the same binary and are recorded in `validation69.json`.

The next fixture uses the actual configured temporary Kura and pre-genesis State
sequence. The production output census now owns its optional pending-Apply
comparison on the heap. The pipeline allocates once from the original sealed
comparison and transfers the same Box into the passive output owner, retaining
all identity, lineage and storage checks. Even the absent comparison no longer
enlarges every ordinary recovery frame. Independent ownership review found no
clone, reconstruction or publication-authority change.

Build70 compiled in 2m44s. All seven observed stack-overflow regressions pass on
the default test stack, along with both actual Validate projections, the exact
factory-network negative and the repaired source assertion (11/16 critical
controls). The remaining five fixture controls now reach the real policy guard:
the newly configured State differs from the convenience State originally used
to sign their genesis context. Build71 derives execution and Nexus context hashes
from that same configured State before signing; the equality guard stays intact.

Formal69 passes the canonical multilane gate and 132/134 selected controls on
8,212 unchanged captured inputs and unchanged HEAD/index. The completed scopes
cover all 36 durable-Validate, 71 construction and 22 inventory controls plus
three nonduplicate startup positives. The two broader successor-source positives
still fail. Following the previously omitted registry include exposes 100 nested
diagnostics, so this is a more complete failure inventory than the earlier 51.
All diagnostics and owner-migration evidence are retained; the broader gate is
not declared complete.

Build71 compiles in 1m43s and passes all 2,094 selected runtime tests: 1,615 Core,
415 data-model and 64 Torii. This includes 563 recovery controls, all 16 critical
regressions and seven observed default-stack overflow cases, plus 1,531 residual
tests from the earlier build64 selection. All 7,134 captured Rust/config/build
and compiled-source-asset inputs, the complete input inventory, HEAD, index and
emitted binary digests remained unchanged from build through execution. The
renamed height-driver regression is explicitly mapped to its already-passed
actual-projection test. The aggregate is sealed in `validation71.json` at
2026-09-20 00:03 JST. These are selected runtime tests, not full crate/workspace
or real-peer qualification; the 17 older Torii failures remain excluded and open.

Three observer-only harness attempts stopped before any residual test: one read
a partially written upstream summary, the second identified the explicit rename,
and the third repeatedly hashed each binary per test instead of per target. Only
the task's own observer was stopped, before spawning tests; no Cargo, rustc,
test or agent process was interrupted. All logs remain and none counts as a
product-test result. The corrected residual run passes all 1,531 tests.

The subsequent formal repair passes 67 focused Serve-storage, status/Decision
and owner-open CAS-chain controls with their semantic negative mutations. Its
intermediate full successor inventory falls from 100 to 80 diagnostics before
the CAS-chain/factory/output updates. This is separate evidence, not a passing
full successor gate. TODO: Record the remaining successor-source repairs and
their unchanged-source full-gate result when complete.

At build71, canonical retry still evaluated current TTL, NTS health, cryptography
and transaction limits before the canonical branch. Build72 separates this
authentication boundary. A private Torii identity proves the original signature,
network and complete entrypoint hash without constructing an AcceptedTransaction.
Both public handlers authenticate and consult canonical custody inside the
original physical compute worker. The peer receiver validates the exact request
and complete journal binding before the original guarded read. Only absent
custody enters all fresh admission policy. The final post-admission race check
uses the existing AcceptedTransaction proof. Sealed reveals retain their complete
outer identity; an enclosed transaction cannot claim the reveal's receipt.

The six-package build72 passes in 2m13s, and all 67 selected Torii controls pass
on unchanged 7,135 Rust/config/build/compiled-asset inputs, HEAD, index and binary.
The three new controls exercise both public handlers and the peer receiver across
expired TTL, disabled Ed25519 admission, reduced byte limits, absent valid inputs,
forged signatures with unchanged intent hashes, wrong networks and substituted
sealed reveals. Original certificate bytes, State custody, empty Queue and local
journal bytes remain unchanged. This is separate evidence from build71's 2,094
runtime tests; it does not qualify the full workspace or a live peer campaign.

All 80 canonical retry controls, including exact ledger mutations, pass. The
first full gate then exposes three duplicated admission-capacity declarations
still naming the former accepted-input binding; the unchanged-source failure is
retained. Updating that independent checker to the same original input and
authenticated identity passes all 55 capacity controls. The initial focused
source check also retains its trailing-comma normalization failure and corrected
77/77 pass. Final capture72b then passes the complete canonical multilane
structural gate and all 135 controls (80 canonical-retry/ledger and 55 capacity)
on 8,358 unchanged captured inputs, HEAD and index. The corrected admission
capacity checker retains all original framing, capacity and publication guards.
`authentication72b-formal/summary.json` records this separate formal epoch.
The last completed supplementary successor inventory has 67 diagnostics; later
focused terminal (25), chunk-signing (13) and armed-output (11) controls pass,
but no complete successor proof pass is claimed.

The 17 older handler fixtures still need migration to the current first-release
admission contract.
The generic batch endpoint itself permits ordinary application batches while
rejecting QueuePlanSynced and bypassing the threshold-key lifecycle exception
used by single submission. Resolve that production admission inconsistency before
treating the batch success expectation as the desired contract.
Exact terminal Queue cleanup, off-chain receipt carry/closure, complete capture
admission, original prepared Validate-to-Apply ownership and the production shared
lane reducer/publication cutover remain open. Full workspace and unchanged real
four/seven-validator fault/restart/final-transaction qualification remain open.
No liveness goal is complete.
