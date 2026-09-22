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

The disjoint 2,566-control Core migration run is still active. Two beacon
optional-slot fixtures mutate the epoch endpoint without rebuilding the bound
mint-finality authority; the off-tree correction constructs both from an explicit
endpoint and preserves the existing assertions. A governance slash successor
reports one unavailable ordinary candidate; diagnosis remains open. Two generated
model instruction rows also remain stale. Source edits remain frozen until the
current runtime captures end; passing compilation does not close these failures,
prepared successor activation, production Native ownership cutover, or the
required unchanged four/seven-validator qualification.
