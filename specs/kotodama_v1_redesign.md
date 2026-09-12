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
| Collections and arithmetic | Checked/fallible list mutation, Result obligations, fused rounding, cursor types, bounded list loops, STATE_SCAN and seek-based hosts are implemented. | Four-validator execution; compiler/VM/Core tests cover rollback, unchanged fallible mutation, no implicit Result loss, rounding modes, representable fused results with large intermediates, pagination bounds and gas. |
| Shared boundaries and rejection | Unit/error schemas, signed descriptors, exact abort propagation, explicit rejection selectors and maintained SDK record consumers are implemented. Cursor schema propagation is implemented. | State/argument/return/nested-call roundtrips and malformed identity/schema/code rejection on the final ABI; wrong-stage test failures and restoration. |
| Authoring and onboarding | Immutable editor snapshot, semantic LSP, rich diagnostics, packaged editor client and staged standalone-test validation are implemented. | Fresh editor, CLI/LSP, offline-project and updated tutorial checks; network invocation remains part of release qualification. |
| Final release assets and qualification | Final V1 artifacts and the packaged editor client are implemented; a39f offline-project and golden checks pass. Implementation and scoped acceptance C9 are complete. | Fresh installed-artifact admission/runtime checks, combined Core/native SDK consumers, four-validator integration, then workspace validation. |

No milestone is closed by source implementation or by a test result from an
earlier schema revision. Cargo.lock and unrelated worktree/index changes are
preserved. Tests requiring node consensus use four validators and mandatory
signed RS16 availability.

## Current isolated qualification — 2026-09-10

C9 implementation and scoped acceptance are complete. The current isolated source is
`65021e4dd261ad2fcca057c215e07cfedbdd399f63070168456a5b67dbd1f71c`,
HEAD `ff22bda4c42219ccc055a69d7b5ce86e25b299e8`, and Cargo.lock
`fe9a6f9e30fe537059868ebddd826a70c1312d09b7d308cbe5d55e6ec7bfd32b`.
The State registry repair defers WSV lookup only for historical carriers beyond
the coherent State height. Live execution and restored-prefix history retain
exact ownership checks; immutable reservation, route/session and payload checks
remain unconditional. Subsequent Core corrections affect test fixtures only;
the SDK diagnostic, deterministic C# test-key and JavaScript controller corrections
are recorded below.

The new cold-start fixture initializes its real genesis authority before State
and signed commitments, then restores the daemon’s complete geometry,
configuration and manifest sequence without changing canonical State. Its applied
two-file admission correction replaces direct QueuePlan owner seeding with signed
admission at height 2, source carriage at 3 and merge application at 4. Original
queued bindings remain intact through reservation. All nine existing fixture
cases keep their prior behavior and assertions; the new case checks snapshots
1/2/3 replayed to 4, exact live rejection at 3 and restored-prefix rejection at 4.
All 11 directly affected tests pass with zero failures or ignored cases, native
0 in 1,161.63 seconds; all 15 capture/preservation checks pass on unchanged a39f.
This covers the new cold-start case, all nine existing historical-fixture callers
and the existing plain-successor archive case. Full Core/workspace qualification
is not implied.

Earlier b814 and b11 Core29 runs each record 27 passes and two failures. The
settlement failure reproduces on the exact d12 executable; cold-start fixture
failures subsequently reach the 86c startup anchor and e1af WSV checkpoint checks.
Exact outcomes, source identities and immutable originals remain in the
[dated record](../docs/history/2026-09-10/kotodama-v1-validation.md#real-admission-fixture-and-focused-validation).
The earlier ordinary Python release rebuild and installed-consumer run passes on
unchanged a39f: 222 + 9 cases, 231 executions and 229 unique tests, with zero
failures or skips. All 12 subprocesses exit 0 and all 15 preservation checks pass;
the freshly installed extension passes the official ABI-23 probe.

The six-file SDK correction is applied in both roots. It fixes four stale `range`
diagnostics in Python, Swift and Kotlin to the existing `take`/`page` policy and
adds exact plain `range` rejection assertions without compatibility support.
The ordinary Apple producer completes all five target builds and C links on 9a45,
including host ABI/crypto execution and official readbacks. Its three generated
Swift hashes are then applied in both roots; official live-pin, provenance and
normalized-fingerprint checks bind producer 9a45 to consumer source 1252.
The subsequent ordinary Apple6 producer qualifies on current 65021 with the
new `dd1683a1…` official fingerprint. All five target builds/C links and host
ABI/crypto checks pass. Its three archives, host library and projected loader
match the earlier artifacts and shipping pins exactly; no pin delta or SDK rerun
is required. Apple5 remains historical evidence at its original source.

On 1252, the ordinary Python Maturin build, fresh installed ABI-23 probe and exact
223 + 9 selections pass: 232 executions/230 unique, zero failures or skips. All
12 subprocesses exit 0 and all 15 preservation checks pass. Cargo reuses Core and
compiles the Python native crate; the installed extension matches the earlier
native bytes. A preceding mistyped 61-character source argument is rejected before
setup or native execution; its failed receipt and unavailable guards remain retained.

The C#42 attempt on 1252 instead reports 25 failed cases and one framework error;
17 parameterized rows are missing. The test-class initializer passes repeated
bytes as an Ed25519 public key, which normal native validation rejects. The
one-file deterministic-seed correction is now applied on 7ce7 with all eight
application guards per root passing. It derives valid public keys with the
existing SDK helper and preserves all 31 callers, assertions and selected names.
The official native binding covers the fixture transition with unchanged Apple
fingerprint. The complete rerun passes all 42 cases, zero failures, skips or
framework errors, including all 17 previously missing rows. All four commands
exit 0 and source/index/artifact guards pass. Kotlin17, including the Java consumer,
and the subsequent Swift24 run also qualify on 7ce7 with all preservation guards.
On 7ce7, the Node 26.8.2 native build and ABI-23 probe pass after the retained
missing-tool failure, but the selected JS tests execute only 29 cases: 28 pass,
one fails and two are not reached. Browser deployment reads obsolete `_controller`
state and rejects a valid authority before the expected schema-hash diagnostic.

The 17-path correction was applied in both roots on source 231de5ae.
`AccountAddress.controllerInfo()` exposes normalized single-key or multisig
records from private state, with frozen records/member arrays and fresh public-key
bytes. All six consumers use that API; strict native admission and browser
unavailability remain enforced. Added tests cover controller isolation, strict
TypeScript declarations, exact restrictions on valid account variants and
independently derived multisig receipt bytes. The complete 72-case selection now
passes on 231de5ae: all 14 groups,
zero failures or ignored cases, and all 21 native subprocesses exit 0. It includes
every original JS31 case, all new controller tests and the multisig helper caller.
Source, exact index bytes, pins, tools, built dist and prior-artifact guards pass.
The earlier 7ce7 failure remains retained. Retained daemon/CLI build-input
continuity and workspace admission pass for 231de5ae. Three later compile
failures expose five caller paths, now corrected. The current workspace test
executes 1,512 passing cases, three storage-startup failures and 27 ignored cases.
Workspace build passes; strict Clippy stops at a retained unrelated primitives
lint. Focused SoraFS9 passes all nine cases. The
[dated record](../docs/history/2026-09-10/kotodama-v1-validation.md#apple-pins-python-and-csharp-fixture-outcomes)
binds each result, failed preparation and source transition separately.

The final sibling documentation source `7e397d51…` passes all six offline commands:
validation, 164 tests, type checking, an explicit 12 GiB VitePress build, link checks
and all 21 locale roots/revision checks. All five preservation guards pass. The
site vendors the exact canonical `0efdae17…` grammar with pending signed-source
provenance and no sibling-Iroha build dependency; actual output highlights all
six tutorial `ko` blocks without fallback. The earlier e2a build, IPC failure and
first focused span-boundary assertion failure retain their scopes. The package's
historical 6 GiB build failure is not relabeled as a `pnpm build` pass.

The original 35-row acceptance matrix keeps its 1,753 executions at their original
sources. The 7ce7 closing index confirms unchanged bytes for 4,502 compiler/Koto/ABI
input records and six packaged editor records; it does not relabel Core runs.
The 7ce7 read-only syntax check passes all 18 generated outputs. The subsequent
17-path application changes only JavaScript SDK source, declarations and tests.
The [dated record](../docs/history/2026-09-10/kotodama-v1-validation.md#sdk-index-recovery-and-highlighted-documentation)
binds the fresh docs, SDK and source-continuity evidence. All 35 redesigned-behavior
rows have scoped focused passes. The broader workspace test outcome and its
unreached targets remain separate. Build passes; Clippy retains the attributed
lint failure, and SoraFS9 passes. The implementation acceptance is complete;
these results do not assert full workspace health or release readiness.
Broader Core/formal failures remain unwaived.

The a39f normal release daemon and normal no-run integration harness builds now
qualify with native 0 and all nine capture guards each. Both exact four-validator
scenarios pass one test, zero failed or ignored, native 0: companion in 233.95
seconds and original in 354.73 seconds. Each uses one attempt and passes all ten
capture guards on unchanged a39f and frozen binaries. The tests also reach
authoritative restart and exact view/state readbacks. One unlabelled cleanup peer exit 1 remains
unexplained after bounded review: its peer and cause are unobserved. The test
passes do not establish all-peer clean shutdown; the exit remains unclassified.
One unchanged targeted repeat of the original scenario then passes 1/1, native 0,
in 360.37 seconds, with one four-validator attempt and all ten capture guards.
All five logged exits are 0 and all five stderr files are empty. The earlier exit 1
does not recur in this single repeat; its cause and classification remain unresolved.
The a39f CLI/Koto build, 10 scaffold and nine debug tests also pass. The offline
project passes 12 command checks and three exact tests; overwrite refusal preserves
the project. The official two-render golden check matches 57 bytecodes and four
manifests with zero changes. These are a39f results, not evidence for later SDK edits.
The [dated results](../docs/history/2026-09-10/kotodama-v1-validation.md#a39-cli-and-targeted-network-repeat)
retain receipt identities and the earlier unexplained exit. The later native/SDK
passes are recorded above; retained-input continuity and admission pass for
231de5ae. Current workspace outcomes and remaining gates are recorded below.

The preceding two-path Kura repair is also applied in both trees. Historical
bundles now consume an existing exact
reservation component after strict durable readback, without requesting admission
to replace the newer certified frontier. All 22 existing capacity tests remain;
two new primary-lane cases cover actual cold restart, future publication and
precise historical-corruption rejection. Core24 on `9cd08581…` records 19 passes
and five failures. Three unchanged failures reproduce on the exact 961 executable.
The new positive fixture lacked its signed lifecycle cursor; the negative used
an uncanonicalized expected path. Both fixtures are corrected, preserving all
22 previous test bodies and exact corruption checks. The complete Core24 rerun
on `d12accbd…` records 21 passes and those same three failures, native 101,
zero ignored, in 187.95 seconds. Both new cold-restart and exact-corruption tests
pass; the independent outcome/source audit passes all 22 controls. The suite
remains failing. The recovery-capacity static contract, workspace formatting and
retired-codec guard pass on d12. Its normal release daemon and normal-profile
harness builds qualify. Both complete four-validator runs then fail at State
hydration before replay, after Applied calls, state 7/readbacks and RBC. The
51-check terminal audit preserves both native-101 failures; the 18-check forensic
review identifies the mandatory registry lookup against empty startup WSV as a
failing predicate. This is later than the repaired Kura capacity guard. The
[dated record](../docs/history/2026-09-10/kotodama-v1-validation.md#state-registry-restart-failure-and-applied-boundary-fix)
retains the exact receipts and the limits of the forensic evidence.

The real packaged editor-host smoke passes all 29 assertions and 14 preservation
controls: semantic snippets/signatures, hover, navigation, references, parameter
label rename, Japanese/emoji UTF-16 diagnostics and recovery, and formatting.
The host uses the exact packaged client and frozen compiler; their source and
bytes remain unchanged. Its retained protocol confirms rename document version 4
and an empty second-format response. This closes the previously missing real-client
smoke, with source scope and earlier runner failures retained in the dated record.

The preceding four-validator scenarios executed on `961db6d8…`. They passed both
Applied calls, all four validators' contract/state readbacks and later RBC checks,
then failed the restarted validator's Kura initialization with the same certified
frontier capacity-identity conflict. Companion fails 0/1 in 234.26 seconds;
original fails 0/1 in 345.12 seconds, both native 101. All eight preservation and
non-success controls per run and the 38-check terminal audit pass. Restart
readback was not qualified by those runs. The earlier verify-wait failures remain distinct.
Apple R8 also completed on `961db6d8…`: all five normal release target builds and
C links pass, with host ABI/crypto execution and 35 independent readback checks.
The message-control daemon build succeeds on that source. The subsequent production
Kura and State inputs require fresh affected binaries/native qualification before
these results can support the repaired candidate. Exact receipts and tool-recovery
limits are in the [dated record](../docs/history/2026-09-10/kotodama-v1-validation.md).

The earlier three-file application captured MAIN as
`a16ac8ada050d40f5c8adc44014e3e5432a53baa2a4bc08e45b5b00af1846f63`;
that recorded source precedes these ledger edits and is distinct from PRIVATE.
The three test-only rendezvous/formal-anchor changes are applied in both trees,
with all eight application checks per tree passing.
The reviewed 27-path change binds contract-call drafts to QueuePlanSynced before
quotation, hashing and signing. Maintained SDK validators require that exact
intent; unrelated transaction policies retain their existing values. Unsigned
preparation is pure. Detached signed submission uses the canonical global ingress
owner, and only its accepted result produces a submitted contract receipt.
The companion fixture now exercises detached `verify`; local signing remains in
the other selected calls. That application alone did not establish a network pass.

Results retain their actual execution sources or explicit input comparisons.
SDK 25, Torii 15, installed Python, pure JavaScript and C# manifest checks
retain their actual `3979c0d7…` results. Miniature 11 and CLI 19 pass on
`ff9a50a0…`; exact fixture-only source comparisons preserve the stated consumer
input scopes. On `bfddb2ff…`, Torii passes 16/16, the gas fixture passes 1/1,
and both the normal-release daemon and no-run harness builds pass. Both fresh
four-validator scenarios fail at verify's Applied wait. The companion logs fatal
frontier guards; the original progresses near its fixed wait cutoff without
those fatal markers. Typed frontier deferral and the cadence-derived fixture wait
are implemented. The focused Core26 selection passes on `a0beef73…`, with
its independent terminal audit passing. The normal-release daemon now has a
separate successful capture on `961db6d8…`; Cargo reports `fresh=true`, reusing
the exact a0 daemon bytes. The applied completion-aware Apply rendezvous is
robustness work. On the earlier `961db6d8…` source, Core5 passes all five cases and the two
selected formal pytest nodes pass, including all 29 semantic mutation checks.
The earlier Core26 result remains scoped to its actual `a0beef73…` execution.
The earlier CLI19 and offline project/golden passes on `961db6d8…` retain that
source scope; the later a39f results are recorded above, alongside JS72 on 231de5ae,
Python on 1252, C#/Kotlin/Swift on 7ce7 and the Apple producer/binding results.
Compiler, bytecode, pure JavaScript and Rust SDK results retain their separately
verified input scopes. Current workspace test records 1,512 passes, three failures
and 27 ignored cases; build passes, Clippy fails at the retained primitives lint,
and focused SoraFS9 passes all nine cases.

| Selection | Executed result and scope |
| --- | --- |
| Compiler library, compiler integration, Koto/LSP and ABI/artifacts | Compiler integration passes 78/78 on `2d270a80…`, including the three-case exact-rejection-arity regression. Library 1,090/1,090, Koto/LSP 37/37 and ABI/artifacts 73/73 retain their recorded affected-input continuity from `f899fb47…`. These are earlier executions, not a whole-source qualification of the current SDK/Torii change. |
| Rust SDK contract boundary | On `3979c0d7…`, all 25 contract/authentication cases pass, zero failed or ignored, including exact signed admission intent. The six blocking-runtime passes remain bound to `4759aae1…` and their approved direct launcher; they were not repeated in this selection. |
| Core completion and recovery | 135/141 pass, six fail, zero ignored; the selection exits 101. All 107 original cases, both new corruption cases and repaired certified-body cases pass. The six failures remain unwaived; their stronger committed-source byte lineage and its limits are recorded below. |
| Core merge-frontier selection | On `a0beef73…`, all 26 exact cases pass, zero failed or ignored, native exit 0. The ordinary test profile uses the existing `iroha-core-tests,bls` features, with no `RUST_MIN_STACK` override. All nine capture and 18 terminal-audit checks pass, including exact source and actual index bytes. This focused result does not close Core141, formal, native or network gates. |
| Core completion-aware rendezvous | On `961db6d8…`, all five exact cases pass, zero failed or ignored, native exit 0: four completion-aware helper cases plus the real SuccessfulApply case. All nine capture and 20 independent audit checks pass, including exact source and actual index bytes. The ordinary test profile and existing `iroha-core-tests,bls` features remain, with no `RUST_MIN_STACK` override. |
| Focused frontier formal checks | On `961db6d8…`, both selected pytest nodes pass, including all 29 semantic mutations, native exit 0, zero failures/errors/skips. All nine private source/index/HEAD/staging/status/lock checks pass. The earlier full-helper baseline failure was not rerun or waived. |
| ABI, VM and public-boundary queue | All 450 execution rows pass on `9f7f713b…`: ABI values 172, values/scan/test driver 57, runtime/lists 50, numeric/pointers 40, data-model contracts 58, Core contracts 61, Core header 1, manifest metadata 6 and Torii state routes 5. These retained passes do not close the separate Core 141 or current native/network/workspace gates. |
| Managed contract-record consumers | On `3979c0d7…`, pure JavaScript passes 18/18 and C# manifest checks pass 21; the earlier C# contract selection fails at the unavailable ABI-23 prerequisite. The new 1252 C# attempt loads the qualified host but fails test-class initialization: 25 failed cases, one framework error, 17 missing rows. The deterministic-seed fixture on 7ce7 then passes all 42 cases, including the 17 rows, with zero failures, skips or framework errors. Kotlin17 and Swift24 now qualify on 7ce7. The later Node26.8.2 JS attempt passes native build/probing but executes 29 cases: 28 pass, one fails at the obsolete deployment authority lookup and two are not reached; the applied controller correction then passes all 72 cases on 231de5ae, with zero failures or ignored cases and all preservation guards. |
| Earlier Python installed consumers | On `3979c0d7…`, the retained 220 cases plus two admission negatives pass 222/222; nine focused positive/substitution/admission cases also pass. This is 231 executions and 229 unique cases, zero failures or skips. The updated pure client wheel is installed in a fresh venv. At that execution, all 2,109 actual native compiler-input records matched the retained ABI-23 extension, which loaded normally. No native rebuild or clean-source release seal is claimed. |
| Python after SDK diagnostics and generated pins | On 1252, normal Maturin build/install, official ABI-23 probing and all 223 + 9 selected cases pass: 232 executions/230 unique, zero failures or skips. All 12 subprocesses and 15 preservation checks pass. The preceding malformed source-argument attempt stops before setup; both receipts are retained. This does not qualify the later C# fixture source. |
| Source assets and formatting | On `961db6d8…`, `cargo fmt --all --check` and `bash scripts/check_no_legacy_codec.sh` each return native 0. All ten preservation checks pass, including exact source entries, actual index bytes, HEAD, staging, status and Cargo.lock. Earlier format/codec records retain their original sources. The later 7ce7 read-only syntax check passes all 18 generated outputs and all nine guards without regeneration. |
| CLI and offline onboarding | On a39f, the normal CLI/Koto build and all 10 scaffold/nine debug cases pass, zero failed or ignored. Each of the three captures passes nine guards and a 17-check audit. The fresh offline project passes 12 commands and three exact tests; expected overwrite refusal preserves the project. All eight external checks pass. Earlier 961 (39 consolidated checks) and `ff9a50a0…` results retain their sources. |
| Canonical artifacts and admission | Retained exporter checks preserve all 69 payload files; the owner seal covers 61 outputs and current-Rust provenance binds 2,613 inputs. Admission 15/15 and browser checks 2/2 retain their recorded scopes. The fresh a39f official golden check renders twice, matches all 57 bytecodes and four manifests in bytes/modes, and reports zero changes. Earlier 961 and `ff9a50a0…` checks retain their sources; no artifact promotion or whole-source release qualification is implied. |
| Torii certified preparation, state routes and gas fixture | On `bfddb2ff…`, all 16 exact Torii cases pass, zero failed or ignored: the retained 15 response/state, selector and certified-ingress controls plus the network-time/drift/TTL regression. The signed genesis gas fixture separately passes 1/1. These focused checks do not establish live consensus acceptance. |
| Startup status read and focused controls | The dedicated startup read thread retains its six passing thread/error/panic, height and retry controls. Required height, errors and retry limits remain unchanged. This is earlier focused evidence. |
| Earlier daemon, harness and network scopes | The a0 normal daemon returns native 0, with nine capture and 32 independent checks passing. A new 961 normal capture returns native 0 with `fresh=true`, nine capture and 35 independent checks passing; its frozen bytes match a0. Both are build-only. The new 961 normal no-run harness build returns native 0; all nine capture and 32 independent audit checks pass. Both network selectors return 0 in dry-run admission against the qualified pair; no validators or scenario tests run in those preflights. These were build preflights; the subsequent 961 cold-restart failures are recorded above. Prior bfdd builds pass; companion fails 0/1, native 101 in 204.16 seconds (two validators exit 1 after fatal frontier guards, two exit 0); original fails 0/1, native 101 in 321.57 seconds (all four exit 0). Verify Applied stays HTTP 404; final readback/RBC/restart assertions are unreached. Exact source/index/lock/binary preservation and earlier failures remain recorded. |
| Historical d12 network outcome and subsequent repair | The d12 normal release daemon and harness builds qualify. Both complete four-validator scenarios fail 0/1, native 101, after Applied calls, all-validator state 7/readbacks and RBC, at State registry validation before replay. The a39f source applies the historical registry boundary repair and replayable admission fixture; all 11 directly affected tests pass, zero failed or ignored, with all 15 preservation checks passing. Earlier Core29 and exact cold-start failures retain their dated scopes. The later a39f scenario results above retain their separate scope. |
| Documentation | On sibling source `7e397d51…`, all six offline commands pass: validation, 164 tests, typecheck, explicit 12 GiB build (5,998 fresh outputs), links and 21 locale roots/revision checks. All five preservation guards pass; all six tutorial `ko` blocks contain colored spans with no fallback. The earlier e2a/161-test pass, first IPC failure and focused assertion failure remain retained. The missing older report is not reconstructed; the historical package 6 GiB failure is distinct. |

The stronger Core comparison matches all 18,661 `ff22bda4…` tree-entry contents
and symlink targets to the retained pre-current-worktree source capture, with
four data-file mode differences explicitly recorded. It also checks 51 emitted
local packages and the historical lock. That actual 139-case execution had
131 passes and eight failures, including all six remaining current failures.
Five match the same panic stage after temporary-path-only normalization. The
payload-slot case instead fails earlier in the current strict occupied-slot
reader; its older restart-stage failure is not relabeled as the same stage.
This is failure lineage through committed-source bytes, not a fresh HEAD run,
pre-V1 baseline or suite pass. All six failures remain unwaived.

The effects closure and Rust include-manifest seals were already stale. The two
exact retired-reader negative seal tests retain their `9f7f713b…` passes and do
not waive those global failures. The completed redesign acceptance retains
these separately attributed blockers; no broader Kura/formal redesign is inferred
from this comparison. Exact receipts and historical stages are in the
[dated record](../docs/history/2026-09-08/kotodama-v1-requalification.md).

The earlier `adc1a627…` companion's detached response passes exact
payload/signature/hash and closed response checks. Omitted status scope already means global; global status
intentionally suppresses pending records. Its HTTP 404 does not prove rejection
or a missing certificate. Strict decoding finds the same valid verify admission
certificate in all four closed stores: authority height 7, proposal height 8,
lane 0/dataspace 0, durability threshold 2 from four validators. This admission
threshold is distinct from consensus QC quorum 3 of 4. All four logical block
indexes contain the same canonical chain 1–8; height 8 contains that certificate
but no external entrypoint or merge reference. Execution, finality and WSV remain
unproven. Decoded header/enqueue timestamps differ by 81,842 ms; this fact alone
establishes no clock causality.

The four signed genesis files from that earlier run each contain 10 instruction transactions,
104 instructions and seven parameter updates, with no `ivm_gas_limit_per_block`.
The effective starting block budget is therefore 4,000,000. Proposal selection
charges the full signed verify bound of 5,000,000; no route share can fit it,
even alone. This proves a scheduling barrier after certified registration,
without reconstructing private queue records or WSV.

The applied fixture sets its signed genesis block budget to the existing
5,000,000 call bound and checks that parameter exactly once in both scenarios.
The 200-entry workload, four validators, deadlines and protocol checks remain.
The applied contract-clock correction uses Core network time only when creation
time is omitted; explicit values, signatures and drift/TTL/NTS checks remain.
The original failed run did not retain its exact prepared timestamp or clock
offset, so its numeric skew is still unproven. Fresh Torii 16 and gas fixture 1
pass, and both new networks contain the exact signed 5,000,000 parameter. The
original now reaches verify submission; both new networks still fail its Applied
wait. Earlier HTTP 409, prepare timeouts and missing-null failures remain historical.

The new companion's exact verify certificate and autonomous anchor appear in
all four identical indexed chains through height 9. Two validators then fail
closed on execution-cache/merge-candidate frontier guards. Merge validation now
distinguishes deferred frontier movement from stable invalidity or storage faults.
Callers stop merge continuation on deferral without erasing completed durable
authority. All nine real Apply cases and the selected lane/runner controls now
pass in Core26, including SuccessfulApply and fatal-parent regressions. The
applied completion-aware rendezvous is robustness work, not a repair of an
observed failed case. Its two Rust test paths and the formal-test anchor path
are absent from eight retained production/consumer input inventories; both Rust
files remain inputs to the Core test crate. This exclusion does not renew older
CLI/native outputs invalidated by the preceding production Core changes.
The original has no matching
fatal marker: all four chains reach height 15, with its exact certificate at 14
and autonomous anchor at 15. Public indexing does not establish finality or WSV.

Original wall logs validate the certificate body 10.682–14.232 seconds after
verify, the autonomous anchor at 40.918–44.431 seconds, and a height-16 body at
59.703–60.171 seconds. That last body is not independently indexed or decoded as
the execution carrier. No per-contract compile/VM duration is recorded. Signed
cadence is 8 seconds, the configured round budget is 80 seconds, and the existing
DA quorum wait is 104 seconds. The fixture now uses that existing helper for
deployment and invocation Applied waits instead of 60 seconds.
Rejection/expiry fail-fast, workload and all downstream checks remain. These
timings justify the fixture bound; they do not prove eventual network success.

The earlier retained Python native build is bound to `122942d3…`; its installed run
checks the recorded native compiler inputs and loaded extension bytes. The
remote-client wheel was built from that run's recorded source. The new Core
change intersects three native compiler inputs and required native requalification.
Two pre-test runner failures—import order, then collected-nodeid
identity—are retained; neither executed tests or changed source assertions.
The final owner pins the repository root and records a bijective mapping of the
original 220 nodeids. All 12 preservation guards pass with IP networking denied. The earlier `961db6d8…`
Python source-only binding passes all 17 guards. The fresh a39f release rebuild
and installed-consumer pass above supersedes the earlier native requalification
hold for that source. The later 1252 build/install and 232-execution pass above
qualify the diagnostic correction and generated pins at that actual source.

The earlier Apple5 producer and official artifact readback pass on 9a45; the
latest Apple6 qualifies current 65021 with unchanged native outputs/pins. The
applied three-pin transition to 1252 passes all three official
live-pin, provenance and normalized-fingerprint commands with preserved source,
index and artifacts. These are local qualification results, not clean-Git
publication certification. Python passes on 1252. After the retained C#42 failure,
the applied 7ce7 fixture correction passes all 42 cases with zero framework errors;
the official fixture-transition binding preserves the Apple fingerprint and artifacts.
Kotlin17 and the subsequent Swift24 run qualify on 7ce7 with exact index preservation.
The 7ce7 JavaScript failure retains 28 passes, one failure and two unreached cases.
The applied controller API correction passes all 72 cases on 231de5ae, including
the exact earlier failing test and both previously unreached contract-call cases.
The normal release-daemon recapture qualifies on unchanged 7ce7, native 0 with all
nine capture checks and exact raw-index preservation. Its frozen binary is the
343,283,728-byte `e6abc762…` ordinary release build (opt 3/debug 0/non-test).
The a39f harness, separate message-control daemon and CLI checks retain their
actual scopes. On 231de5ae, the retained-input review passes all 22 checks;
workspace admission passes all 14 checks and three previews. The
[dated record](../docs/history/2026-09-10/kotodama-v1-validation.md#retained-native-input-continuity-and-workspace-start)
binds the unchanged build inputs and original execution identities. The
[subsequent compile failures](../docs/history/2026-09-10/kotodama-v1-validation.md#workspace-cid-test-compilation-failure)
all retain native exit 101 before tests execute and all 11 preservation checks.
Five corrected paths cover a CID test fixture, three alias-bootstrap test sites,
and three SoraFS caller files. The SoraFS changes include a production CLI-bin
reader, not only tests: two readers pass explicit byte slices, and a canonical
fixture uses final `load_prepared_storage_payload` with real directory/manifest
commitments. Assertions and production APIs remain intact. Retained f899 and
committed-source comparisons establish limited lineage, not an original dirty
baseline. Those exact BIN/helper paths are absent from retained daemon/CLI,
SDK and Kotodama harness compiled inputs; the shared library remains distinct.

The current 65021 workspace test completes with native 101: 1,512 passed,
three failed and 27 ignored across the observed summaries. The three sandbox
startup cases cannot reserve a nonzero automatic storage budget under the
existing headroom policy; they fail before block 1. The bounded source/disk
observation attributes this run to available storage, without original-task
baseline attribution or a policy change. Normal Cargo stops at that failed
suite; later workspace targets are not claimed as executed. All 11 preservation
checks pass. The [dated results](../docs/history/2026-09-10/kotodama-v1-validation.md#workspace-test-storage-startup-outcome)
retain exact counts and failure names. The separate ordinary workspace build
passes with native 0 and all 17 checks. Strict Clippy exits 101 at its first
actual error, `clippy::option_if_let_else` in `iroha_primitives/src/conststr.rs`.
That complete file matches retained f899/a39/7ce/current source and MAIN HEAD;
PRIVATE HEAD differs, and original dirty-baseline attribution remains unproven.
Later targets are not lint-qualified; unrelated warnings are not counted as
Clippy failures. Focused SoraFS9 passes exactly nine cases, zero failures or
ignored cases, with both native commands and all 16 checks passing on 65021.
The implementation and all 35 redesigned-behavior acceptance rows are complete.
Required broad gates were executed and their failures attributed within the
recorded limits; those workspace-health failures and unreached targets remain
explicit, with no full-workspace or release-readiness claim.

Earlier Apple failures retain their exact scopes: the first attempt lacked
required output directories; R7 failed strict PQClean reference provenance after
its first host Cargo build; the later cleanup owner failed its exact-removal
guard despite bounded attribution. The [September 8 record](../docs/history/2026-09-08/kotodama-v1-requalification.md)
retains timings, inventories and failures. The successful five-target Apple and
message-control builds on 961 remain historical, with their [terminal receipts](../docs/history/2026-09-10/kotodama-v1-validation.md#recovered-terminal-builds).
They do not qualify later production Core changes or the applied SDK correction.

The workspace requires the qualified normal daemon, a separately frozen ordinary
dev daemon with `test-network-message-control`, and the CLI artifact binding.
Its ordinary test selection includes the real four-validator message-control
case; the feature-gated Parliament case remains excluded. Maintained native/SDK
delivery now has the scoped passing SDK evidence above. Retained-input
continuity and admission pass for 231de5ae. The later current-source workspace
execution and ordinary build/Clippy have the scoped outcomes above; SoraFS9
passes. There is no outstanding redesign implementation or acceptance gate.
Completed a39f and 7ce7 prerequisites keep their actual source identities.

The official Apple source seal normalizes exactly the three generated literals
in `IrohaSwift/Sources/IrohaSwift/NativeBridge.swift`. The successful live-pin and
provenance checks prove the specific 9a45-to-1252 pin transition and the later
C# fixture transition to 7ce7. The subsequent official JS-transition checks
also pass on 231de5ae with the same d35b fingerprint. The later three SoraFS BIN
paths belong to the official whole-package selection, changing that fingerprint
despite compiled-library exclusion. Fresh ordinary Apple6 closes this with
`dd1683a1…` on 65021; every native output and shipping pin is unchanged. These
checks retain historical source classifications and do not establish clean-Git
publication. Workspace prerequisites preserve those execution identities. The preparation's index-refresh incident
and exact recovery are disclosed in the dated record; no uninterrupted index
preservation is claimed across that incident. The later Swift capture again passes
24 native cases but fails raw-index preservation; the overlapping daemon returns
native 0 while also failing that strict guard. Exact root recovery restores the
original bytes and mode. A subsequent Swift24 capture qualifies with optional
Git locks disabled. The subsequent normal release capture also qualifies on
7ce7, including exact raw-index preservation. The accepted input-continuity
review and workspace admission bind those retained outputs into the current
capture without repeating Rust/CLI execution solely for SDK-only edits. Earlier
failed captures are not reclassified by matching binaries or generic pass flags.

MAIN HEAD `b46c286ee62f64a81f829ae2206e81bf51918774` and PRIVATE HEAD
`ff22bda4c42219ccc055a69d7b5ce86e25b299e8` remain distinct provenance identities.

The [native publication contract](../docs/norito_bridge_release.md) explicitly
permits source-fingerprinted local integration artifacts and separately requires
clean source for release artifacts. That publication certification remains
unsatisfied; this nonpublishing implementation task does not require a commit or
clean-Git certification. The accepted source-owner audit and publication scope
are recorded in the [dated scope correction](../docs/history/2026-09-08/kotodama-v1-requalification.md#implementation-and-native-publication-scope--2026-09-09).
Compiler/Koto semantic acceptance and supplemental editor-host checks retain
their distinct scopes; no new editor-host probe is claimed.

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

## Historical qualification records

Superseded qualification prose is preserved byte-for-byte, with section digests,
in the [2026-09-08 record](../docs/history/2026-09-08/kotodama-v1-requalification.md).
The earlier [2026-09-07 archive](../docs/history/2026-09-07/kotodama-v1-redesign-qualification.md)
remains unchanged. Historical counts and unavailable temporary paths do not close
any current acceptance gate. The retained headings below preserve existing links.

### Latest compiler qualification

This superseded checkpoint is in the [dated record](../docs/history/2026-09-08/kotodama-v1-requalification.md#latest-compiler-qualification).

### Offline first-project qualification

This superseded checkpoint is in the [dated record](../docs/history/2026-09-08/kotodama-v1-requalification.md#offline-first-project-qualification).

### Latest runtime qualification — 2026-09-07

This superseded checkpoint is in the [dated record](../docs/history/2026-09-08/kotodama-v1-requalification.md#latest-runtime-qualification--2026-09-07).

### Resumed qualification checkpoint — 2026-09-07

This superseded checkpoint is in the [dated record](../docs/history/2026-09-08/kotodama-v1-requalification.md#resumed-qualification-checkpoint--2026-09-07).
