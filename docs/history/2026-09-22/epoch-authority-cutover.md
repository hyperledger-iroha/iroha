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
