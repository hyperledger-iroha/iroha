# Current optimizations validation and SDK input ownership

Work is confined to `/Users/takemiyamakoto/devstuff/iroha` on `optimizations`.
Observed base: `33df4724ecd976ac667029435bde69c2517cdeb0`. This is development
validation; no immutable candidate or release gate is closed. The previous
integration-branch instruction is superseded. First-release compatibility paths
and an HSM prerequisite remain prohibited.

## Runtime source reconciliation

The current Storage implementation uses the original current/undo trees and
transaction-local `SortedTouches`. Its paired reservation includes touch growth
and key copies before mutation. The earlier target-only three-map proposal is
obsolete and was not imported. Native mutex/runtime funding, closed model-payload
admission and complete World/State reservation remain open.

The full current MV selection passes **304 tests, zero failures or ignores**.
The receipt is `target/first-release-optimizations-mv-validation-20260921/identity.json`;
it retains transitive source inputs, six binary identities and exact commands.
Observed inputs remained unchanged. The old MV borrow-check failures are resolved
in this source, not accepted or hidden by changing the test command.

Fresh Core test compilation passes in 626.32 seconds with the retained features
`iroha-core-tests,zk-tests,halo2-dev-tests,zk-halo2-ipa`. All eight Time/predecessor
controls and fourteen event-owner controls pass. The former precommit capacity
failure no longer occurs. The first output-owner selector collected zero tests
and failed; it is not a pass. A continuation over the same source and binary
resolved all 55 declared output tests to their actual names: **54 passed, one
failed**, with no skips or input drift. Receipts, logs and the original selector
failure remain in `target/first-release-optimizations-core-owners-20260921/`.

The remaining periodic fixture tried to install a fresh State primary over its
already authenticated configured Kura. It now uses the fallible State constructor
already used by committed-proof readers, configures test execution defaults and
asserts the original stored header bytes remain unchanged. The three distinct
periodic invocations, action hashes, completion bindings, effects, work and
rollback assertions are preserved. Current-source rerun is retained separately
under `target/first-release-optimizations-core-owners-20260921-2/`. Recompilation
passes in 117.88 seconds; all **103 selected controls pass** (eight Time,
fourteen event and the complete 81-test output-producer module), with zero
failures, ignores or observed source drift. Ordinary worker stacks are retained.
Subsequent workspace formatting changes only SCCP test wrapping, schema test
module order and xtask test wrapping; these are outside the repaired fixture.
`cargo fmt --all -- --check`, the retired-codec guard and `git diff --check` pass
after that formatting cleanup. Final candidate validation remains separate.

The current structural formal-ledger diagnostic exits 1 with **547 error lines,
531 distinct**, in `target/first-release-optimizations-formal-ledger-20260921.log`.
Failures include reviewed seals, source shapes, lifecycle/application custody,
retirement and replay/publication contracts. This run is not TLC, Apalache,
TLAPS or Verus qualification. Each implementation/model mismatch needs semantic
review; blindly replacing expected hashes would not resolve these obligations.

The shared TLC-result checker now separates the newly added action-property
helper from fixed-success and validates each helper's guards independently.
Historical SHA-matched preimages prove that the original seven helper bodies
and both reviewed runner execution functions are unchanged. The candidate
runner's retired standalone-manifest case corresponds to the removed model and
configuration; all 40 current mutant calls remain. The in-flight runner retains
its original 22 mutants plus three added cases. Their exact branch inventories
and retained-case count are checked independently of the refreshed source seals.
The integrated selection passes **154 controls** in 30.54 seconds (88 existing
shared-result tests and 66 new structural/runtime controls; 6,122 unrelated
tests deselected). Synthetic log controls do not constitute a TLC model run.
Review and exact pre/postimages are in
`target/first-release-current-tlc-contract-review/`; integrated output is
`target/first-release-current-tlc-integrated.log`. Other formal owners remain open.
The release preflight and its authenticated receipt now register the new test
file exactly once. Actual collection found 6,729 cases in the prior 24 selectors,
557 more than the stale declared count; adding the 66 new controls gives
**6,795 unique cases across 25 selectors**, with every prior node preserved.
All 29 focused registration and negative controls pass against the integrated
sources. Exact node inventories, source hashes and the unchanged surrounding
component pins are retained in `target/first-release-current-tlc-contract-registration/`;
the integrated log is `target/first-release-current-tlc-registration-integrated.log`.
Collection and simulated child-output controls do not assert that the complete
6,795-case corpus passes. Full-runner syntax checking also remains unavailable
with this host's Bash 3.2, which rejects preexisting descriptor syntax; the
changed preflight fragment executes in the focused controls.

The current source-file budget diagnostic reports **277 findings** in
`target/first-release-current-source-budget.log`. This release guard remains
failing; no file-size limit or oversized baseline was relaxed.

## Snapshot hash-journal resource implementation

Snapshot-tail validation now hashes the exact durable prefix through the
original `FileWrap` using 4 KiB of fixed scratch. It no longer materializes the
full prefix or a second slice inventory solely for the digest. The in-memory
snapshot slice also hashes without the auxiliary slice list. The existing
domain, little-endian height, canonical hash bytes and digest finalization stay
identical. Read-only rejection retains marker/journal evidence; the repairing
consumer retains its existing deletion conditions, with I/O refusal propagated.

The shared range check rejects offset, length and end overflow before seeking
or allocating. Previously, `start = u64::MAX / 32 + 1` could overflow or wrap to
the first hash. The implementation lives in
`crates/iroha_core/src/kura/snapshot_hash_journal.rs`; focused controls are in
`crates/iroha_core/src/kura/tests/18_snapshot_hash_streaming.rs`.
Native compilation and scoped execution are tracked under
`target/first-release-kura-streaming-validation-20260921/`. The first build
passes in 172.08 seconds and all **21 selected controls pass**, with zero
failures, skips or source drift. It reported an unused test import, subsequently
removed. The final rebuild and expanded existing-marker/roundtrip selection are
tracked separately under `target/first-release-kura-streaming-validation-20260921-2/`;
the rebuild passes without warnings, and all **24 selected tests pass**, with
zero failures, ignores or source drift. The final workspace format check,
retired-codec guard, changed-shell syntax checks and `git diff --check` pass.
Read-only review found no correctness defect. Concurrent corruption and
truncation within one chunk can change which
path-bearing I/O error appears first; both failures retain evidence. Deterministic
mid-read truncation injection is not covered by these tests. Whole startup-history
funding and Native cutover remain open.

## Lane-reservation compaction recovery

Strict Queue startup previously checked an interrupted compaction temporary's
metadata length, then allocated a second whole-file vector and called unbounded
`read_to_end`. A same-inode writer could grow that file beyond the admitted
length before the final rejection. Recovery now compares the original retained
handle against the authenticated expected prefix with fixed 4 KiB stack scratch,
consuming at most the captured length plus one growth-probe byte. Original
single-link, handle/path identity, final-length and pre-removal checks remain.
The comparison does not open, seek or delete files. It propagates actual body
and probe I/O errors even after observing a content mismatch.

Truncation after metadata capture now returns `UnexpectedEof` directly; the
previous path could return a later `InvalidData` length mismatch. Both refuse
startup before cleanup. The canonical expected encoding and decoded replay
still retain their existing allocations; this change does not complete their
resource funding or the Native cutover.

The review and original source identities are retained in
`target/first-release-next-runtime-admission-20260921/`. Thirteen focused controls
cover scratch boundaries, partial reads, oversize admission before reads,
interrupted reads, real same-inode growth/truncation and exact growth-probe bounds.
Two further controls exercise startup with explicit 64 KiB frame, 128 KiB file
and 32-owner limits: all authentic empty/chunk-boundary/full prefixes recover
without changing the canonical journal, while a hardlinked authentic temporary
retains both links and the canonical bytes. All 66 existing reservation-journal
cases remain selected.

The initial build failed the workspace enum-size lint in a new test helper;
that helper now retains the original `io::Error` object. No source drift was
observed. The corrected scoped build and execution are tracked in
`target/first-release-queue-compaction-validation-20260921-2/`: the corrected
locked offline build passes in 152.16 seconds, and all **81 tests pass** in
76.49 seconds, with zero failures, ignores or source drift and no worker-stack
override. This includes every original 66-case reservation-journal test.
Workspace formatting, the retired-codec guard and `git diff --check` pass.
The original failed build remains in
`target/first-release-queue-compaction-validation-20260921/`.

## SoraFS SDK evidence implementation

The [package-index specification](../../../specs/sorafs/reference_sdk_package_index.md)
defines the closed six-consumer file graph and its independently opened physical
input owners. Its Java adapter opens original packages, dependency/JDK/native
inputs and retained observations, checks exact Kotlin distribution membership,
and rederives the two 25-case execution records. Runner and probe origins join
their actual compiled owners. FIFO leaves reject before blocking, paths retain
their original descriptors, and completion checks both content and ancestors.
The mandatory dependency projection joins each process's actual JDK 21 effective
JAR classes, including multi-release overrides, to its original runtime origins.
It requires the JUnit engine/launcher and rejects foreign, missing, repeated or
unowned generated classes. Execution and Android probe observations are separate.

Component tests use explicitly synthetic classes and mocked process/native
boundaries where stated. Those tests validate rejection and ownership logic;
they are not successful SDK executions or release evidence. The integrated
selection passes **486 tests, zero failures/errors/skips**, including the index,
Java producer/adapter/dependency controls, signed manifest and native artifact
checker tests. Exact source inputs remained unchanged; commands, hashes and JUnit
results are in `target/first-release-sdk-index-integrated-20260921/identity.json`.
The three package component suites now run in the release CI gate; changes to
their owners, Java runners and tests trigger the release workflow. The focused
automation suite passes **1,167 tests** in 57.84 seconds; its output is retained
in `target/first-release-sdk-index-ci.log`. Shell syntax and the final retired-codec
guard also pass. This verifies CI wiring, not the complete release gate.
The automation validator also accepts the three current release workflows.
The separate actual JDK/JUnit parser diagnostic uses synthetic test bodies and
therefore supplies no SDK/native qualification.

The Python native CI lane authenticated its private wheels but did not request
installed-package mode for pytest. Its conftest could therefore select checkout
SDK/native source instead. The child now explicitly sets the existing
installed-package test mode, which validates private-environment import owners;
the before/after wheel authentication and all eight test-file selections remain.
The workflow/dispatch suite passes **42 tests**, including actual shell child
dispatch with unset and hostile inherited modes, and negative controls removing
the binding. The evidence shell contract and syntax checks also pass. Logs are
`target/first-release-python-installed-owner.log` and
`target/first-release-python-evidence-shell-contract.log`. These component tests
do not rerun the real native wheel build or produce retained wheel-execution
evidence. The release-evidence runner's stale hardware-receipt description now
names the existing custody/completed-operation receipt contract; no HSM check or
authorization requirement was substituted.

The existing Python wheel verifier now exposes the same archive/RECORD parser
for immutable captured bytes. A content-only `WheelArchive` cannot claim a path,
seal, installation or execution; `WheelPreflight` retains those original file
checks before delegating to that parser. The parsing body and existing installed
module/loader checks are preserved. The focused test executes the exact original
inline fixture harness unchanged and then verifies **76 byte/path parity pairs**,
including malformed ZIP/RECORD/owner/suffix cases and exact resource boundaries.
Its two pytest tests also check source extraction, no file/import access from the
byte parser, immutable input and seal-before-parser ordering. The oversized
original shell harness is unchanged.

These controls and their three source triggers are registered in release CI.
The integrated wheel, release-smoke contract and automation selection passes
**1,179 tests** in 60.40 seconds; the automation validator accepts all three
workflows. Output is `target/first-release-python-wheel-integrated.log`, and exact
parser pre/postimages are in
`target/first-release-python-wheel-byte-owner/revision-2/`. These are fixture and
component controls, not actual wheel installation or native execution. A concrete
Python execution producer and consuming adapter still need implementation.

The fixed POSIX Python child and its closed immutable report parser are now
integrated. The child uses the existing wheel verifier before imports and retains
original installed files and loaded module/spec/loader owners through the exact
77 reference cases and all 231 setup/call/teardown observations. It rechecks the
original native module after the deliberate monkeypatch teardown. Its entire
source snapshot, including all ancestor directory identities, is checked before
imports and after tests. Unexpected package initializers, sibling files, foreign
empty directories and substituted directories reject. An earlier unapplied
packet omitted those ancestor paths; the corrected revision remains the sole
implementation.

The parser reuses the canonical wheel `FileSeal.parse`, returns frozen
observations, and requires exact module-name/member/source joins. It checks all
actual log bytes preceding one canonical report frame. Logs are bounded to
32 MiB and report JSON to 8 MiB; the combined envelope includes Base64 expansion.
Regular-to-FIFO races refuse through nonblocking, no-follow opens. These tooling
limits do not change production proof or runtime ceilings. The coupled contract
is [Python reference child V1](../../../specs/sorafs/python_reference_child_v1.md).

The revised child packet is
`target/first-release-python-wheel-child/revision-2/`; the report helper packet is
`target/first-release-python-execution-contract-review-20260921/`. Their 46 and
108 focused controls are synthetic observation/ownership tests. The real pytest
mechanics control retains exact source decorators and parameter IDs but replaces
bodies only in its separate synthetic module; the original wheel harness uses
an inert extension. No canonical native assertion's result is claimed by these
controls. Original reference assertions, wheel verification and the large shell
harness remain unchanged.

Both suites and their source triggers are registered in release CI. The combined
current-source child, report, wheel-parser, release-smoke and automation suite
passes **1,333 tests, zero failures/errors/skips**, in 62.21 seconds with no
observed input drift. Logs, JUnit and source identities are in
`target/first-release-python-child-validation-20260921-2/`. The earlier equal-count
run is preserved separately: its assertions passed, but a concurrent POSIX-scope
documentation clarification caused the source-observation guard to reject it;
it is not an unchanged-source result. The release automation checker accepts
all three workflows and changed-shell syntax/diff checks pass.

The subsequent reservation-journal file budget is ratcheted down from 5,323 to
5,190 lines after the reviewed extraction. No source limit was raised. Its focused
budget suite passes **51 tests**; the full guard still reports **276 findings**.
The controls and diagnostic are retained under
`target/first-release-source-budget-controls.log` and
`target/first-release-source-budget-after-queue-ratchet.*`; the broader source
budget guard remains open. The actual child CLI also passes help and malformed
closed-input refusal smokes without emitting a report or invoking native code
(`target/first-release-python-child-cli-smoke/`). This metadata-only tightening follows the scoped
Python source observation; it is not an immutable whole-candidate seal.

TODO: implement the parent execution producer and original-index Python adapter,
including original SDK/native wheel and native-manifest joins, full
CPython/stdlib and dependency artifact custody, owned bounded process execution,
retained outputs and independently authenticated candidate/producer approvals.
Actual unchanged native-reference execution and supported-platform qualification
remain open. The report parser alone cannot authenticate a process or promote a
release.

The other five concrete adapters and the atomic signed-aggregate/SF11 cutover
remain open. The new component does not authorize promotion, replay native ABI
probes, establish complete JDK provenance, qualify Android devices, or replace
independent signatures and actual matching-candidate package executions.

## Executing Cell pair ownership

`mv::Cell` now keeps both acquired current/undo writers inside one private
consuming owner through ordinary/replacement execution, same-cut replacement,
capture refusal and abandonment. Both physical locks release before either native
notification runs; physical poison verdicts are captured before arbitrary wake
callbacks. Capture moves the original allocations together. Publication and the
existing JSON representation retain their original semantics. No second execution
or codec path was added.

The exact-preimage packet is
`target/first-release-cell-writer-owner-20260921/implementation.patch`; its caller
and limitation map is `INTEGRATION-REVIEW.md` in that directory. Fresh locked,
offline validation passes all **310 MV tests**, preserving all prior 304 names,
with no failures, ignored tests, source drift or binary replacement. The build
took 4.09 seconds. Source observations, retained binaries and logs are in
`target/first-release-cell-pair-validation-20260921/`.

A current Core rebuild took 240.34 seconds and passes **10 actual runtime-journal
regressions** in 27.09 seconds, with zero failures/ignored tests/source drift and
matching binary hashes before/after execution. These exercise ordinary and
replacement capture, exact allocation addresses through abort/retry, busy and
changed writers, installation refusal, static worker handoff and prepared cleanup.
The receipt is `target/first-release-cell-core-validation-20260921/`. Ordinary
worker stacks were used. Workspace formatting and the retired-codec guard pass.

TODO: the pair owner begins after both acquisitions succeed. Partial second-writer
acquisition, allocation-refund callbacks and aggregate Runtime/World/State
acquisition/capture/disposal still require joined ownership. This change does not
fund the production retained validator or activate Native lane consensus. F02,
F03 and all fourteen overall release goals remain open.

## Python producer original-input custody and publication

The canonical `scripts/build_sorafs_python_consumer_artifact.py` now owns the
fixed Python child's actual process execution. It requires a clean source commit,
an independently selected workspace-source digest, original ABI-23 manifest and
native/SDK wheels, and separately pinned CPython/runtime/dependency manifests.
There is no dirty-source override or caller-supplied successful report. The
[producer contract](../../../specs/sorafs/python_consumer_producer_v1.md) records
its exact POSIX host scope and remaining adapter/qualification obligations.

The integrated owners retain primary input descriptors and directory ancestry;
scan complete bounded source, installed environment and configured stdlib trees;
join both wheel payloads to the exact candidate package recipes and sources; and
join all fifteen dependencies to original archives and the full installed
inventory before an installed interpreter starts. The sole native/SDK wheel
parser and installed-file verifier remain authoritative for those two packages.
The original native payload must match the original native manifest. The nested
ABI probe now explicitly uses `-I -B`: isolated mode ignores the environment's
bytecode setting, so the previous invocation could create cache files inside
otherwise immutable inputs. An actual inert subprocess regression verifies the
flags and absence of generated cache without claiming native qualification.

Process execution retains bounded separate stdout/stderr and exact exit status.
The parent consumes the final fixed report against the original 77-case source,
all 231 phases and actual preceding pipe bytes. A separately retained runtime
bundle carries executable, shared-runtime and stdlib inputs. Its original staged
handle is read back and parsed against the independently pinned manifest. The
execution ZIP carries complete installed bytes, sources, metadata, logs and the
actual manifest. All final input, runtime, source and environment checks occur
while their owners remain live, before either completed artifact name exists.
No-replace publication links the runtime bundle first and execution ZIP last;
failure rolls back only this attempt's own links. Cleanup closes handles without
performing a late check after success. This is not signed approval or a claim of
atomic power-loss durability.

The original wheel parser also now accepts a valid explicit package-root
ZIP directory without indexing a nonexistent second path segment. It still
rejects reserved module directories. The unchanged original wheel harness and
**80 original-byte/path pairs** exercise that correction; the large original
shell harness was not rewritten.

Current-source combined validation passes **1,606 tests** in 67.80 seconds
(68.27-second driver), with zero failures, errors, skips or source drift. The
receipt, command, JUnit and input hashes are in
`target/first-release-python-parent-validation-20260921/`. It includes the
producer/environment/process/custody/runtime/package/dependency/publication,
child/report, native probe, wheel, smoke and release-CI component controls.
Five additional runtime-bundle/publication composition controls and two actual
CI-registration checks pass after their final registration (**7 tests**, 1.24
seconds), in `target/first-release-python-runtime-publication-join-root.log`.
The additional composition tests retain the original runtime fixture; they test
semantic bundle refusals and late original-runtime changes before publication.
The release workflow and strict release script include all new suites and paths.
Automation validation, shell syntax and `git diff --check` pass.

An actual offline installation control in
`target/first-release-python-offline-install-controls/run-1/` consumes all
fifteen downloaded real wheels plus the two previously built host SDK wheels,
using the production process owner and hash-required offline pip bootstrap. Both
actual subprocesses exit zero, with empty installation stderr. Static native/SDK
verification authenticates **10 + 60 files**; dependency verification joins
**1,302 rows**, covering the complete **1,380-file**, **85,313,443-byte**
environment together with its original bootstrap files. Source, original wheels,
interpreter and final environment remain unchanged. No process starts after
installation, no historical native payload is loaded, and no qualification child
runs. Those dependency pins were selected for this component control; they are
not independently approved release toolchain inputs. The original execution
script, logs, identities and scope are in its `STATIC_ORIGIN_REVIEW.md`.

Earlier component diagnostics are retained: the initial distribution probe
returned a top-level list where the closed decoder required an object; after
correction, the same environment/process selection passes 62 tests. The final
combined run supersedes those narrower controls. The source-budget guard still
reports **276 findings**; none concern these new producer files and no ceiling
was raised. Full source-budget closure remains required.

TODO: implement the original-index Python adapter, consume actual original
wheel/runtime/dependency/native inputs from that index, and join authenticated
producer/operator approval in the signed aggregate. The final native/SDK packages,
77 native-backed cases, all platforms/hardware, independent review and release
qualification still require matching-candidate execution. The dirty checkout is
not a release candidate. F12, SF11 and all fourteen overall goals remain open.

## Partial Cell acquisition cleanup

The follow-on change to `Cell::acquire_charged_writers` moves the original undo
writer into the second acquisition's unwind scope. A second clone panic or
already-poisoned current writer now releases the raw current acquisition and
original undo writer before either native wake callback. The successful path
returns the same `CellWriters`; no new guard, compatibility path, clone or
publication identity is introduced.

All **315 MV tests pass**, preserving all 310 prior cases. The locked/offline
build takes 4.11 seconds; source inputs and executed binaries remain unchanged,
with zero failed or ignored tests. Five new tests cover all three ordinary,
replacement and same-cut entry points, actual first/second clone failure,
original pointers/publication identity, real poisoned acquisition, completed-pair
notification timing and the actual outer-layout charge refund/retention policy.
The receipt is `target/first-release-cell-partial-validation-20260921/` and the
source/caller review is `target/first-release-cell-partial-acquisition-20260921/`.

A fresh Core feature-profile build passes in **186.14 seconds**, followed by all
**10 runtime-journal regressions** in **26.94 seconds** on ordinary worker stacks.
Source observations and private executable hashes match before/after execution,
with no failures or ignored tests. The exact selections and logs are in
`target/first-release-cell-partial-core-validation-20260921/`. Workspace formatting
passes after this patch.

This closes the identified native wake-order gap in partial Cell acquisition.
TODO: arbitrary allocation-charge refund callbacks and aggregate Runtime/World/
State acquisition, disposal and funding still need their enclosing ownership.
A failed clone retains its original conservative charge; this test does not
claim that outer-layout charges fund nested payloads. The production retained
validator, Native cutover and all overall release goals remain open.

## Python captured-content and bounded-parser integration

The sole wheel verifier now owns one installed-byte relation used by both live
file custody and captured artifact replay. Content results contain no physical
file seals or execution authority. The dependency verifier consumes those exact
content results; no duplicate RECORD/direct-URL algorithm or compatibility entry
point remains. The execution ZIP codec and nine-operation command catalog also
have one shared owner for production and subsequent artifact replay.

All **1,814 combined Python tests pass**, with zero failures, errors, skips or
source drift, in 81.03 seconds including driver overhead. Exact source identities,
commands and results are in
`target/first-release-python-parent-validation-20260921-2/`; source inventory
SHA-256 is `02ad6353bda600257f6a9f07b339067678bb6d5ce1b1dd2117ecf19855fd80e0`.
This supersedes the earlier 1,606-case component run. The original wheel harness
and all 80 byte-versus-path comparisons remain intact.

The archive preflight walks fixed headers before all three untrusted `ZipFile`
constructors, bounding actual directory records and raw names before allocation.
It rejects a reproduced ZIP64 locator in a permitted member comment that caused
a classic one-member directory to allocate 66 entries. Existing wheel member
extra/comment coverage remains accepted. The reviewed patch and reproduction are
in `target/first-release-python-archive-preflight-20260921/` and
`target/first-release-python-zip-admission-review/`. These are finite parser
bounds, not aggregate allocation funding or maximum-host qualification.

Native ABI and symbol-output subprocesses now have bounded stdout/stderr and
deadlines. The applied revision preserves the outer process group so its timeout
also contains the nested probe. The first draft created an escaping inner session;
it was rejected before integration and its reproduction remains in
`target/first-release-native-bounded-probe-review-20260921/`. Revision 2 includes
real nested timeout/cancellation and TERM-ignore controls. Windows API controls
are synthetic; actual Windows execution remains open.

A current-source static replay again matches all 15 original dependency wheels,
1,302 dependency rows, both native/SDK inventories (10 + 60 files), and the full
1,380-file earlier offline installation. No subprocess or native/SDK import runs
during this replay. Results and unchanged source hashes are in
`target/first-release-python-installed-content-current-20260921/evidence/`.
Those historical component artifacts and locally chosen pins do not qualify the
release candidate. The initial archive test failure, where `ZipInfo` sanitized a
NUL before fixture encoding, is preserved; the corrected test mutates actual ZIP
headers and passes.

Automation validation, changed shell syntax, retired codec guard and
`git diff --check` pass. Workspace formatting already passed after the final Cell
patch. The source-budget guard still reports **276 findings**; no ceiling was
raised. TODO: implement the consuming original-index Python adapter and the
signed aggregate joins, then execute final native-backed cases and required
platforms against the matching immutable candidate. All overall goals stay open.
