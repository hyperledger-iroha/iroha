# Kotodama V1 redesign implementation and acceptance

This is the execution ledger for the approved first-release syntax and usability
redesign. The canonical rules live in [the grammar](kotodama_grammar.md),
[numeric V1](kotodama_numeric_v1.md), and the IVM ABI schema/syscall sources.
Only the final ABI V1 design ships: no compatibility parser, call aliases,
edition switch, or alternative retired runtime path. Japanese branding remains
`seiyaku`/`誓約`, `kotoage`/`言挙げ`, `hajimari`/`始まり`, and `kaizen`/`改善`.

## Active implementation milestones

| Milestone | Implementation | Required closure evidence |
| --- | --- | --- |
| Declaration and value semantics | Explicit named/positional modes, named struct patterns, composable Unit, nominal error descriptors and imported value types are implemented. | Fresh compiler tests: signature-only label changes, source-order argument evaluation, one initializer evaluation, field reordering, exhaustive matches, exact package identities. |
| Collections and arithmetic | Checked/fallible list mutation, Result obligations, fused rounding, cursor types, bounded list loops, STATE_SCAN and seek-based hosts are implemented. Fresh Core contract/codec/scan selections pass 60/60. | Four-validator execution; compiler/VM/Core tests cover rollback, unchanged fallible mutation, no implicit Result loss, rounding modes, representable fused results with large intermediates, pagination bounds and gas. |
| Shared boundaries and rejection | Unit/error schemas, signed descriptors, exact abort propagation, explicit rejection selectors and maintained SDK record consumers are implemented. Cursor schema propagation is implemented. | State/argument/return/nested-call roundtrips and malformed identity/schema/code rejection on the final ABI; wrong-stage test failures and restoration. |
| Authoring and onboarding | Immutable editor snapshot, semantic LSP, rich diagnostics, packaged editor client and staged standalone-test validation are implemented. Scaffold publication handling passes ten fresh tests; corrected tutorial builds and link checks pass with the documented 12 GiB build heap. | Fresh editor, CLI/LSP, offline-project and updated tutorial checks; network invocation remains part of release qualification. |
| Final release assets and qualification | Only final V1 artifacts are retained. Current compiler/editor source changes require fresh generator and installed-asset verification. Qualification C9 remains open. | Fresh installed-artifact admission/runtime checks, combined Core/native SDK consumers, four-validator integration, then workspace validation. |

No milestone is closed by source implementation or by a test result from an
earlier schema revision. Cargo.lock and unrelated worktree/index changes are
preserved. Tests requiring node consensus use four validators and mandatory
signed RS16 availability.

## Latest compiler qualification

All 1,090 compiler library tests pass on the current candidate at
`target/kotodama-v1-qualification/20260907T083242Z-compiler-lib-3d7f16def1/`.
There are no failures or skips; source, Cargo.lock, HEAD and staged contents are
unchanged during the run. The new nominal-identity test permutes package, module
and import ordering eight ways and compares complete error catalogs, separate
variant schemas and embedded public return schemas. The DEX payout vectors and
both-tag Result if-let execution pass in the fresh 50-test VM capture below;
the cursor authorization case passes in the Core qualification captures.

Compiler integration now passes 77/77 in
`20260907T083348Z-compiler-integration-73e361ac61`. Its preceding 74/77 run
exposed one stale Json::parse label fixture and two zero-cost if-let regressions.
The fixture now accepts only the positional constructor and retains exact
unknown/duplicate-label, arity, direct-literal and source-span checks. Shared
statement/expression lowering branches directly on canonical Option/Result tags,
swapping branch targets for tag zero, while nominal errors retain exact code
comparison. Initializers still run once and payloads bind only in the selected
branch. The original exact IR and executable-byte comparisons pass unchanged.
The fresh compiler library run includes this production correction.

Koto/LSP passes 37/37 in `20260907T083423Z-koto-tests-6520640c56`, including
exact Japanese notes, help and related labels with UTF-16 locations. The editor
acceptance mapping binds its fifteen requirements to 27 compiler and 22 Koto
cases actually executed in these fresh suites. Local debug initially passes 8/9
in `20260907T083512Z-cli-debug-c0ab7fdeba`: one fixture incorrectly expected
Unicode normalization before duplicate detection. Its test-only correction
checks canonical NFC key/byte readback, rejects decomposed and mixed spellings
with the exact StatePath error, and checks identical raw keys fail with Norito
DuplicateField at the JSON stage. Production remains strict. The corrected
nine-test selection passes 9/9 in `20260907T084730Z-cli-debug-1dcfefb112`,
with exact membership, zero ignored cases and unchanged source, index and lock.

## Offline first-project qualification

Fresh normal CLI and Koto builds pass in
`20260907T084954Z-build-cli-3a5776b482`; the fixture exporter build also passes
in `20260907T090404Z-build-fixture-export-34aedeb808`. Both captures preserve
source, index, HEAD and Cargo.lock and retain their exact executables.

The complete generated-project workflow passes in
`20260907T090456Z-first-project-39f2ad4f13`, with IP networking denied. The
published project has exactly eight authoring files, including both manifests,
editor settings, the branded stateful contract and standalone tests. Actual
check, build, locked build, Koto project check, format and schema commands pass.
All three discovered standalone cases execute successfully: `adds_to_total`,
`rejects_zero_amount` and `requires_invocation_permission`. Reusing the nonempty
destination fails with the expected reason and preserves its complete inventory.
No runtime credentials or network side effects are generated. The complete
workflow, frozen-binary binding and unchanged-source checks are retained in its
qualification and acceptance-facts records. Native SDK, final artifact admission,
four-validator execution/restart and workspace qualification remain open.

## Latest runtime qualification — 2026-09-07

The Core completion/recovery selection now passes **107/107**, with zero ignored
cases, in `20260907T071604Z-core-retirement-qualification-c171433b75`. Both
actual cold-restart fixtures complete their exact queued worker through the
production coordinator publication and acknowledgement before teardown. Their
fixture cleanup retains the validated carrier, exact completion ordinal, empty
queue census and open output guard; it does not forge a signing result. All five
new recovery regressions, the 21 retained boundary cases and the cursor
permission revoke/restore test pass. This supersedes the earlier 105/107 result
below without erasing its two fixture-teardown failures.

Additional fresh selections pass with no ignored tests:

| Selection | Passed | Capture |
| --- | ---: | --- |
| VM runtime, lists and fused arithmetic after the if-let correction | 50 | `20260907T084222Z-ivm-runtime-lists-8a6d2f203b` |
| ABI policy and contract artifact acceptance | 72 | `20260907T082416Z-ivm-abi-artifacts-d1bd447fdb` |
| VM values, bounded state scan and exact test driver | 57 | `20260907T072722Z-ivm-values-driver-0ec105bba7` |
| Numeric syscalls, shared fixtures and pointer policy | 40 | `20260907T073728Z-ivm-numeric-pointers-73fe8d7b9c` |
| Generated syscall documentation | 1 | `20260907T074228Z-ivm-syscall-docs-7671aab125` |
| Data-model contract values and exact executor rejection | 57 | `20260907T074312Z-model-contracts-4880d59fc1` |
| Core execution header binding | 1 | `20260907T075254Z-core-header-ce420c0aa0` |
| Core manifest ABI acceptance/rejection | 6 | `20260907T075832Z-core-manifest-d0d963af07` |
| Torii public nominal state, argument products and cursors | 5 | `20260907T080014Z-torii-contract-state-8490f9c6b3` |

These receipts preserve the complete source inventory, HEAD, staged contents and
Cargo.lock during each command. The ABI value selection separately passes
172/172 as recorded below. Their scopes do not replace final native, generated
artifact, four-validator or workspace execution.

The VM runtime/list selection initially passes 49/50; its added DEX vector
fails before execution because a hand-formatted JSON record is not in canonical
key order. It now uses `Json::try_new(norito::json!(...))`, retaining all input
precision, payout, runtime and constant-folding assertions. The ABI/artifact
selection initially passes 70/72; two stale `range` metadata fixtures now use
`page`, check both `page` and `take`, and explicitly reject otherwise-valid
retired `range` metadata. The existing LSP diagnostic test additionally asserts
exact Japanese note, help and related-location messages alongside UTF-16 ranges.
The three reviewed test-only patches are applied in
`20260907T081805Z-three-reviewed-fixture-patches-217f8fe7`. Runtime/list tests
subsequently pass 50/50 both before and after the if-let correction; artifact
tests pass 72/72 and Koto tests 37/37. No production alias or compatibility path
is introduced.

The full Sumeragi proof-ledger checker exits 1 after 2,925 seconds with 276
diagnostic lines in `seal-refresh-owner-20260907T070618Z-4uryw_kx/` under the
qualification root. It does not report the eight reviewed kernel seals or the
actual-index provider manifest. Completed static attribution establishes 242
failed predicates on authenticated HEAD inputs. This includes all 35 diagnostics
on the four adapter/effects/ledger/scheduler roots changed by this work. The
remaining 34 have identical complete source, owner, helper and expected-hash
inputs since the earliest retained resumed snapshot at 03:34:38. Those 34 are
not established as HEAD failures or as predating the original user task. No
diagnostic remains unclassified relative to these two distinct baselines.
This is static attribution, not a passing gate or a full HEAD checker execution.
The exact comparisons are retained in
`full-proof-diagnostic-attribution-20260907T081458Z-lpzpt413/retry/completed-static-attribution.json`.
No unrelated source seals are refreshed to suppress these diagnostics.

The twelve selected source guards have passing execution evidence: eleven
unchanged cases from the initial selection and the corrected Prepare-cache test
with all six original mutation predicates and expected diagnostics. Its stale
physical-provider references are replaced with the actual included leaves,
which match HEAD; the final single-test capture passes in 43.62 seconds with
zero failures or skips. The receipt is
`prepare-cache-remaining-paths-20260907T082151Z-m4sus8iw/test-20260907T082259Z-qtts16bk`.
This scoped success does not turn the broader failed checker into a pass.

The sibling documentation default production command also fails with a 6 GiB
V8 heap on immutable HEAD `de24208350f23c15ca5474ccb525ca9f1dee873a`, confirming
a pre-existing build-resource failure. Its source/dependency snapshot, denied
IP-network probe, actual exit and both checkouts' preservation are captured in
`docs-HEAD-default-build-20260907T071301Z-k1zsk2vx`. The updated tutorials pass
the 12 GiB production build and link checks for all 21 locales. Iroha's own
build remains independent of that optional checkout.

## Bounded live scan contract

`STATE_SCAN = 0x010038` consumes `r10` canonical NoritoBytes(StatePath map),
`r11` canonical NoritoBytes(StateCursorV1) or zero, and raw `r12` limit 1..64.
`r13`, `r14`, and `r15` are zero. It returns `r10` canonical
NoritoBytes(Vec<StatePath>), `r11` next cursor or zero, `r12` selected count,
and `r13` examined count. Every input/output is public under the strict pointer
ABI policy. Admission marks this as a durable-state read; proven map prefixes
produce wildcard read metadata, while unresolved accesses serialize.

A cursor binds the authoritative host instance, map name, exact key/value schema
hash, key kind and last examined canonical map path. Its canonical Norito frame
is at most 64 KiB. The schema hash uses
`KOTODAMA_STATE_MAP_CURSOR_SCHEMA_V1\0` plus the complete canonical Norito
EmbeddedStateType::StateMap frame. Type identity and map position do not confer
authorization. Hosts validate the current declaration and read permissions on
every call. Production instance binding comes from the authenticated contract
address namespace; isolated local hosts receive a deterministic instance context.

Backing storage and overlays seek strictly after the cursor and merge in canonical
path order. Each distinct merged position consumes one candidate, including a
tombstone. The scan stops immediately after 64 candidates or N live keys; it
neither counts the map nor probes for another position. A bounded page carries
its last examined position even when its continuation will be an empty terminal
page. Deleted cursor keys remain valid. Later calls read current invocation
state and overlay; insertions before the cursor are not revisited. Values are
materialized only for selected keys, at most N, using their signed schemas.

## Resumed qualification checkpoint — 2026-09-07

The temporary `/tmp/kotodama-*` and `/private/tmp/kotodama-*` records cited
below are unavailable after resumption, including the acceptance crosswalk,
Apple publication receipts, guarded network tools and final release daemon.
Recovered integration-harness and JavaScript-native bytes match their recorded
prior hashes, but that does not authenticate them against subsequent source edits.
The repository-local Apple package advertises ABI 21 and lacks the required
universal macOS slice; current Swift requires ABI 23. It cannot qualify the
current candidate. Existing native outputs must remain preserved while their
official owners produce fresh source-bound artifacts.

Source corrections retain receiver facts from the exact locked graph in incomplete
editor buffers, execute declared standalone tests before scaffold publication,
and reject empty test declarations or suites. Scaffold publication now uses one
rename without deleting an existing empty destination first. Ten Unix tests cover
validation failures, exact-rejection mismatches, injected final-publication failure,
late nonempty destination creation and successful empty-directory publication.
All ten pass in the fresh CLI run at
`target/kotodama-v1-qualification/20260907T050848Z-cli-scaffold-516f13a5d2/`,
with no skips and unchanged source, lock, HEAD and staged contents. The same
build compiles the production Core cancellation path; its runtime tests remain pending.
Swift fixtures now select the configured authenticated external artifact and use
the canonical contract address. A new Core regression selection exercises
certified Fetch/Store completion when a timeout supersedes its reducer owner;
it reproduces obsolete completion rejection but does not establish the cause of
all earlier network timeouts. Fresh compiler library tests pass 1,089/1,089, including all sixteen
editor tests and cursor positions after partially typed member names. The run at
`target/kotodama-v1-qualification/20260907T034223Z-compiler-lib-d664c52c85/`
records exit zero, unchanged source/lock/staged contents during execution and the
frozen Cargo-reported test executable. Completion retirement and native
SDK corrections still require fresh focused tests. The complete generated-project
command workflow still requires fresh normal CLI and koto executables. Retired-codec, diff and history/current-view
guards also pass; the history verifier checks 64,736 records and 67,311 occurrences.

Offline Python manifest/nominal-schema tests pass 220/220 under the existing
private Python 3.12.14 environment, with 139 selected inputs unchanged. Evidence
is `target/kotodama-current-python-manifest/qualification.json`. This is
source-mode schema evidence; the imported cached crypto extension is identified
in the report and is not a freshly qualified native build. JavaScript's five
selected schema/artifact test files pass 18/18, with 573 inputs unchanged and no
native-binding acquisition calls; evidence is
`target/kotodama-current-js-manifest/qualification.json`. Both runs deny network
access. The editor client tests pass 3/3 and all eighteen generated syntax files
remain current.

Focused Kotlin-owned JVM and Java-source consumers pass 17/17 without skips:
fourteen manifest tests, one Java nominal-schema test and two argument-record/
nested-Option transport cases. Gradle 9.3.0 runs offline on Temurin 21.0.11 with
one worker; JDK 8 compilation compatibility remains enforced. Test execution is
fresh, compiler tasks reuse verified up-to-date outputs, and source/lock/staged
hashes remain unchanged. Evidence is
`target/kotodama-v1-qualification/20260907T041025Z-kotlin-contract-consumers-f2b0ca1b7d/`.
No JNI build is claimed by this selection.

C# passes all 21 focused manifest/nominal-schema cases with no skips under its
pinned .NET SDK 8.0.419. The SDK is installed privately after checking Microsoft's
published archive SHA-512; global SDK settings and `global.json` remain unchanged.
All 193 selected source/cache inputs are unchanged. Tests use cached restore
assets with IP networking denied and local Unix-domain IPC permitted for the
test runner. Evidence is `target/kotodama-current-csharp-manifest-sdk419/`;
`target/kotodama-dotnet-sdk-8.0.419/installation.json` retains SDK provenance.
The earlier missing-SDK and blocked local-IPC attempts are resolved harness
prerequisites, not product test failures or native qualification.

The new Core supersession regression compiles, but all four cases stop in the
shared Prepare-QC fixture setup before exercising the target transition. The
captured command exits 101, with source/lock/staged contents unchanged, at
`target/kotodama-v1-qualification/20260907T034424Z-core-fetch-supersession-e8e413528c/`.
This is a test-fixture failure, not a demonstrated runtime reproduction or fix.
The second run, at
`target/kotodama-v1-qualification/20260907T040156Z-core-fetch-supersession-257170e540/`,
also exits 101 with unchanged source/lock/staged contents. Selecting the frozen
view leader fixes Fetch creation, but the older hand-built owner helper rejects
the authentic manifest-less Fetch before the target transition. The fixture is
being changed to use empty-owner construction and normal selected-response
admission, which binds the authenticated response manifest separately. Production
consensus behavior remains unchanged.

The third run, at
`target/kotodama-v1-qualification/20260907T041534Z-core-fetch-supersession-d8fac686ac/`,
compiles and executes four tests with unchanged source/lock/staged contents.
Both Fetch cases now pass real ingress admission, fsynced Ready publication,
the Busy fence and authenticated timeout/view installation, then fail at the
target retry with `Err(DispatchProjection)`. They reproduce the target Fetch
failure, including the protected-body case. Both Store cases still fail before
timeout while publishing the initial Fetch successor: the test's empty owner
lacks the paired live lifecycle ordinal authority. The fixture is being bound
to the normal shared runtime/coordinator authority before Fetch creation. The
command exits 101; no production fix or passing network result is claimed.

The fourth capture, at
`target/kotodama-v1-qualification/20260907T042933Z-core-fetch-supersession-f312dea852/`,
compiles but the newly added test logger fails before fixture execution because
the test has no Tokio reactor. The logger is removed. This is a fixture regression,
not evidence against the production transition.

The fifth capture, at
`target/kotodama-v1-qualification/20260907T043849Z-core-fetch-supersession-42c26242c6/`,
reaches the intended completion retry in all four cases. Fetch and Store both
reject with `Err(DispatchProjection)` after the Busy fence, authenticated timeout
and view installation; protected and unprotected cases reproduce. There are no
setup failures, no ignored tests and no production consensus changes in this
capture. Its command exits 101 with unchanged source/HEAD/lock/staged contents.
The exact frozen Core libtest SHA-256 is
`18c08b8d721eca110ba92315e6294df9454573fe770d778d4ff7d99c392cd936`.
The runtime repair now stages exact obsolete-work cancellation before carrier
retirement. Its sealed registry/reducer join and immutable marker comparison
preserve the durable body and current owner. Seven adapter negative/identity tests
and expanded ordinary periodic/FIFO progress and publication-failure tests await
execution. No passing repair result is claimed. All four reproduced cases remain
required.

That same freshly built Core executable passes all sixty contract-focused cases:
thirteen nested-call cases, three dispatched-call gas/rollback cases, twenty
return codecs, eight bounded merge cases, one production schema-bound scan, four
nested-state rollback cases and eleven metadata/rejection cases. The ten exact
selections each execute their expected nonzero count with no failures or skips.
Evidence is
`target/kotodama-v1-qualification/20260907T044628Z-core-contract-frozen-427e85d586/`;
source/HEAD/lock/staged contents and the frozen executable remain unchanged.
These passing contract paths do not close the reproduced completion bug.

The editor and native-JavaScript setup commands are corrected in English and all
twenty tutorial translations. Current content, locale/provenance checks and 56
focused validator tests pass. The package's normal `pnpm run build` exhausts its
fixed 6 GiB heap and aborts; that attempt is preserved in
`target/kotodama-current-docs-after-setup/`. The same maintained VitePress builder
passes with an explicit 12 GiB process heap in 97.4 seconds. Fresh built-link and
all twenty-one locale-root checks pass; all twenty-one built tutorial pages contain
the corrected setup. Source/dependency inputs remain unchanged during execution.
Evidence is `target/kotodama-current-docs-build-12g/qualification.json`. This is a
successful production documentation build with a larger heap, not a passing
unmodified package command or an established baseline for its memory failure.
Iroha's build remains independent of the sibling checkout.

The first full cancellation candidate compiles and executes 78 selected Core
cases at
`target/kotodama-v1-qualification/20260907T052636Z-core-retirement-qualification-8f4e7f247f/`:
68 pass, ten fail, none are skipped, and source/lock/index/HEAD are unchanged.
All seven adapter authentication/supersession cases, four closed-output or
failed-publication regressions, and the new 90-assertion source case pass.
Seven success regressions pass exact durable cancellation, body preservation and
current-owner/marker invariants, then misuse the dispatcher on an empty census;
the production caller first classifies this state as no work. Their fixture must
follow that precondition before current-work and complete recovery assertions
can qualify. Two source checks need review of the new opaque registry import
group; another existing source check rejects its cold leader-wire lock-round
anchor. These failures do not yet qualify the cancellation candidate.
The empty-census fixture now uses the production no-work classifier. The registry
guard pins the exact new two-item opaque import group. The lock-round assertion
also fails in the preserved earlier Core executable; its adapter delegation and
factory source are HEAD-identical. Evidence is
`target/kotodama-v1-qualification/retirement-guard-baseline-20260907T053918Z-ptadz8jh/`.
Its refreshed guard follows the actual factory, retaining the lock checks and
adding exact context, owner and WAL-frontier checks. All five compaction tests
pass with the 2,161-line reduction floor intact. A new production nested-call test
checks the same valid map cursor under denied, granted, revoked and restored
entrypoint permission; that regression passes in the next 81-test Core capture.


The next current-source Core capture (`20260907T055132Z-core-retirement-qualification-47788ca071`)
passes 76/81 tests, with no ignored cases. The corrected empty-census checks,
ordinary cancellation cases, failure-publication checks and cursor authorization
regression pass. Five failures remain: two restart cases, one coalesced Store
progress assertion, and an EnterView source guard plus its aggregate runner. The
periodic timer legitimately emits both the protected PrepareQC and Store; a test-only
Store counter now checks that actual batch without changing the production effect
classification. Startup diagnostics now distinguish authenticated adapter replay
failure from census preparation. These edits and the remaining source guard require
fresh Rust execution. The restart fixture used generation one although ordinary
view installation and the production cold opener use `Generation::INITIAL`; its
helper now follows that production seed without changing persisted tags or replay
validation. The next run must establish whether this resolves the restart failure.


The following capture (`20260907T061551Z-core-retirement-qualification-892a435c29`)
passes 78/81, with every source guard passing and no ignored tests. The real
ordinary restart still rejects BodyAvailable: WAL recovery restores the protected
old-round lock but does not seed its missing body work. This is a runtime recovery
issue. The two Store cases reach a test-only assertion that incorrectly expects a
single broadcast; the real periodic batch contains the protected PrepareQC, its
TimeoutCertificate and one Store. The corrected observation now checks all three
and the exact PrepareQC. A fourteenth fixture case adds a real strict same-view TC
upgrade before restart to test persisted generation recovery. These latest fixture
changes and the prospective runtime correction are not yet qualified.


The next fixture-only baseline (`20260907T062856Z-core-fetch-supersession-b2a583871e`)
passes 11/14. Both genuine restart cases now report exactly “BodyAvailable replay
has no matching work”; the corrected mixed-broadcast/current-progress case passes.
The attempted fourteenth case fails its own strict-upgrade precondition: its
already-known PrepareQC cannot strictly advance the retained highest QC. That
invalid fixture extension is removed. Five independent reducer tests now cover
exact lock-body recovery, bounded lock/high versus Decision ownership, unchanged
no-timeout seeds, live/WAL generation parity with successively higher certificates,
and ordered malformed/foreign WAL rejection. Their baseline runs before any
production recovery change; prior failing fixtures remain recorded as such.


The production-unchanged 18-test baseline
`20260907T064552Z-core-cold-recovery-baseline-8fa058b68d` passes 13/18: both real
cold restarts and two exact lock-body unit tests fail on missing body work; the
live/WAL parity test confirms generation zero versus live generation one after a
valid strict upgrade. No-timeout seeds and exact malformed-WAL rejections pass.
The reviewed repair now seeds only the exact undecided lock through the same
non-overwriting insertion used by live Fetch, without a new startup effect or
historical Prepare authority. Recovery reconstructs generation in the same ordered
WAL pass, using the live checked transition and preserving durable validation error
precedence. The six-file production/guard patch is applied; scoped format and diff
checks pass. The expanded runtime selection adds 21 existing boundary, FIFO,
Decision and authenticated-WAL checks. Its capture
`20260907T065745Z-core-retirement-qualification-3f325b77a0` passes 105/107,
including all five new recovery tests and all 21 existing boundary cases. Both
cold fixtures now reach `ValidateQueued`, then fail the unchanged receiver
teardown guard because the fixture leaves that work queued. A test-only cleanup
must finish and acknowledge the exact worker completion before teardown; the
production recovery assertions pass, but these two complete tests remain open.

Fresh ABI value tests pass 172/172 with no failures or ignored tests in
`20260907T070644Z-abi-values-8ec2bc7dc3`. Both captures preserve source,
Cargo.lock, HEAD and the complete current index during execution.

Fresh workspace formatting (`cargo fmt --all -- --check`), retired-codec checks
and the exact 2026-09-06 history archive verification all pass without changing
source or index. Their current-source receipts are retained under the qualification
root; full workspace compilation, lint and execution remain pending.


The three new include providers are now staged through standard Git for the
repository's actual-index admission check. The controlled staging receipt
`owned-provider-staging-20260907T064513Z-78227f8e` verifies all 18,617 original
index rows unchanged, exactly three new stage-zero blobs, and unchanged worktree,
HEAD and Cargo.lock. No commit was created; no unrelated entry was staged.

The semantic-helper source guard is refreshed for the approved V1 test bodies,
error-value traversal, exact rejection selector, removed `StateKeys` builtin and
diagnostic identities, including the new fifteen-test labels/patterns leaf. Its
existing line cap, mutation checks, required regression names and effect-walker
invariants remain; all twenty-six selected source/syntax/IR tests pass after the
refresh. The previous guard also rejects isolated HEAD sources at its
builtin-set check; this source-guard baseline is distinct from Rust runtime
evidence. Review inputs and the exact patch are retained in
`target/kotodama-v1-qualification/semantic-guard-review/`.

New command evidence is retained under the ignored
`target/kotodama-v1-qualification/` directory with exact source/lock/staged-content
inventories, actual exit status and frozen Cargo-reported binaries. An active
capture is not a pass. C9 stays open until current-candidate compiler/CLI/Core,
native SDK, final artifacts, normal-release four-validator execution/restart and
workspace validation have authoritative evidence. Preserve separately established
baseline failures without claiming that historical results prove current success.

## Historical qualification records

Earlier checkpoint prose is preserved once in the
[dated subsystem record](../docs/history/2026-09-07/kotodama-v1-redesign-qualification.md).
Its original section bytes and SHA-256 remain recorded there. Historical counts
and unavailable temporary paths do not close any current acceptance gate.
