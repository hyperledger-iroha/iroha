# Epoch authority cutover after the September 22 merge

The incoming `f05f5477230c82aba0b97b3d009ba711dfbf6045` source replaced epoch-coupled
mint-finality rosters with key generations and complete epoch authorizations but
left production consumers and fixtures using removed types and fields. Its
`v2_context.rs` blob was unchanged from the preceding source. The coordinated
seven-package build6 failed with eight SCCP fixture errors before Core test
compilation; no runtime tests passed in that build.

The correction carries one complete authorization and one immutable generation
through genesis, context freezing, signing, certificate verification and recursive
mint authority. Checkpoint heads and release pins use authorization digests, not
key-generation digests. Scheduling retention keeps the exact incumbent validators
and keys while advancing a contiguous epoch interval from the previous certified
authorization. Installed beacon identity and finalized pulse verification remain
mandatory. Genesis uses its signed generation-zero template and exact interval.

The deleted next-roster parameter had no implemented replacement preparation
owner in incoming Core. It is removed from State and custom-parameter admission;
no compatibility alias, fallback, fabricated activation identity or silent roster
replacement is introduced. TODO: implement the frozen successor, all-seat custody
readiness and atomic activation before enabling production committee replacement.
That outcome remains separate from compiling the retention path.

Test fixtures use real paired-Pasta public keys and full authorization bodies with
explicit intervals. Positive later epochs have actual predecessor heights; test
fixture bodies do not claim to be authenticated DKG transcripts or certificates.
Production genesis, recovery, signing and proof verification retain their real
trust boundaries.

Validation is in progress. Build5 and build6 failures remain preserved under
`dist/sumeragi-main-work/generation167-checkpoint-binding/`. The prior 935-test
checkpoint remains historical evidence for its own captured source. TODO: append
fresh compilation, exact runtime regression results and formal source receipts
for the completed cutover. All six broader Sumeragi liveness goals remain open.

The nine-library build7 captured unchanged inputs and failed after 76.56 seconds
at the genesis decoder's missing `PublicLaneMonetaryPlanV1` argument. The incoming
staking API also changed reward processing from an asset-specific cursor to a
lane/account cursor plus exact unpaid source accruals. Consumer corrections now
carry explicit signed plans, retain the original custody sources, and keep account,
asset-definition and domain deletion guards over the appropriate retained state.
The former blanket staking-fee rejection test is replaced by exact principal/fee,
malformed-plan, opaque-execution and reserve-only controls for the implemented
signed-effect policy. These Rust controls are not yet qualified on the new image.

The private-index diagnostic formal gate passed after the complete-input and
recovery bindings were aligned. Its overall source capture changed in four
deployment Python files; it is not an unchanged-candidate receipt. The real index
was not staged, and the ordinary gate still requires the new Kura provider files
to be included in the reviewed candidate. The parser/membership controls passed
26 tests, and the retired-codec guard passed. Separate recovery review found a
missing ancestor-directory durability barrier in the proved-missing raw-artifact
path; the correction places that barrier before publishing a raw slot, with
repeated-failure controls still awaiting the shared Rust image.

Build8 failed on unchanged inputs after 308.30 seconds. Its diagnostics exposed
the remaining SCCP point representation, Core peer import, reward-controller
migration, instruction wire-identity calls and denied test function cast. Those
callers were corrected without restoring the removed APIs. Controller changes
now move the processing cursor and each unpaid recipient/source accrual while
preserving the exact custody reserves.

Build9 also captured unchanged inputs and failed after 504.81 seconds: fourteen
data-model test errors remained in explicit monetary-plan fixtures and manual
fixed-codec roundtrips, and Torii still expected the retired asset-specific reward
cursor. Cargo emitted a new Core test executable and six other completed test
executables. Scoped runtime checks of those emitted artifacts do not establish a
successful aggregate build. The exact Core executable passed all 327 selected
controls, including the original 58 review controls and expanded snapshot, Apply,
context, recursive-authorization, reward-custody and fee checks. The final source
join binds identical build-before, build-after and runtime input maps and the
binary `ecd3c02e6606dda9c82bb19a97a08273c6b0cb04aae52dcb1117a519dcb60768`.
The receipt is `generation169-merged-regressions/build9-core-review-runtime1/`.
This predates the newly identified lifecycle preview ordering correction.

The exact build9 Config executable passed 650 of 651 tests. The default context
golden failed because the staged staking default now uses the canonical universal
XOR fee asset while the recommended genesis hash still commits the old staking
asset. A diagnostic linked to the emitted libraries reproduces the old hash by
changing only that asset input. The source capture, failure and diagnostic remain
under `generation169-merged-regressions/build9-config-runtime1` and
`default-context-preimage`. The recommended model hash and Config golden now
commit the canonical XOR default together; the control also checks that staking
and fees select the same default asset. The corrected image is not yet tested.

The fifth private-index canonical structural run passed with 12,441 unchanged
inputs. The sixth passed its structural check, but its broader capture changed
only `docs/history/2026-09-22/model-enum-codecs.md`. Neither receipt asserts
production trace extraction or substitutes for including the new provider files
in the reviewed candidate.

The lifecycle audit found a separate production ordering defect: a signed manual
lane replacement could stage while economic state was empty, and a later reward
instruction in the same block could add an unpaid obligation. Checkpoint preview
then removed its record before the final retirement guard inspected it. Preview
now rechecks the unpruned World against the original predecessor before any
lifecycle pruning, returns the existing lifecycle error, and propagates refusal
through finalized publication and replay. Three signed-path controls cover an
unprocessed reward, retained dust after cursor advancement, and idempotent clean
replacement. Their compilation and execution remain pending the next build;
they are not included in the preceding 327-test pass.
The live Validate service already runs the same economic guard while preparing
the original carrier prefix, before issuing vote authority. The preview defect
therefore does not establish a current live QC stall: this correction makes
preview and replay uphold their own prerequisite rather than depend on that
earlier caller check.


Build10 failed on unchanged inputs after 247.42 seconds: a newly added Genesis
fixture used the removed `AccountId::FromStr` path. That caller now uses the
existing canonical account parser. The emitted model executable passed all six
monetary codec controls. Broader model execution exposed stale generated signed
plan fixtures and two height-context goldens. The latter are corrected from the
actual encoder for generation-zero genesis and the Retain authorization layout;
they do not arise from the separate default-XOR context correction. Generated
fixture printing is maintenance evidence, not a normal runtime pass.

The Core-only build11 passed on 7,631 unchanged inputs after 491.53 seconds, using
the explicit Core features selected for the aggregate build. The actual transitive
artifact/feature closure is recorded; this does not establish feature-closure
equivalence with the nine-library aggregate. Its binary is
`d765f04d80c10b9cc33824b9e27e4476c7c627467bbeb5261141ef89fa0549b0`.
The review selection passed 330 of 331 controls on identical build/runtime maps.
The new late-dust preview fixture failed before the guard because its signed
claim lacked committed NPoS epoch parameters. The fixture now seeds that required
state before its authenticated predecessor; every assertion is retained and
re-execution is pending. The late-reward refusal and clean idempotent preview
controls passed. Exact receipts, failure and correction hashes remain under
`generation169-merged-regressions/build11-core-review-runtime1/`.

The independently selected 80 receipt/custody controls passed on the same build11
image, including the complete-consumer regression which previously consumed the
injected directory fault after an innocent current-payload absence probe. The
corrected read-only physical acquisition preserves its original mutation epoch.
The paired measurement controls also passed, but showed no meaningful overall
speedup; correctness evidence is separate from performance evidence.

## Build 12 consumer checkpoint

`generation167-checkpoint-binding/checkpoint-build12` completes the nine-library
`cargo test --lib --no-run --locked --offline` command successfully in
968.131 seconds. Build log SHA-256 is
`1556e38ea37ce628614950812ef686da91311e2daff00b8cc89d45d3b381526f`.
The corrected dust fixture and canonical-XOR Config context now pass: Core
review selection 331/331; full Config library 651/651. Their runtime directories
under `generation169-merged-regressions` contain final source joins, binary and
log hashes, and the actual compiler feature closure. All 7,631 listed input
hashes, branch, HEAD and index match across these builds and runs. The capture
omits the two artifact OpenAPI mirrors and is not complete compiler-input or
release sealing.

The disjoint 2,566-control Core migration run subsequently completed with the
20 actual failures classified below. Passing compilation does not close these
failures, prepared successor activation, production Native ownership cutover,
or the required unchanged four/seven-validator qualification.

## September 23 small repair candidate

Build12's disjoint Core migration selection is terminal: 2,546 successful
controls and 20 actual failures. The original harness summary counted three
successful child-process wrappers as failures because it expected exactly one
libtest result row; original logs and summary are preserved, with separate
classification evidence. Listed source inputs and Git metadata remained unchanged.

The small repair candidate preserves the original assertions while restoring
complete fixture obligations: matched beacon/mint-authority intervals, actual
3-of-4 lane certificates and application receipts after global governance
carriers, retained stake custody and aggregate reserves, original two-lane Kura
catalogs, exact frozen governance escrow, and native replay-index publication.
SCCP establishes initial authenticated storage before finalized artifacts are
inserted. The PLAIN execution path now performs its existing typed-proposal
exclusion before loading a standalone referendum policy. Strict asset parsing
retains its precise whitespace refusal. The final two generated instruction
rows and all three canonical OpenAPI mirrors are corrected. No legacy fallback,
validation bypass, fabricated availability, staging or unsigned commit is added.

`checkpoint-build13` failed with two test-code typing errors: the State network
accessor name and an ambiguous escrow quantity conversion. Both are corrected.
A concurrent merge changed 241 listed inputs, HEAD and the index during that
build, so its artifacts cannot validate the current checkout. Follow-up inspection
also repaired the beacon interval/call-site conflict and a duplicated mint-finality
test declaration without dropping assertions. Formatting completed successfully.

`checkpoint-build14` completed with compiler failures after the merge. It is a
diagnostic receipt: a concurrent context edit and subsequent repair edits changed
its captured inputs. The diagnosed production issues include test-gated exports
now required by retained publication, a missing local-refusal import, a Waker
borrow, and account rotation removing claim keys from the accrual table. These
corrections are applied. Authentication fixtures now use the canonical epoch
constructors and an actual signed beacon pulse; old shadowed helper calls are
removed. Remaining Native dispatch fixtures are being migrated with real actor
backpressure and independent lane custody. The next focused Core selection
includes all 20 earlier actual failures, their related modules and repaired Native runtime boundaries. The 124
pending-membership formal checks pass (`generation172-small-compile-repair/`
contains the JUnit receipt); this is separate from Rust/runtime qualification. The large
staking preparation/SDK/Torii, explicit public-XOR and earlier live-guard test
drafts remain separate and unapplied. The new source capture includes both
artifact OpenAPI mirrors; the older build12 receipt remains scoped to its original
7,631 listed inputs. Fresh compilation and regression results are pending; the
full original liveness and network qualification goals remain active.


`checkpoint-check15` passes the Core production library check with the recorded
nine-library Core feature set. Four listed inputs changed concurrently, so this
is diagnostic compilation rather than an unchanged candidate receipt. The
legacy-codec gate and four exact startup/candidate source checks pass separately.
The Native dispatch correction now attempts one already-issued output before
fresh physical ingress. Backpressure retains original packet/ticket custody and
still permits ingress, clocks and asynchronous workers. Its three restored tests
use real four-validator source state, body/WAL workers, local Commit generation
and the actual bounded actor; actor closure must fail before removing the next
physical row. The patch and source hashes are retained under
`generation174-native-dispatch`; test compilation and runtime are pending.


`checkpoint-build16` completed the nine-library test compilation with exit 0
in 925.96 seconds. One captured source (`sumeragi/mod.rs`) and the index changed
during compilation; the retained artifacts are diagnostic, not qualification of
an unchanged candidate. Its selected model controls pass 435/435 and the Config
library passes 651/651. Separate final-source joins explicitly record the drift.
The 438-control Core repair run is still in progress. Its three restored Native
dispatch controls currently fail at the original fair-ingress ownership handoff,
before the output-order assertions; this remains a live defect under diagnosis.

The follow-up gossip boundary audit found that dummy-probe sizing no longer
measured a large frame after canonical cached-body validation was enforced.
The applied correction uses a bounded maximum search over the actual canonical
NetworkMessage and signed P2P envelope size counters. New regressions materialize
and authenticate real direct/broadcast frames at the 256 KiB boundary and retain
original queue/Kura custody across refusal. The original 160 KiB control remains
intact, and the body-substitution control now supplies a receive deadline so it
reaches authentication instead of returning before it. Two mint authorization
head/retention controls removed during the merge are restored alongside the newer
bootstrap control. These follow-ups require fresh compilation and runtime.


Build16's repair selection is terminal: **430/438 pass**, including every one
of the earlier 20 actual migration failures. Five failures share the omitted
Native family in `FairV2IngressOwnershipEvidence::matches_message`; removing only
the concurrent correction from current source reconstructs build16's exact
initial `mod.rs` hash. The correction is preserved for build17. Two QueuePlan
owner fixtures incorrectly changed the execution policy after parent finality;
an off-tree follow-up establishes both lanes before producing the parent chain.
One control-only Native publication regression remains under separate diagnosis.
The original logs and failing source joins remain intact. Build17 includes the
exact gossip sizing and restored mint/Native assertions; its results are pending.


`checkpoint-build17` passes nine-library test compilation in 224.78 seconds
with all 7,665 listed source inputs unchanged; the Git index changed separately.
Model 435/435, Config 651/651 and Torii 153/153 (original 21 plus the complete
OpenAPI module) have successful final source/binary joins. The twelve urgent
Core controls pass 10/12: all three actual Native dispatch cases, the original
160 KiB certified gossip and its non-vacuous substitution refusal, frame-cap
refusals, and all three mint authorization controls pass. Both new signed-frame
boundary controls correctly detect another missing prefix: Norito `Arc<T>`
length-delimits its owned value before the NetworkMessage enum field delimiter.
The count now includes both prefixes; those failures are retained, not relabelled.

Follow-ups are applied: all thirteen QueuePlan owner fixture call sites install
both lanes before finalizing parents, with all original assertions and a new
policy-continuity control; control-only publication explicitly preserves the
original destination-asset absence; an additional Native owned-ingress control
requires exact signed Control/Decision originals and rejects valid substituted
messages. Independent review found no QueuePlan candidate defects. Fresh build
and runtime remain required for these follow-ups. All liveness/network release
goals remain open.


`checkpoint-build18` passes all nine library test builds with source, HEAD and
index unchanged. Final joins pass for model 435/435, Config 651/651, Torii
153/153 and all **19 urgent Core controls**. These include every one of the eight
build16 failures, both real signed gossip boundary controls, preserved 160 KiB
and substitution cases, exact Control/Decision ownership, and the restored mint
and QueuePlan policy controls. The reviewed final cleanup removes four unused
mutable bindings and an unreferenced test-only route helper; no assertion or
production behavior changes. The next build reruns the original review selection
plus every QueuePlan control and the added ownership regressions. Full workspace,
complete resource admission and real four/seven-validator campaigns remain open.


`checkpoint-build19` passes nine-library compilation in 81.16 seconds with
source and Git metadata unchanged. Model 435/435, Config 651/651 and Torii
153/153 retain successful final joins. The expanded Core review is terminal at
361/364, including **all original 58 review controls passing**. The three failures
are obsolete claim-state wording in tests that correctly receive the stricter
unpaid-accrual custody refusal. Only their expected diagnostic phrases change;
all preserved-state, exact accrual/reserve and post-settlement deletion assertions
remain. Their original failures and source receipt are preserved. Build20 will
verify these precise expectation corrections and repeat the full selected review.

The next real-process gate is the current `sumeragi_v2_runner_isolated` target:
authoritative four-validator genesis, stopped initial leader, acknowledged
quorum Hold/release, four-validator restart/final transaction and seven-validator
two-outage restart/final transaction. Same-source normal and message-control
daemons must be built and pinned explicitly; the harness can otherwise reuse
stale artifacts. Current controller coverage lacks Native Control/Decision
selectors, so existing global network cases cannot qualify Native loss/recovery.
The bounded controller extension is being prepared separately; no network pass
or release claim follows from the Core fixture tests.


`checkpoint-build20` completes all nine library test builds in 84.31 seconds
with the 7,665 listed inputs, HEAD and index unchanged. Final source/binary joins
pass for **364/364 expanded Core review controls**, all **22/22 urgent Core
controls**, model **435/435**, Config **651/651** and Torii **153/153**. The Core
review retains all original 58 reported-regression controls and every QueuePlan
control; the urgent selection includes the three repaired unpaid-accrual
diagnostic expectations without weakening custody or post-settlement assertions.
Original failing runs remain preserved. These captures cover their listed inputs
and actual compiler feature closures, not every environment/compiler input.

The real-network integration target is being checked separately. No daemon or
network campaign has run in this checkpoint. Release/evidence builds must use
the repository's release/deploy profiles, not diagnostic `local-release`, and
the production source seal must not bypass the current active Git operation.
Complete resource admission, successor activation and unchanged four/seven-
validator qualification remain open; no L1–L6 or release goal is closed.


The fresh canonical pending-membership formal regression run passes **124 tests**
(625 deselected) in 298.33 seconds, including the original current-owner ledger
acceptance control. `checkpoint-network-check21` checks the actual isolated
network target successfully in 460.06 seconds with every listed source unchanged.
HEAD/index changed independently during that check; the merge marker subsequently
disappeared. The native-artifact source manifest now succeeds, but no clean
release identity, sealed build or live-network result is claimed.

The feature-isolated Native fault-controller extension is applied for validation:
exact instance/height/view/phase/family selectors, one strict command/ack format
6, and real signed Control/Decision hold/release/drop custody tests. Existing
global selector tests and four integration descriptor fixtures migrate together;
there is no old-format decoder. Formatting, diff whitespace and retired-codec
gates pass. Its daemon/client test compilation and runtime are still pending.


A fresh production-path review confirms the Native cutover is connected:
`v2_runner` creates `NativeRunnerProcess`, the physical loop services its sources
and polls it, the native candidate consumes driver Decisions, and the owned
validator executes and publishes the economic source before settling the original
lane Apply. Current status/goals now distinguish this implemented connection from
its still-open real-process qualification. Stale inactive-module comments are not
a reason to add a second implementation. The next Native network regression must
establish the actual initial-author fault without racing rule installation or
substituting the global leader for the independently selected Native leader.


Controller build22 preserves a real feature-specific compilation failure: the
daemon read-only-config fixture omitted `test_network_production_beacon_custody`
from `Args`. It now supplies the same cfg-gated `false` as the other initializers,
without changing its byte/mode/inode preservation assertions. The test-client
controller selection passes 21/21 on build22's emitted executable; aggregate
compilation did not pass. The `iroha3d` wrapper has zero tests and the strict
runner refused it; actual daemon tests belong to the `irohad` library. Build23
compiles both actual libraries and requires the repaired config control alongside
every original global and new Native controller test. Its results are pending.


`checkpoint-native-controller23` passes both actual library test builds in
46.94 seconds with all listed source, HEAD and index unchanged. Final source/
binary joins pass for **30/30 daemon controls** (every controller test plus the
repaired read-only config fixture) and **21/21 test-client controller controls**.
This includes all eight Native wire kinds, real signed Control/Decision custody,
exact instance/height/view/phase/family separation, malformed-coordinate refusal
and the existing global selector/acknowledgement cases. The updated integration
consumer target is being checked separately. These are unit/runtime controls,
not a four-validator packet-loss or silent-author network result.


`checkpoint-network-check24` successfully checks the updated isolated integration
consumer with its four strict descriptor fixture updates; captured source and Git
metadata remain unchanged. The next build uses the normal `release` daemon profile
and actual build metadata, without the local-fast metadata override. It will
capture the official native-artifact workspace manifest and retain its binary
identity. This is preparation for real-process regressions, not a clean signed
release/source-seal claim.


## Genesis runtime failures and retained Apply correction (2026-09-23)

The real-metadata release daemon builds `normal-daemon25` and
`normal-daemon28` pass, as do the standard-profile isolated network harness
builds `native-harness26` and `native-harness27`. Build28 and harness27 share
the native-artifact workspace manifest
`d1af847f1abd45fcb78b5cc65e76f3d00baf1ea9edca29d86d91c3436eb66441`;
the daemon hash is
`b711dae0e2064ee2a83d06160cae462d122742a4693534940263269b259d50f0`
and harness hash is
`a08aba229148c815143e495d001e2a0b3291e79ea4180b524a30e2f414181a3c`.
Each capture retains its exact commands, source observations and binary under
`dist/sumeragi-main-work/generation188-real-process-build/`. These are local
artifacts, not a clean signed release.

Two unchanged-source controls fail and remain retained. `restart-fixture28`
fails during actual signed four-validator genesis pre-execution, before any
daemon starts: Genesis-scoped staking incorrectly demands NPoS parameters,
which permissioned genesis prohibits. The correction accepts only initial
height one and signed expiry one under authenticated Genesis scope. Network
scope retains its committed NPoS validity window; exact transfer and custody
checks remain mandatory. The four/seven-validator fixture now configures the
actual `3f + 1` lane geometry and explicitly requests funded lane authority;
it must pass on the normal test stack.

`native-silent28` launches four real NPoS validators but times out after
240 seconds during genesis startup. All four retain the same CommitQC at
height one, view zero; none reaches Applied. The silent-author outage and sole
Native transaction are never reached. Source and both artifacts remain
unchanged. The retained logs show one pending application with empty I/O,
and a sample of one test-owned daemon shows an idle I/O worker. The parked
recovered Prepare Broadcast is an intentional durable retention, not runnable
work and not authority to release it early.

Source tracing identifies a deferred-Apply retry gap: consuming ApplyDeferred
removes its physical completion, but the only retry was inside a drain that
the lifecycle pre-gate never calls for an empty completion queue. A released
dependency therefore needed unrelated completion traffic. The old probe also
dropped its ReleaseFuture immediately, canceling its wake registration. The
correction services the same retained task at authenticated Completion rank
and keeps its original release future alive across turns and backpressure.
The dedicated lifecycle Apply path uses the same retained dependency, replacing
its identical canceled-wake probe while preserving the guarded result and exact
queue acknowledgement. The starvation census now identifies a retained Apply's actual dependency.
The failure logs did not expose that original dependency, so this source
finding is not yet proof that the complete runtime failure is repaired.
Focused regression execution and a fresh matching-source network rerun remain
pending. L1–L6, full resource admission and unchanged four/seven-validator
fault qualification remain open.


The fresh pending-membership formal selection passes **124 controls** (625
deselected); an earlier invocation retained 39 temporary-directory setup errors
because its requested parent directory did not yet exist, then passed after
that invocation issue was corrected. `checkpoint-apply-retry29` compiles Core's
library tests successfully, but an external merge changed 90 captured inputs
and Git metadata during compilation. It cannot qualify an unchanged candidate.
The transient conflict was resolved without root index/branch mutations; the
new retry and genesis corrections survive. The three new staking controls pass diagnostically. The three new pre-gate
controls fail in fixture setup before their Apply assertions: their chosen
committee selects the local proposal-author path while the reused cold helper
requires remote certified-body Fetch. They now use its existing view-zero
committee and assert that remote-author precondition explicitly. A subsequent
capture must include
the reconciled source plus the dedicated live/recovered Apply wake regressions.
The latter retain every existing finality assertion and verify release wake,
foreign-release rejection, exact task/acknowledgement custody under queue
saturation, and successful requeue before their original application step.


## Retained Apply runtime and single validation owner (2026-09-23)

`checkpoint-apply-retry30` compiles the Core library test target successfully in
405.01 seconds with every captured Rust/build input unchanged. HEAD remains
unchanged; an external operation changes the index, so this is not an unchanged
Git-metadata or sealed-release claim. The source-bound 563-control runtime
selection passes 561 and fails two structural tests. All eight priority controls
pass: the three Genesis/Network monetary-lifetime cases, all three ordinary
Completion retry cases, and both live/recovered lifecycle Apply cases. The
original 58 reported-regression controls remain selected. The initial selector
stops before test execution because five monetary/reward tests were renamed by
the external merge; their current assertions were reviewed and the exact new
names retained in the subsequent selection.

Running each lifecycle source contract separately yields 49 passes and six
failures. These include an actual first-release surface defect: both detached
Validate and its dispatch still compile the old scalar callback API alongside
the production retained execution method. The scalar consumers are unit fixtures.
The correction moves those adapters into that test module and gates the two
scalar body-store execution/replay callbacks with `cfg(test)`. Production keeps
one retained execution path. The source contracts now follow the real
store/service pair from recovery through validation and Apply, while preserving
single-execution counts, exact lineage and post-fsync settlement ordering. The
separate worker lineage test is updated to inspect the concrete Native
publication method, authenticate both lineages before publication, retain the
original deferred task/refusal and consume one actual publication.

The independently prepared seven-file patch is retained in
`dist/sumeragi-main-work/generation198-merged-review/` with SHA-256
`de5b39bd11082d6f8444407b4b53f753032a691e9b48f2d4986a82e4d9eb8559`.
All exact bases and candidates match at application, and the Git index is
unchanged by that application. An isolated execution of the repository's exact
source-string checker reproduces the six baseline failures, accepts all 55
candidate contracts, and rejects six weakening mutations. This is structural
source evidence; the actual Core test build and runtime rerun remain required.
`cargo fmt --all` finishes before `checkpoint-retained-owner32` begins.

Two Python recovery gates also still require scalar replay. They now share a
qualified-method contract requiring the original verified context, exact store,
retained replay before one promotion, and transfer of the same service. All 18
focused positive/negative cases pass. Four existing whole-consumer mutation
cases stop in fixture copying because the reviewed include inventory still
references a retired QueuePlan module; those setup failures remain retained,
and inventory alignment is in progress. The pending-membership formal rerun
on the reconciled source separately passes all 124 selected controls.

The real-metadata network harness attempt `native-harness31` stops before Cargo:
the official native-artifact source manifest rejects the active Git merge.
No bypass or mutation of that operation is used. Fresh real-process validation
of the Apply correction, the four/seven-validator genesis fixture and the silent
initial Native author remains open. The prior unchanged-source genesis failure
is not promoted into a passing network result by these unit/source checks.


`checkpoint-retained-owner32` fails with three `E0624` errors, all in worker
unit fixtures calling the moved scalar dispatch helper from a sibling module.
Its source and metadata remain unchanged. The helper now has visibility only
within Sumeragi under its existing `cfg(test)` impl; neither production scalar
execution API is restored. The next compile must qualify this correction.


`checkpoint-retained-owner33` passes Core unit-test compilation in 319.58 seconds
with captured source, HEAD and index unchanged. The joined
`build33-retained-owner-runtime1` executes all 663 selected controls successfully:
the original 58 review controls, complete body-store, worker, Apply and registry
selections, all 55 lifecycle structural contracts and the Genesis controls. All
eight priority Apply retry/wake and monetary regressions pass. The test binary
and selected source are unchanged through the final join.
`checkpoint-retained-production34` also passes production Core library checking
in 204.14 seconds with unchanged captured inputs and Git metadata; the retired
scalar validation callbacks remain test-only. These local loops use the
repository-supported stable metadata mode and do not qualify release artifacts.

The formal include inventory now follows the actual lexical modules and order,
removes the retired QueuePlan admission handoff, and names every reached provider.
Its independent literal mirror and canonical digest are updated together. All
44 loader and 112 inventory controls pass, and the 124 pending-membership controls
pass on that inventory. The four broader successor mutation controls then expose
two additional fixture roots missing from their explicit inventory. Both are now
named explicitly, preserving the unique exact count; no missing-source fallback
is introduced. Their initial complete-fixture run retains 86 diagnostics (66
unique), including missing roots and stale Native lifecycle contracts, in
`generation199-retained-replay-contract/`. The broad baseline is not passing.

The recovery factory gates now bind the qualified authenticated factory, preserve
the existing Kura/State/WAL/body-root identity checks, and require the exact
verified context, returned replayed store/service pair and original service
transfer. Both consumer gates share this contract, including single-construction,
single-replay and single-transfer counts. The retained Serve relation follows
the same transition, and lifecycle storage requires NativeApplyService. All 46
focused replay/factory positive and mutation controls pass. Broader successor
contracts and real-process validation remain open; the active merge still
prevents the official release source manifest from admitting a new network build.


Completion source checks now require one retained Apply retry after runner-rank
authentication and before parked/physical classification. The retired registered
sidecar path is forbidden; all five current ordinary-cursor exits remain counted,
and the exact Proposal Sign permit remains required. A negative test initially
exposed that this focused subsection did not reject an erased permit predicate;
the semantic order check now does. The failed 56/57 receipt is retained, followed
by a successful 57-control replay/factory/Completion run.

Recovered Sign gates previously located their owner using the following Apply
comment. They now select the exact defining type and methods, preserving guarded
result custody, exact queue transfer and projection, queue acknowledgement before
guard disarm, and fail-stop abandonment. The combined replay/factory/Completion
and Sign run passes all 91 controls; the existing Serve-directory suite passes
all 30 controls. The six reviewed Ready Proposal Sign item seals and their
literal mirror are aligned with current Native owner signatures and original
Completion priority. Eight focused controls pass, including semantic mutations
after resealing, and all three named runtime regressions pass on the unchanged
build33 artifact with a final source join. The broader successor gate remains
unqualified; its latest complete scan before these final repairs records 66
diagnostics, retained in `generation199-retained-replay-contract/`.

The standard local integration harness build `checkpoint-genesis-fixture35`
passes in 374.55 seconds with captured source and Git metadata unchanged. Its
exact signed-genesis fixture constructs and verifies both four- and seven-peer
committees successfully in 7.43 seconds on the normal test stack. No daemon is
started. The source, binary and metadata remain unchanged through the run;
`generation188-real-process-build/signed-genesis-fixture35-local1/summary.json`
records this deliberately limited local scope. The official real-network source
manifest still rejects the active merge; no stable-metadata artifact is admitted
as release or real-network qualification.


The complete successor production source-fidelity scan now reports zero errors
on unchanged captured Rust, checker, HEAD and index inputs (93.39 seconds;
`generation205-neutral-apply-contract/successor-final.json`). This follows 46
pending-Kura controls, 31 retained-Apply controls, 15 exact live-census controls
and 61 Native runner owner controls. The 23 existing lineage mutations pass
across the retained 22-pass/one-failure run and its corrected one-test rerun;
the failed receipt remains recorded. These are executable source contracts and
mutation tests, not a complete formal proof or network progress evidence.

The local-release daemon build `checkpoint-local-diagnostic-daemon36` passes
with unchanged captured Rust and Git metadata. Its exact-source join with
harness35 produces the real four-peer `native-silent36-local1` diagnostic,
which fails after 250.50 seconds during genesis startup. One validator applies
height 1; three retain the decided genesis application with a durable checkpoint
but no final witness. The Native author-outage portion is never reached. The
source, binaries and Git metadata remain unchanged throughout this failed run.
The official release/native-artifact workflow still rejects the active merge;
this documented local development diagnostic does not bypass or satisfy it.

Read-only samples of that diagnostic's own nodes show an idle Kura writer,
Apply repeatedly authenticating the original source, and the runner blocked in
its exact advert-tip read. Source inspection finds four separate publication
lease acquisitions around source authentication, witness persistence, archive
persistence and final State preparation. A queued reader can take the released
prune fence between phases and force a complete retry. The new candidate keeps
one original `SourceAuthenticatedCarrier` and its joint Kura lease continuously
through those phases. Witness staging/promotion share the same guarded Kura
writers and exact durable receipt/artifact authentication; partial failures
still release all physical owners before returning original retry custody.
Six new regressions cover uninterrupted custody with a competing advert reader,
receipt/artifact/proof substitution, exact retry accounting and corrupt evidence.
Compilation and current-source runtime validation of this candidate are pending;
the earlier passing source scan and runtime counts do not qualify these edits.


`checkpoint-single-lease37` retains the initial compile failure from the removed
archive-local Kura error variant and the new test's height type. The corrected
`checkpoint-single-lease38` passes Core test compilation in 159.92 seconds on
unchanged captured Rust and Git inputs. The 355-control runtime selection passes
354 controls, including all six new lease regressions and the original 58 review
controls. Its only failure is the existing aggregate panic-cleanup test's stale
participant count (283 versus 287); the original build33 binary reproduces the
same failure. The callback count and zero-busy assertions pass. The count repair
is prepared separately and has not been used to overwrite this failed receipt.

The corrected candidate's complete successor source-fidelity scan also passes
with zero diagnostics in 94.11 seconds on unchanged captured inputs. The new
single-lease gate and exact matching ledger entries pass 76 actual-source and
mutation controls, repeated canonically in 3.65 seconds. Its encompassing Native
preparation checker still reports 24 unchanged pre-existing mismatches; the
single-lease change resolves exactly 20 relevant diagnostics without adding any.
The remaining typed-error, retained-owner and Native source/constructor bindings
are under separate correction. The rebuilt integration harness40 passes; the
matching local-release daemon39 and network diagnostic remain pending.


The matching local-release daemon39 succeeds in 603.43 seconds and harness40
in 306.89 seconds. Their unchanged exact-source `native-silent39-local1` run
now gets all four validators through genesis and verifies the signed genesis
finality. It fails after 26.15 seconds at the first account query during setup,
before stopping the initial Native author. The original three-replica Apply
stall is no longer reproduced; the outage test has not passed. Source, binary
and Git metadata remain unchanged through this local diagnostic.

The next candidate also places the retained Kura lease before the decision's
original admission owner in `SourceAuthenticatedCarrier`, so abandonment and
unwind unlock the complete boundary before value cleanup callbacks. A real
four-validator fixture probes both drop paths. The aggregate cleanup regression
now derives its expected participant count from the original World probe inventory
plus the four runtime cells and transaction membership; zero-busy and exact wake
checks remain intact. The account-query helper now preserves the SDK query's
underlying error instead of hiding it under `SingleQueryError`. Compilation,
356 selected Core regressions and another exact-source network diagnostic are
pending. An initial harness43 invocation named the source include instead of
the registered isolated test target and failed without compiling; harness44
corrects the command. Neither failure receipt is overwritten.


`checkpoint-single-lease41` passes Core test compilation in 173.70 seconds.
All 356 selected Core regressions then pass, including the new drop/unwind
control and repaired aggregate participant-count check, with an unchanged final
source/binary join. Matching daemon42 and harness44 also compile successfully.
Their exact-source `native-silent42-local1` again applies genesis on every peer,
then exposes the query's actual HTTP 400 diagnostic: ordinary `FindAccounts`
has no source-specific bounded adapter. Its unchanged failure remains retained.
The visibility assertion only needs identities; the next harness uses the
existing bounded `FindAccountIds` producer and propagates every cursor failure.
Two tests distinguish exact presence, complete absence and failed traversal.
This does not implement the remaining ordinary account-metadata adapter, close
process-memory admission, or qualify Native author-outage progress.

### September 23: actual Native author outage reached

The captured local daemon45 and harness46 builds pass on matching Rust inputs.
All four focused identity/cursor/SDK-worker controls pass. The setup assertion
uses the existing bounded `FindAccountIds` producer; errors remain errors, and
this does not implement the separate account-metadata query adapter.

`native-silent45-local1` fails in 217.12 seconds: signed genesis finality verifies
on all four validators, the predicted first Native author exits, and the three
survivors admit the only submitted input. All remain at global height 2 with the
input queued; execution finality exceeds the unchanged 180-second outage bound.
Source inputs, copied executables and Git metadata remain unchanged throughout
the run. Its retained peer logs and WAL files are under
`dist/sumeragi-main-work/generation188-real-process-build/native-silent45-local1/`.
No restart or execution-finality success is claimed. The 356-control Core pass
and successful genesis Apply establish their scoped boundaries only; L1–L6 and
the unchanged four/seven-validator qualification remain open.

### September 23: Native wire classifier failure isolated

The unchanged local retry `native-silent45-local2-debug` reproduces the actual
outage failure in 216.82 test seconds. Its environment logging filter was stripped
by the test launcher, so it is not DEBUG coverage. The next captured daemon47 /
harness48 run sets the filter in the fixture configuration and fails in 213.67
test seconds. All three survivors report `unknown Sumeragi block discriminant`
from the mandatory P2P credit stream. The raw classifier only admitted global
and retired lane tags 0–10, although the actual outbound Native protocol uses
11 and 12. Direct driver tests manually forwarded decoded messages and therefore
bypassed the broken boundary. The current candidate admits both Native tags,
checks the Native envelope revision and payload kind, and installs explicit
control-frame/sequence/depth/allocation decode limits. Its tests exercise the
real nested NetworkMessage/BlockMessageWire classifier and bounded decoder for
all five Native controls plus Decision, ten layouts and every admitted exact
committee size, with malformed and oversize controls. Build/runtime validation
of this candidate is pending; no outage success is claimed.

The four-file Native formal alignment is applied from reviewed patch
`88bc51fe9ea9c1c393f7166052538739229ee4a6f697ef4e67d33315e587b453`
without index changes. All 56 changed canonical controls and five additional
uncached generic/prepublication controls pass with stable captured inputs.
Direct Native preparation, generic Native model and Native prepublication
consumers each report zero diagnostics. The full successor source-fidelity
consumer independently reports zero in 92.10 seconds, with an unchanged
source capture. The earlier intermediate 1,244-test run remains 1,241 passed /
three stale-anchor-or-spelling failures, subsequently repaired in the scoped
canonical run; it is not a final full-suite pass. Separate passive recovery
contracts still name retired lane scheduling and remain open.

### September 23: real-wire admission and post-idle execution clock

Captured Core build52 and its unchanged binary/source join pass all 303 selected
controls, including the original 58 review cases, Native raw-wire advertised
layouts, descriptor retirement, exact completion ownership, and Apply controls.
The layout matrix compares each independently framed nested message; Norito
legitimately omits requested layout flags unused by a given message shape.

Matching local daemon50/harness51 clear the earlier raw tag 11/12 transport
rejections. The real four-validator silent-initial-author run reaches a terminal
rejection at global height 3 in 74.44 test seconds. Read-only canonical decoding
of the retained block identifies `native input creation time must precede its
actual carrier`. The global timestamp rule considered ordinary transactions but
ignored Native economic inputs, so an input submitted after an idle interval
could be later than its own carrier. This is a distinct observed root cause;
the run does not establish successful execution or outage qualification.

The clock correction shares a checked millisecond floor between proposal
assembly and static validation: max(parent time plus cadence, each retained
timed Network input plus one millisecond). Native prefix fitting recomputes
against the original baseline, excluding deferred inputs. Complete admission
controls do not execute and cannot advance the carrier clock. Overflow refuses
before signing; sealed reveals use their signed transaction's creation time.
Core build53 and focused clock regressions are in progress. Local daemon
diagnostics remain separate from release qualification while the existing Git
merge blocks the official release workflow.

The captured Core53 build passes compilation. Its exact-source runtime run
passes 363 of 367 selected controls, including ordinary/snapshot clock checks,
sealed-input and overflow cases, and all original review controls. Three new
future-input cases fail fixture authentication before assembly because the
fixture changed admission time without rebuilding its journal claim; the
prepared correction constructs a complete fresh binding before signing. One
existing closed-owner fixture assumes a physical drain has completed after a
fixed poll count; its original-owner wait is being corrected. No successful
post-idle candidate test is claimed from this run.

The canonical passive-recovery contract passes all 77 focused controls with
unchanged Rust and contract bytes. A separate current-source check exposes three
stale generic Autonomous tokens and six terminal-binding diagnostics. Aligning
the pending-tip original-owner transfer, locked-body reconciliation before
activation, and Native candidate-source retention clears all nine diagnostics
in an unchanged-source probe. The 32-case terminal/mutation regression is in
progress. These structural checks do not prove eventual source delivery.

The corrected canonical terminal/mutation suite passes all 32 controls with
unchanged sources in 85.29 seconds. Its new generic-producer mutation fixture
explicitly copies the current runner owner before checking the actual generic
consumer. Matching unchanged daemon54 and harness55 compile successfully; the
four-validator author-outage/restart diagnostic56 is running.


### September 23: successful finite input exposes restart metadata omission

The unchanged local daemon54/harness55 diagnostic, retained as
`generation188-real-process-build/native-silent56-local1`, executes the sole
finite input successfully on all three surviving validators. It verifies the
original view-zero author is offline and the exact later-view Native CommitQC
has three authenticated shares. Restarting that author fails strict recovery at
height one: finality exists without a complete checkpoint-bound manifest. The
89.50-second test reports zero passed and one failed; its exact source/binary
join does not turn this failure into outage/restart qualification.

All four captured stores have canonical bodies, WSV checkpoints and finality;
none has commit manifests. The retained publisher's Decided phase omitted that
writer and ordered finality before the checkpoint. The current correction uses
the original captured checkpoint and verified decision, orders body, checkpoint,
authenticated manifest plus digest binding, finality, then exact writer receipt.
It preserves strict rejection of finality without complete metadata and does not
repair or rewrite the captured failing stores. A retained-owner regression now
injects each preceding write failure, checks no State visibility or ownership
replacement, invokes strict startup planning at each cut, and retries the same
execution. Runtime validation is pending.

Core57 failed compilation on a private BodyCustody recovery field; the correction
uses its existing defining owner's source-recovery accessor. Core58 captures
that correction together with publication ordering and the corrected signed
clock fixtures. Neither compilation nor current runtime completion is yet claimed.


### Current retained publication durability candidate

Core59 compiles on unchanged inputs. Its captured runtime selection passes
474 of 477 tests with unchanged source, Git metadata and binary. All original
58 review controls, the actual retained-owner checkpoint/manifest/finality cuts,
existing-finality resync refusal/substitution cases and three actual source-request
closure controls pass. Three newly added clock fixtures fail: their nominal
10,000/20,000 millisecond times precede the actual parent near 1,700,000,000,100,
and the fixed first transaction hash defeats the bounded second-hash search.
The next fixture uses explicit offsets after its real parent, derives expected
clock values from the authenticated retained inputs, and searches a deterministic
small first identity before initial admission. No signed input is mutated after
certification. The failed receipt remains preserved; it is not a passing join.

The manifest writer additionally synced its own directory but not the directory
entry in its parent before publishing the checkpoint's manifest digest. That
pre-finality cut could retain a reference to a lost newly created directory.
The current correction holds and syncs every original ancestor through the Kura
root before binding the checkpoint. Its regression forces each of four actual
directory barriers after the real manifest rename, requires an unbound checkpoint
and recoverable pending tip, then retries. Finality retry likewise resyncs the
exact verified file and all held ancestors, rejecting changed bytes or identity.
The existing-finality tests now force all four ancestor barriers.

The corrected source-request formal gate passes 87 canonical controls and two
uncached generic consumers on unchanged snapshots. The originally reviewed
pending-membership ledger control passes independently on current declarations.
Four historical checkpoint controls have moved from the dormant Apply adapter
to genuine Native retained publication and immutable captured State. The dormant
Apply chain itself remains compiled and unretired: its remaining fixture and
formal migrations are open, so no removal or current-source proof is claimed.
Core60 now captures the fixture, directory-durability and checkpoint-test changes.


Core60 compiles unchanged and its captured runtime reports 529 of 530 tests
passing, including all three corrected clock controls, four migrated checkpoint
controls and all four finality ancestor-sync cuts. The sole failure is fixture
setup: the new standalone manifest test invokes strict startup planning on blank
Kura without first admitting its physical geometry. Generation222 prepares the
existing authenticated initial-geometry helper before the block append and fault
injection; it preserves all strict planning and durability assertions.

During that runtime, another writer completed the merge to HEAD
`ddaa8ede8818912b864aaad73566c6f4cc3a87f0` and changed CLI public reset, Core
beacon/readiness, Kagami localnet and two Torii test sources. Our run's final
source/Git join therefore fails even though its retained executable is unchanged.
The 529 passes qualify that captured executable only. No other task was contacted
or interrupted, and no concurrent edit was reverted. Daemon61/harness62 are
allowed to finish naturally; fresh source/artifact joins are required afterward.
The earlier active-merge blocker is now historical, not the current Git state.


### Current source join and signed-genesis lifecycle retirement

Core63 compiles and all 530 selected runtime controls pass. Its final receipt
joins the emitted and retained binary to unchanged Rust/config/fixture inputs,
branch, HEAD and index. This includes every original 58 review control, all
manifest/finality ancestor-barrier cuts, corrected Native clocks, physical
publication, source-request retirement and the migrated checkpoint controls.
Formatting and all five retired-codec guards pass. Daemon64 and harness65 also
compile on that same unchanged capture. Daemon61/harness62 completed naturally
but remain unqualified because their inputs changed during the earlier merge.

The matching local four-validator `native-silent64-local1` diagnostic again
successfully executes its only transaction on the three survivors and verifies
the later-view Native quorum. The restarted author now passes strict complete
Kura replay at genesis, proving that the missing-manifest failure is cleared.
The test then fails at the next lifecycle boundary: `CompleteTip finality has
neither an exact Apply lineage nor a canonical physical predecessor frame`.
Its 107.58-second receipt reports zero passed, one failed, and unchanged source,
Git metadata and executables. All test-owned processes have stopped; captured
stores remain untouched. This is still a failed outage/restart qualification.

The rejected height-one log is physically present and nonempty. Recovery wrongly
assumed signed genesis always has an empty lifecycle log; its physical-frame
branch required post-genesis rotating-leader policy. Generation224 now separates
authenticated genesis context from ledger emptiness. Existing physical frames
can retire under their exact signed-genesis or post-genesis policy, followed by
the same artifact/receipt/activation identity, Kura root, directory/frame binding,
complete ownership census and Serve authentication. Missing-frame EmptyGenesis
still requires zero high-water, no records and no debt. Tests require the actual
physical capability, reject wrong policy and missing capability, and observe
persisted cancellation with the original ordinal floor for both policies.
Core66, daemon67 and harness68 capture this correction; its runtime and fresh
network results remain pending. No L1–L6 or release gate is closed.


### Native restart qualification and historical replay closure

Core66 and its final runtime/source/binary/Git join pass all 574 selected controls,
including the original 58 review regressions and 44 CompleteTip physical-frame
and genesis recovery controls. Matching daemon67/harness68 pass
`native_silent_initial_author_finalizes_one_finite_input` in 115.448576 seconds.
The actual first Native author is stopped before the sole finite input. Three
survivors execute it under an authenticated later-view three-share NativeQC;
the stopped validator restarts from genesis, catches up, and all four peers
agree on account State and the exact result-bearing block wire.

The separate seven-validator `authoritative_v2_finalizes_through_two_validator_restarts`
diagnostic fails on unchanged source and artifacts after successful Native
economic execution. Restart rejects committed block 3 with
`ExecutionContextInvalid("native lane economic carrier is not active")` during
atomic replay prevalidation. The failure is the generic historical validator's
retired Native gate. Its captured stores remain unchanged. The next correction
shares the existing source-owned Native preflight and recorded execution before
live preparation captures its witness, retaining Native custody through the
existing finality, exact-wire, checkpoint and isolated-State replay tail.

These are local development observations, not signed release qualification.
The four-validator result does not close the seven-validator restart failure or
the remaining loss, reordering, backpressure and final-transaction matrix.
All six liveness goals remain open.


The historical replay correction now shares
`ValidBlock::validate_and_record_native_candidate` with live preparation. Both
consume the same source authentication, global preflight, recorded execution
and original Native custody. Replay authenticates the exact historical proposal
first, reconstructs successor authority from the durable parent receipt or the
exact State-authenticated snapshot record, and retains that custody through
witness, CommitQC, exact wire, metadata and checkpoint checks. Both shared parent
checks accept the authenticated snapshot anchor. The entire range still runs in
an isolated State before one final installation. Generic execution remains
closed to unowned Native inputs.

Core69 compiles and passes all 574 existing selected controls with an unchanged
final source/binary/Git join. This establishes regression preservation, not
execution of the new historical Native branch. Generation228 adds four genuine
retained-publication to replay controls covering independent and atomic Native
inputs plus late correlated checkpoint rejection without any State or Kura
mutation. Core70 exposes a missing test-only `StorageReadOnly` import; its failed
receipt is retained. The replay tail also requires custody presence to match
Native input presence; dropping an optional owner cannot silently skip its checks.

Core71 compiles but its six new runtime controls produce one pass and five
failures before exercising the intended branches: the four publication fixtures
lack the genesis domain authority required by historical replay, and the positive
snapshot fixture has a one-member State roster instead of its signed four-member
global context. Those fixtures now establish the genesis account and domain, and
the exact global roster and BLS proofs, before genesis and all signed admissions.
No State authentication or production validation requirement is relaxed.

Core72 compiles on unchanged source. All four actual Native publication/replay
controls pass, including independent and atomic late checkpoint rejection with
unchanged State and Kura. The wrong-snapshot-State control also passes. The positive
snapshot fixture correctly rejects its locally available parent body; it now uses
the existing hash-only snapshot fixture conversion before authentication. Core75
compiles and all six targeted controls pass with the final unchanged
source/binary/Git join. Its complete 580-control selection and matching development
daemon73/harness74 network qualification remain pending. These partial results do
not close the seven-validator restart or broader liveness goals.


Generation229 moves the original Native preparation contract to the shared
execution kernel and binds the live/replay consumers, exact historical authority,
source custody and full replay tail. Three actual source consumers pass on an
immutable complete source mirror. The initial 37-control run passes 36 and exposes
a lexical collision: a later staged-merge-entry lookup masks substitution of the
manifest argument. That failed receipt is preserved. The final refinement binds
the complete Native-presence predicate and exact manifest, witness, commitment
and original-wire arguments; all ten uncached positive/substitution controls pass
on its unchanged mirror. The reviewed three-file checker/ledger/test refinement
is applied with exact file hashes and an unchanged Git index and HEAD. These are
source-binding checks, not a proof of runtime progress. Final canonical consumers
and the ongoing existing publication controls retain separate receipts.

Matching daemon73 and harness74 now compile successfully with unchanged captured
Rust inputs, branch, HEAD and index. The seven-validator two-restart diagnostic
runs on these exact binaries; its result is pending. The complete Core75 selection
also remains in progress. No release or broader fault-matrix conclusion follows
from compilation or the six targeted runtime passes.


Core75 subsequently passes the complete 580-control selection with its final
unchanged source/binary/Git join. Generation229 also completes all 84 existing
publication/durability controls on its immutable mirror. Its final canonical
consumers pass four uncached checks, including the original staged-entry mutation,
and join exactly to all three applied refinement hashes.

The matching seven-validator daemon73/harness74 diagnostic fails in 87.886357
seconds with all captured inputs and binaries unchanged. It advances past genesis
but exits before the restart stage: `pleasant_antelope` fails its height-2/3
handoff with `finalized ingress cut retained physical ownership`. Native transport
has a process lifetime and its terminal selector deliberately excludes those
messages; the independent poll handles at most one physical Native occurrence per
turn. Both global rollover callers nevertheless require whole-process queue
emptiness. This conflicts with the existing exact Native retention/rebinding
path. The strict full-drain condition must remain available for full closure.

Generation232 introduces a separate authenticated closed-global cut in the
existing lifecycle ingress-position owner. It reuses release-mode structural
validation of all counters, indices, original byte allocations and occurrence
positions; rejects every remaining global/live leader owner; then authenticates
the immutable retained Native evidence, message, origin and routes outside the
State mutex while retaining service/publication fences. It mutates no queue or
process retry owner. Both global callers use that cut; the strict full-process
cut is unchanged. Added controls preserve multiple original/coalesced messages,
reject thirteen Native accounting/custody corruptions, retain the original
allocation across roster replacement, and reject all nine stale global lane
accounts under both cuts. Core76 and matching network qualification are pending.
The failure receipt and actual peer stores remain retained; no network pass or
L1–L6 completion is claimed.

Core76 compiles with unchanged captured inputs. Its expanded physical-ingress and
caller selection passes 161 of 163 tests, including the new retained-Native,
corrupted-accounting and strict-drain regressions. Two older tests fail before
their intended checks: one still expects the now-active public Native entrypoint
to reject admission, and one locates the recovered-Sign completion using a
retired missing-sidecar Apply comment. Generation233 prepares test-only repairs:
exercise public admission and duplicate coalescing, and check the exact
recovered-Sign struct and implementation separately. All ownership restrictions
remain asserted. These corrections await the next captured Core build; the
161/163 result is not a passing suite.

Generation232's formal companion passes all 199 focused uncached controls on the
captured candidate, including rejection of release-mode authentication bypasses,
the strict full-drain negative and both global caller regressions. The nine
reviewed Python files are applied with exact candidate hashes and unchanged
branch, HEAD and index. A separate canonical-consumer check is in progress.

The canonical generation232 consumers pass all 199 checks in 59.72 seconds with
unchanged source and Git files. A second post-merge capture also passes 199 in
59.42 seconds. Daemon77 exits successfully, but its build overlaps an external
merge from `ddaa8ede8818912b864aaad73566c6f4cc3a87f0` to
`b45ec0457eb0664e46c2cdfd8a4b619caefd02b9`, changing 229 captured Rust inputs.
Harness78 is unchanged on the preceding inputs. The exact-artifact network
preflight refuses this pair before starting any peer; this is neither a network
pass nor a runtime counterexample. All external changes are retained. The two
test-only expectation corrections are applied and fresh Core79, daemon80 and
harness81 captures are compiling the current checkout.

Current Core79, daemon80 and harness81 stop at the same merged DataModel schema
error: the release-manifest Revoke action uses named enum fields unsupported by
IntoSchema. Generation235 replaces them with one dedicated documented revocation
payload, updates every Rust consumer and adds binary/strict-JSON/schema coverage;
no compatibility decoder is added. The registry regression also gains its missing
cfg(test) boundary. The codec retirement guard passes. Model82 gets past that
production schema error but fails test compilation on two separate merged
KAGEMUSHA fixtures (missing authenticated app evidence and a moved borrowed
policy), before any test executes. Authentic fixture corrections are prepared
off-tree for the next source capture.

After the merge, generation229's four uncached Native-preparation consumers fail
fixture setup with 47 owner-binding diagnostics on unchanged inputs. The full
canonical multilane gate reports 545 diagnostic lines on unchanged source,
including 291 repeated refusals for the newly included Kura transaction-history
budget test. Generation236 reviews that three-test include, extends the precise
allowlist and its authenticated digest, and passes both the current-manifest and
exact Kura owner/source controls. The intermediate missing-digest failures remain
recorded. Other Native, membership, prepublication and delegated-State bindings
remain under review; no whole-gate pass is asserted.

Generation233's genuine process regression is applied for the next Core build.
It retains an actual PreparedDequeued owner under control capacity one, preserves
a third queued occurrence through the authenticated global cut, real gate
retirement/rebinding and a changed global roster, then requires all three real
timeout votes to reach Native view one at fixed time. It adds no production API
or fabricated publication authority; runtime qualification remains pending.


The post-merge production daemon87 compiles, but its source join is invalidated by
necessary test-source repairs. A fresh daemon91 then builds on unchanged source
and Git metadata. The harness88 also compiles with an invalidated source join;
harness92 is a fresh capture. No newer network pass is asserted.

Model89 passes all ten release-manifest regressions on unchanged captured inputs.
Its retained component artifact subsequently passes 15/16 enrollment/hardware
controls. The remaining test expected issuer-evidence rejection for a changed
nonce, but the new authenticated app owner correctly rejects that nonce earlier
as a binding mismatch. The fixture now asserts that exact rejection and separately
signs a detached issuer evidence commitment to preserve the issuer-boundary
negative control. No production authentication check is relaxed.

Core86 exposes two missing capability-constant imports and a missing terminal
credential app-binding witness. Those fixtures now import the defining constant
and supply both exact credential bindings; both curve suites additionally mutate
each binding separately. The pending Core90 capture includes these corrections.
The missing privacy fixture include is restored from the exact preceding source
with current Orchard/private-IVM reserve records. Its `test_*` filename had been
ignored by the repository-wide pattern; an exact Git-ignore exception now keeps
the required include visible. The production P-256 gadget also imports the trait
which defines its existing coordinate operations. Core/privacy runtime execution
remains pending.

Generation240 replaces admission-capacity checks for retired runner-side merge
selection with the actual single Native worker and authenticated preparation path.
Its 98 controls pass on unchanged captured sources, including stale completion,
second-worker, source custody, wrong State/guard, ordinary fallback and effect
clearing mutations. Exact framing and height-based optional-evidence priority
remain enforced by the existing assembler checks. This is structural evidence;
it does not establish network or release qualification.


Generation235's five-file membership/Native binding patch is applied only after
all canonical bases and reviewed candidate hashes match. Membership passes 189
controls; Native focused controls pass 137 plus one corrected, uncached free-function
extractor mutation. The final 17 uncached controls pass, including four actual
consumers and removed/commented fixture-gate and Busy-recovery mutations. That
last broad snapshot changed only in the concurrent parent-owned admission-capacity
checker; all generation235-owned files and Rust inputs remain unchanged. Earlier
failed extraction/setup receipts are retained. No Rust implementation or proof
claim is changed by this patch.


## Current Core90 and real-network checkpoint

Core/model90, daemon91 and harness92 compile with unchanged captured inputs and
Git metadata. The retained model artifact passes all 26 release-manifest and
enrollment/hardware controls. The complete selected Core runtime executes all
809 controls on its unchanged captured source and binary: 804 pass, four abort
with default-stack overflow, and the recovered-Sign source check cannot find its
retired doc-comment delimiter. The separate five-control global-cut selection
passes its four genuine runtime regressions and fails that same source check.
Both failed suite joins remain recorded; neither is a passing suite. The four
overflows are the control-only, single and atomic Native-service retained
execution/publication tests and retained-current-genesis publication.

Matching daemon91/harness92 pass the real seven-validator two-restart diagnostic
in 298.296444 seconds and the real four-validator silent-author diagnostic in
127.048639 seconds. Each runs exactly once with real-network enforcement, no
skips, unchanged listed inputs, unchanged retained binaries and unchanged Git
metadata. The seven-validator test checks economic execution before the outage,
execution with two validators offline, both authenticated historical restarts,
and the final sole finite transaction on every validator. The four-validator
test checks the unavailable initial Native author, later-view quorum, sole-input
execution and restart recovery. Receipts and peer logs are retained under
`dist/sumeragi-main-work/generation188-real-process-build/two-restarts91-local1`
and `silent-author91-local1`. Test-log SHA-256 values are respectively
`c9d34ffd080c8e6d7fb9f9f83b5e991f957c51975b6e04c571e6f5597dc3dac3` and
`50f24147b798df308488dccdd13a5ea955582422bb6046a482ae7ec747db6e9f`.
These use the documented local-release/stable-metadata development artifacts;
they do not qualify the clean signed release, all fault schedules or production
readiness.

The current canonical multilane structural gate passes on unchanged sources
(generation234 `multilane-current3`). Generation240's actual candidate/capacity
controls pass 98 tests. Its inventory interface repair passes all eleven tests;
the earlier 18/19 result is retained. Generation242 passes all 25 exact
drain/handoff controls and five newly added negative controls. Generation241
passes all 116 delegated-State controls, 78 related controls and five uncached
conversion controls. Its final join records unchanged Rust, owned checker
candidates and Git, and identifies the three concurrent parent-owned formal
applications exactly. The gate binds five refinement kernels and the composed
in-flight relation; it explicitly makes no production trace-extraction claim.
The L1–L6 goals, default-stack failures, complete process-memory admission and
full fault/release matrix remain open.


The additional four-validator NPoS leader-timeout scenario fails twice on the
unchanged daemon91/harness92 candidate (60.48 and 57.73 seconds including the
wrapper). All validators finalize genesis; three advance past height two, while
one writes height-two finality but exits through `ProductionV2Services::drop`.
The relay observes that fail-stop and exits before the outer runner prints its
initiating error. Both original failed receipts and durable peer directories are
retained (`npos-timeout91-local1` and `npos-timeout91-local2`); the permissioned
network passes do not qualify this NPoS schedule. Generation245 adds diagnostic
reporting for Native source/process and lifecycle ingress failures while their
activated services still exist. This is diagnostic work, not a claimed repair
of the NPoS failure.

Generation242's retained-stack diagnosis independently reproduces all four
default-stack failures against the unchanged Core90 artifact under LLDB. The
shared production call chain materializes all World fields in one 587,664-byte
closure, inside a 139,136-byte World journal frame, a 277,248-byte State component
frame and a 649,680-byte prepared-carrier frame. The current correction isolates
each concrete field's temporary construction in a non-inlined helper borrowing
its original slot. It reuses the admitted wrapper allocation and preserves
original capture, cleanup and notification custody. An explicit fixed 2 MiB
regression covers the actual retained Native execution/retry/revalidation path;
no stack-size increase is used. Core93, daemon94 and harness95 are compiling
this correction, the recovered-Sign delimiter repair and the initiating-error
diagnostics. No passing post-correction runtime result is asserted yet.


## Core93 and NPoS94 follow-up

Core93, daemon94 and harness95 compile on identical captured Rust inputs with
unchanged Git metadata. The focused default-stack selection passes all 137
controls, including the four former retained-publication overflows, the new
explicit 2 MiB Native body-store retry and the recovered-Sign source boundary.
Its final source/executable join is true. The broader selection executes all
810 controls: 809 pass and the terminal-finalization source test still expects
`native.poll(` before the diagnostic closure introduced a line break. That suite
remains failed; its unchanged-source receipt and failure are retained.

The matching four-validator NPoS outage/restart diagnostic passes once in
196.099495 seconds, with no ignored cases and unchanged source, binaries and Git.
Its log SHA-256 is
`a88930c83441ba862f1c2d22acdd6eb0da8e0a7c1d9c7f009f9d393ffea103cf`.
This pass does not explain the two earlier daemon91 fail-stops. A separate
historical replay of the exact old daemon91/harness92 pair also passes in
207.995141 seconds. That run inserts an off-tree library which delays only
nonzero process exit by two seconds so the initiating runner error can be
printed after fail-stop; it is diagnostic evidence, not unmodified network
qualification. Original binaries and current workspace inputs remain unchanged.
Its log SHA-256 is
`ecf4d7030e795275213150e78233234cbe0da366f07e55cb040caea8db831b09`.
The intermittent failure remains open pending a concrete initiating error and
regression.

The current canonical multilane gate (`multilane-current5`) passes on unchanged
sources after generation243 aligns the sole Autonomous runner ledger row and
passive consumer with the exact diagnostic wrappers. All 16 companion controls
pass, including changed arguments, lost error propagation and injected control
actions. No proof or production trace-extraction claim changes.

## Core96 assertion closure and subsequent queue counterexample

Core96 corrects the Native-poll source assertion without changing its required
arguments or error propagation. All 810 selected controls then pass with an
unchanged source/executable/Git join. Daemon97/harness98 separately pass the real
seven-validator two-restart diagnostic in 235.197038 seconds.

The subsequent [Native/global ingress record](../2026-09-23/native-global-ingress-ordering.md)
contains the new terminal-drain and dependency-priority counterexamples, their
final regressions and network evidence. The older NPoS failures remain
causally unattributed; neither successful replays nor the independently
reproduced queue defect establish their initiating error.
