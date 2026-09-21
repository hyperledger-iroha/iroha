# First-release source integration

The [completion goals](../../../specs/first_release_completion_goals.md) remain
active. This checkpoint records source reconciliation and local checks, not a
release candidate or production qualification.

## Preserved source and integration

The integration branch `codex/complete-privacy-sorafs-multilane` starts at
`f11eed2d6c7113163295e65703d9c206a5ce07d9`, the original clean `optimizations`
checkout. The SoraFS branch tip
`a156eda165531852d97029d1a3f77a1ebfd5d29e` is already an ancestor; its additional
implementation was in uncommitted source. No commit or release signature has
been created by this integration.

A concurrent local commit, `9f9cfcc61f4cabed296104bc761c3d8c74ce8846`, later
captured 428 changed paths on this branch. This agent did not create that commit.
Additional concurrent changes briefly left five unmerged retained-execution and
source-checker index entries. Their fifteen original stage blobs, working
versions, binary staged/unstaged patches and Git identities were preserved under
`concurrent-integration-20260920T131830Z` in the same preservation directory
before the concurrent resolution completed. No reset or replacement of that
work was performed. The resolved actual-source baseline passes one test with
425 deselected; the earlier long mutation run encountered changing source and
does not establish a qualified source matrix. The branch is still unfrozen.

The original SoraFS checkout remains untouched. The owner-only preservation
directory `iroha-release-preservation-20260920T115022Z` beside the checkouts
contains its Git identity, index/status, binary patch, source manifest and
verified source archive. The archive SHA-256 is
`bff3e8361ba6973921895a2eb416e8b5b61c7f7dc9d395ec3310fb8bc7a3b6af`.
It captures tracked and nonignored untracked source. A subsequent audit found
one ignored source directory, `crates/iroha_sccp/src/test_fixtures`, in addition
to caches and generated artifacts. Its descendant tests are separately
preserved and integrated; their original SHA-256 is
`2db30ac8b2f104c0154c99601228f3f76c138dbc5ef7574e33dca045774ba9f0`.

A complete copy-on-write filesystem copy also preserves ignored source,
artifacts and build outputs. Its 149,686 entries match the original names,
types, modes, regular-file sizes and symlink targets. That metadata comparison
does not claim a content hash of the 263,684,457,950 bytes of regular files;
source content is independently checked by the source archives.

An ignored-source audit of the integration checkout also found the existing
FASTPQ compact-protocol `test_fixture.rs`, required by tracked test modules but
hidden by the repository's `test_*` rule. Its original SHA-256 is
`b50e7e9509cd7fe6524059e330147c9695d23411076c8a24bd2bc62051bd8dc9`;
`integration-ignored-source.tar.gz` in the preservation directory captures it
before staging. Narrow ignore exceptions keep this file and the SCCP fixture
sources discoverable in future clones. The FASTPQ file remains explicitly a
synthetic polynomial fixture with test-only limits, not a production-relation
or compact-proof qualification result.

The three-way source reconciliation preserves newer State, execution-output,
Kura and socket-deadline owners. Current finality assertions were migrated
before removing the retired hardware test modules. The SoraFS configuration
uses authenticated software custody without an HSM prerequisite. Retired
hardware configuration names and proof APIs are removed, not aliased.
Generated schema output must come from the combined source; the older SoraFS
schema was deliberately not copied over current descriptors.

## Implementation boundaries

- Retained validation refuses descriptor exhaustion before body I/O/decode,
  reserves the subject before execution, keeps a tombstone on unwind, releases
  only the new vacant slot on explicit pre-owner refusal, and drops payloads
  before their original producer. This does not fund State/World allocations.
- Cold marker recovery distinguishes the bound current height's State from
  Kura finality and requires its canonical execution image. The current Apply
  path already executes finalized-but-unapplied State once; cold replay must
  not introduce a second execution. Earlier context directories are outside
  the active BodyStore's revalidation inventory.
- Final-promotion account preparation derives Reserve/Complete from executed
  native Checks, binds the full payload and approved fees, retains Complete's
  actual receipt, and requires fresh checks after key I/O. Configured provider,
  observer, clock, floor and spending-source assembly remains unfinished.
- MKHE delta preparation uses the original source/session and sole opening
  inventory. Completing the remaining producers, authenticated production
  source, whole composite verifier and bounded qPCS remains separate work.

Native ingress stays closed until funded retained execution, both Apply
consumers and lifecycle publication replace the old path together. The four
generic unsupported signer purposes and final-promotion gate remain closed.

## Local validation and reconciliation failures

The first ten focused SoraFS script suites pass 1,793 tests. The IVM-only and
retired-codec guards pass. The retained-validation source controls pass nine
ordering/lifetime cases and seven mutation cases; the recovery change also
passes the existing actual-source baseline. These are scoped local checks.
The original delta-source selection passes all twelve Rust tests, with zero
failures or ignored cases, in 849.52 seconds; its sixteen-file source manifest
is retained with the log. The subsequent beta/m continuation and test-setup
changes require their own fresh execution. Neither selection establishes a
full authenticated production source or composite proof.

The first native model test run fails two tests because imported fixed codec
inputs used the older `ed13…` policy specimen while current captures use
`9376…`. The Nexus regression confirms the same mismatch. The fixture inputs
are corrected to the existing captured values; the canonical fixture bytes
are preserved. The first formatting attempt exposes the ignored descendant
module omission; after its preservation/import and current header-API update,
workspace formatting succeeds.

The first combined check intersects in-progress MKHE source wiring and fails.
The next check reaches Core and exposes retired proof/result APIs and header
constructors in imported signer queries and fixtures. Those callers require
the current typed execution-output/proof owners. Their reconciliation now uses
those owners directly; twelve fixture constructors remove only the retired
result-root argument. Review confirms their other inputs and assertions are
unchanged. The third check finds remaining fixture imports and two of those
constructors; those are corrected before the fourth combined check. Native
Core/daemon execution is still pending at this checkpoint. The fourth check
accepts Core and Torii production/test code, then finds compile errors in the
new daemon account-transaction slice. Those require correction and a fresh
combined check. The fifth combined check passes all six requested packages'
production/test compilation in 11 minutes 48 seconds; 99 daemon warnings
remain, so this is not a strict Clippy pass. The separate SoraFS configuration
selection passes all 66 tests. Must-use validation results now explicitly assert successful
receipts; test-only KAGEMUSHA helper functions are scoped to their actual unit
test callers rather than leaking unused helpers into feature-enabled libraries.
Logs are under
`target/first-release-*`; failed runs are retained and are not counted as
passes. Source is still changing and has no immutable release seal.

The corrected native model selection passes five concrete-identity tests;
the Nexus selection passes two. Each contains one explicitly ignored fixture
capture generator. The final-promotion model selection passes all nineteen
tests. These use the rebuilt HTTP-feature model test binary and preserve the
existing canonical captured bytes.

The release profile, fail-closed OpenAPI, contract-call and Nexus lane smoke
selection passes 182 tests. The lane fixture now carries the required
`runtime_catalog_hash: null` for its baseline-only catalog; the strict parser
still rejects the retired field inventory. The broader OpenAPI compaction
selection exposes an existing source-size failure: the two unchanged baseline
Rust files total 3,410 lines against a 1,902-line ceiling. The ceiling remains
unchanged. The workspace source-file budget also fails with 273 findings;
neither source-size gate is qualified by this integration.

The release bootstrap had pinned an older artifact-contract helper even in
the original checkout. Review against the last matching helper revision
`9c17d8bd07692be097e0e70e2b360a3e9b9443fa` confirms the intervening changes add
owner-only descriptor-walk directory creation and rehash pinned bytes after
use. The 31 artifact-contract tests pass. Both bootstrap pins now match the
actual helper SHA-256
`ae28b33969b6b9cc201877fc860b6936d0a162f3474fcabba9882f44339b3965`;
the other helper pins remain unchanged. The audit and reviewed diff are
retained in `target/first-release-bootstrap-pin-audit.json` and
`target/first-release-bootstrap-contract-review.diff`. This local source
review is not an independent security audit or release seal.

Full workspace/SDK/native/hardware execution, schema/OpenAPI regeneration,
distributed lifecycle and soak evidence, independent reviews, signed readiness
summaries and promotion remain open against the eventual matching candidate.

The first Kagami generator build failed because the imported older
`cfg(test)` module gate hid current production scaling-evidence dispatch.
Restoring the current module declaration preserves that CLI and its existing
authority checks; it does not qualify the remaining scaling producer. The
second locked Kagami build passes, with eight warnings. Canonical schema
regeneration waits for the new election context to settle and a fresh generator
built from that exact model; the earlier generator cannot qualify later fields.

## Charged Cell restoration prerequisite

MV's existing Norito Cell parser now accepts explicit prepaid current/undo
allocation charges and moves both exact decoded values into their EBR owners.
The untracked parser delegates to the same implementation. There is no default
charged decoder, alternate wire layout or post-allocation admission. The
constructor avoids intermediate generations and value clones, and charged
serialization retains the canonical `{revert,blocks}` object.

The full locked MV suite passes 208 tests: 144 unit, 21 EBR allocation,
6 identity, 13 owned-allocation, 22 map-generation and 2 documentation tests.
This includes malformed-input refund, capacity refusal before parser entry,
unwind, retained-reader/epoch and actual `System.dealloc` ordering checks.
The log is `target/first-release-mv-charged-tests.log`; source/evidence lives
under `target/first-release-mv-charged-restore-20260920`. An unused import was
removed after the run. Concurrent Storage undo changes are captured separately;
these tests do not establish complete production State funding. Nested values,
parser scratch, control allocations and the configured aggregate admission
pool still require actual owners and fallible production propagation.

## MKHE delta and beta/m validation scope

The original delta run used the sixteen-file capture
`target/first-release-mkhe-delta-source.sha256.txt`; its twelve tests passed in
849.52 seconds in `target/first-release-mkhe-delta-tests.log`. The subsequent
beta/m admission and optimized earlier-inventory fixture setup are captured
separately in `target/first-release-mkhe-beta-source.sha256.txt` (twenty files).
The latter candidate requires a fresh run; the earlier pass is not attributed
to these later source changes. The fresh seventeen-test run now passes with
zero failures or ignored cases in 849.76 seconds, recorded in
`target/first-release-mkhe-beta-tests.log`. Its twenty-file source manifest was
rechecked before completion. The intentional entropy-unwind panic in the log
is caught by the adversarial test, which passes; it is not a process failure.

Run the updated seventeen delta/beta tests against the settled combined source:

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked -p iroha_zkp_halo2 --lib prepared_difference -- --nocapture
```

The filter includes the beta/m continuation nested under the delta commitment
owner. Relevant adjacent controls use the exact module filters below; the
`::tests::` suffix avoids rerunning descendant delta/beta suites through an
ancestor name. All 26 adjacent controls passed on the same frozen binary:
8 radix, 2 comparator-plane, 4 comparator-commitment, 6 retained-session and
6 existing-candidate cases. The recorded selections and results are in
`target/first-release-mkhe-beta-adjacent/results.json`. Later signed-source
changes require new compilation and are not covered by those passes.

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked -p iroha_zkp_halo2 --lib radix_range_v2::tests:: -- --nocapture
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked -p iroha_zkp_halo2 --lib prepared_comparator_plane_v1::statement_tests:: -- --nocapture
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked -p iroha_zkp_halo2 --lib prepared_comparator_commitment_v1::tests:: -- --nocapture
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked -p iroha_zkp_halo2 --lib retained_source_session_v1::tests:: -- --nocapture
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked -p iroha_zkp_halo2 --lib existing_radix_candidate_v1::tests:: -- --nocapture
```

The new tests check integer delta boundaries, sealed-bit/source correspondence,
strict cursor and inventory geometry, canonical chunk order, actual selected
boundary MSMs, original rho/opening custody and refusal/drop behavior. Earlier
D/S/top/delta inventory points used to enter later-stage tests are explicitly
synthetic. The optimized fixture setup samples each original rho and checks the
existing candidate, blinding and owner-binding KATs through final owner
validation; it does not replace production token checks or count synthetic
points as actual source-derived MSMs.

A separate read-only agent reviewed source/packed linkage, original-session
transfer, delta and beta/m coordinates, retained dispatch and fixtures, and
reported no concrete correctness findings. It ran no builds and performed no
independent cryptographic security assessment. Positive stage tests do not
establish a complete authenticated production source/dispatch execution,
all actual commitment MSMs, stored-opening tails, full composite proofs,
whole-proof resource bounds, SDK/hardware parity or release qualification.
Those remain separate obligations under the unchanged production gates.

The subsequent signed-source slice consumes the completed beta/m owner and
fills physical inventory `25,112..27,176` with the existing 1,032 signed and
1,032 negative-magnitude commitments. It uses the retained authenticated
compact source, natural signed coordinates, original entropy and sole opening
inventory. The derived positive point is checked without creating another
commitment or rho. No resource ceiling, production authority or qualification
flag changes. Its source-coupled contract is
[`prepared_small_signed_plane_v1.md`](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/incremental_source_phase23_radix_range_v2/prepared_small_signed_plane_v1.md).

`target/first-release-mkhe-signed-source.sha256.txt` captures 28 relevant
source/doc files for that candidate and was verified after its test binary
compiled. The binary and identity were pinned under
`target/first-release-mkhe-signed-native` before subsequent source changes.
Scoped formatting and `git diff --check` passed. The following fifteen-test
selection passed all 15 cases, zero failed/ignored, in 641.98 seconds; the log is
`target/first-release-mkhe-signed-tests.log`:

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked -p iroha_zkp_halo2 --lib prepared_small_signed -- --nocapture
```

The selection includes five prepared-value controls, three source-reader
controls, five original-session commitment controls and two retained-dispatch
controls. Selected boundary commitments and retained dispatch execute actual
MSMs, while earlier inventory points remain synthetic fixtures. Tiny encrypted
file reads exercise the actual confidential storage API but do not inhabit a
complete production replay owner. Full signed inventory execution,
source-packing/membership proofs, Q-mask production, governed source admission
and composite/resource qualification remain open.

The subsequent opening-tail slice changes that source graph and has separate
runtime validation. Its 37-file source/doc manifest is
`target/first-release-mkhe-opening-tail-source.sha256.txt`; scoped formatting of
21 Rust files and `git diff --check` passed before the coordinated build.
The first test compilation found an ambiguous test-only entropy type; its exact
generic annotation was fixed before refreshing this manifest. The retry compiled
in 17.18 seconds, and all 37 source/doc hashes were verified immediately afterward.
The pinned binary and identity are retained under
`target/first-release-mkhe-opening-tail-native`. The manifest SHA-256 is
`c971d2b0e309ffd731024e4686f224904ab66efedfb2c5ce112ab7b5ed733447`, and the binary
SHA-256 is `ed376658fb9b7813dc2505303f90e808a6361b30e12886e82b22d091a359a9a9`.
Each comparator/top, beta/m and signed producer returns an
opaque tail made from its just-admitted original inventory ticket and exact
retained rho. Its prepared source emits 32 value chunks followed by the canonical
`rho_BE32 || point33 || zero-padding16319` tail before source handoff. No extra
mask, inventory slot, source authority or resource ceiling is introduced.
The ordered two-file writer and authenticated complete-pair reopen remain open.

The following selection passed all 16 cases, zero failed/ignored, in 555.04
seconds against that pinned binary; its log is
`target/first-release-mkhe-opening-tail-tests-2.log`:

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked -p iroha_zkp_halo2 --lib prepared_opening_tail -- --nocapture
```

It selects ten geometry/lifecycle/refusal controls and six strengthened existing
actual-MSM/retained-dispatch controls. The latter compare each emitted tail with
the same original rho/ticket and existing independent sparse group equations;
eleven actual MSMs cover bD, bS, beta, m, signed and negative boundaries plus
retained signed dispatch. Earlier inventories remain synthetic fixtures.
Erasure tests observe disposal of existing scalar/vector zeroization guards on
success, refusal and unwind; they do not qualify compiler/register erasure.

The next `QMaskDigit` slot is still physical ordinal 27,176. Its specified
equations constrain `S_q + Sbar_q = q_l - 1`, the final zero coefficient and
four radix digits, but no authenticated `S_q` coefficient producer or reviewed
sampling/lifecycle owner exists in the current graph. The corresponding
`q_mask_s_commitment_owner` production handoff remains uninhabited. This slice
does not replace it with fixture coefficients, a detached digest or arbitrary
entropy, and no qPCS/composite or production qualification is claimed.

The subsequent ordered-storage budget slice attaches the shared ledger to the
actual existing two-file writer/snapshot lifecycle. Atomic admission reserves
both files and their complete write/seal I/O before creation. Reads use that
same ledger; a local capacity refusal retains the exact authenticated pair and
does not debit I/O. Cancellation closes the detached files before releasing
live-file credits and refunds only unattempted reserved I/O. Fixed native caps,
canonical context geometry and all production authority seals remain unchanged.
The source-coupled scope and exclusions are in
[`resource_budget_v1.md`](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/global_lookup_statement_v1/vector_arithmetic_plane_openings_v1/ordered_snapshot_v1/resource_budget_v1.md).

Its focused runtime selection is eleven cases (four ledger controls
and seven actual encrypted-file lifecycle controls):

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked -p iroha_zkp_halo2 --lib ordered_storage_budget -- --nocapture
```

The broader `ordered_snapshot_v1` filter also includes the nine existing
geometry, syntax and actual-file adversarial controls. Scoped formatting and
`git diff --check` pass. The broader filter passed all 20 cases, zero
failed/ignored, in 0.25 seconds; its log is
`target/first-release-mkhe-ordered-storage-budget-tests.log`. All 13 source/doc
hashes were verified, and the binary/identity are pinned under
`target/first-release-mkhe-ordered-storage-budget-native`. This is separate from
the preceding pinned tail16 evidence. Its source/doc manifest is
`target/first-release-mkhe-ordered-storage-budget-source.sha256.txt`, SHA-256
`04e9fc3df50fa79a9482d88bc1196ad78f648a6d5e45061e8272e81008789eea`.
Tiny encrypted files are storage fixtures,
not authenticated production source evidence. Full-size execution, one shared
issuer across all production source/qPCS consumers, RSS/control allocation
accounting and the context/source-to-storage schedule remain open.

A later bounded reconciliation removes the unconsumed 344-weight canonical-
reopen placeholder and its local plane-context axis. Original source commitments,
blindings, authenticated replay, fixed caps and all genuine authority gates are
unchanged. Independent byte encoding reproduced both prior KATs before deriving
the revised source-record/context/plan KATs. Rust runtime validation of this
separate source slice is pending; none of the preceding pinned results is
relabelled as evidence for it. The unchanged 1,376-owner native source/packing
relation still requires its genuine production derived-mask/source owners, and
the 38-limb preflight remains insufficient for native40. The source-coupled
[dependency audit](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/global_lookup_statement_v1/vector_arithmetic_plane_openings_v1/canonical_reopen_reconciliation_audit_v1.md)
records the remaining storage/proof boundaries.

## Native runtime and formal diagnostic follow-up

The rebuilt Core binary `iroha_core-7253dd96f98845a3` passes all eleven retained
validation controls and all 91 Apply/recovery controls. The latter includes
the new missing-execution-image, missing-finality, State-height mismatch and
exactly-once recovery cases. Ten signer-check controls also pass. The new
setup facade has one failing assertion about an entirely empty contract-state
map after genuine Register/Grant; signer finality has one failing fixture
that tries to rebind a network-pinned Kura. Both failures remain recorded in
`target/first-release-core-native-controls`. Test repair must preserve actual
custody-history absence and foreign-network refusal. The binary predates the
last test-only receipt-subject assertions and the immutable account-binding
accessor; final-source qualification therefore still needs a fresh build.

The 18-step Apalache gate remains unpassed. Isolated diagnostic copies preserve
all twenty configured invariants and the `after` schedule. Dynamic-domain and
fixed-domain disjointness rewrites both reach their declared 600-second CPU
limits with unresolved SMT queries, without a counterexample or pass. The
experimental arrays encoding on the unchanged canonical model/config fails
with RuntimeError at step-seven enabledness; its trace follows the pinned
checker's solver-UNKNOWN/timeout branch. None changes the canonical model,
source correspondence pins or release obligations. Exact inputs, commands,
results and interpretation are retained in
`target/first-release-apalache-diagnostics-20260920/summary.json` and the three
referenced diagnostic packets. No diagnostic substitutes for the required
completed canonical bound.


## Integrated SoraFS validation and interface refinement

The manifest crate passes 1,075 unit tests. Its initial integration invocation
omitted the `dev-tools` feature, so twelve generator-dependent cases failed;
that invocation is retained as a failure, not fixture drift or a pass. Building
the actual generators with `--features dev-tools --test sorafs_manifest_integration`
then passes all 62 integration cases. The doc-test target contains zero cases.
Logs are `target/first-release-sorafs-manifest-tests.log`,
`target/first-release-sorafs-manifest-integration-tests-2.log` and
`target/first-release-sorafs-manifest-doc-tests.log`.

Strict Clippy exposed six shared Merkle-map issues, followed by signer/PoP and
fixture-generator issues. The Merkle-map changes retain its hash inputs and
pass six controls, including independently rebuilt references across all split
bits, insertion/deletion orders and mixed mutations. PoP prove/verify now takes
one typed `PopMembershipPresentationV1` containing the independently expected
challenge, verifier context and action binding; all callers use that sole API.
Signer operation commitments return a typed error, completed stream-token rows
are boxed under their existing authenticated subject, and test/generator lint
issues are corrected. No compatibility wrapper or lint waiver was added.

The exact strict command passes after these corrections:

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- clippy --locked -p sorafs_manifest --features dev-tools --all-targets -- -D warnings
```

Its log is `target/first-release-sorafs-manifest-clippy-4.log`; earlier numbered
logs preserve the failed attempts. The subsequent complete
`cargo test --locked -p sorafs_manifest --features dev-tools` run passes all
1,076 library tests, twelve generator tests and 62 integration tests. Its log
is `target/first-release-sorafs-manifest-tests-2.log`; zero-case binary and doc
targets do not add qualification evidence. Real source, provider and finalized-operation integration,
inner promotion authorization, independent review and production deployment
qualification remain open.

The earlier retained-source mutation suite was interrupted after a concurrent
local integration changed its source contract during execution. Its mixed-source
errors are not a candidate result. The current source baseline alone passes
(one selected case, 425 deselected); the full current mutation suite still
requires a stable-source run. Both logs remain under `target/first-release-*`.

## Frozen public governance and snapshot allocation integration

The public conviction model now exposes explicit canonical Norito schema identities,
required frozen context and final-result fields, and strict JSON rejection of
unknown fields. The canonical schema registry includes both context/result roots
and their nested policy/decision shapes. These are public governance prerequisites;
anonymous credential, confidential bond and dropout-resilient tally construction
remain open. No older-layout decoder is added.

The six initial arithmetic/codec controls passed after correcting schema identities
and value-copy semantics (log `target/first-release-plain-model-tests-3.log`).
That pinned binary predates the seventh strict-JSON test and schema-map registration.
The subsequent 38-file source scope is
`target/first-release-plain-source-manifest-4.json`, SHA-256
`1963055ac848425f812fe2188478ab9bb8841e0522c1d63810ca73eb849d8cf3`.
Fresh model, schema and Core runtime qualification remains pending for that scope.

Combined production check 6 exposed Core's prohibition of raw allocation and stale
public fixture calls. The exact charged byte allocation now belongs to MV through
its safe `ChargedByteBuffer` API; all six allocator/refusal/deallocation-order
controls pass (`target/first-release-mv-byte-buffer-tests.log`). Core holds that
original owner through authenticated snapshot reading/initialization and defers
refund notification through the complete locked publication operation. This covers
only the backing input bytes, not decoded State, nested allocations or RSS.

Check 7 then rejected Torii's obsolete `Copy` derive for the now-owned referendum
policy; the response is `Clone` only. Check 8 passes in 2m29s:

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- check --locked -p iroha_config -p iroha_data_model -p iroha_core -p iroha_torii -p irohad -p sorafs_manifest -p sorafs_node -p iroha_schema_gen --features iroha_core/iroha-core-tests
```

Logs `target/first-release-combined-check-{6,7,8}.log` retain the attempts. The
successful check still reports six Core and 99 daemon warnings; it is not strict
workspace Clippy or test execution. Review subsequently identified a PLAIN
restitution aggregate-capacity gap, so this check predates that next correction.

The SoraFS/finality OpenAPI contract tests have been compacted without changing
their pinned asset, source guard, required test inventory or assertions. Their
combined 1,894 lines fit the unchanged 1,902 ceiling. The unchanged source guard
passes four tests and four subtests (`target/first-release-openapi-compaction-current.log`);
native OpenAPI tests remain pending. The exact three-file source packet is
`target/first-release-openapi-contract-source-manifest.json`, SHA-256
`08ed72b4bec1961de02ee7710441b30552e1ccc120a28c46398a1ab6cd19c5fb`.

The strict public conviction model subsequently passes all seven tests, zero
failed/ignored, in 0.02s (`target/first-release-plain-model-tests-4.log`). The pinned
binary and source-scope identity are in `target/first-release-plain-strict-model-native`.
Review found and corrected a restitution path that could restore slashed weight
beyond aggregate capacity after other ballots consumed the freed headroom. It
now checks the frozen corpus before any custody movement; the new funded regression
also preserves the retained closed result. Its Core runtime test remains pending.
The updated 38-file source manifest is `target/first-release-plain-source-manifest-5.json`,
SHA-256 `65eeb6e9135b51741aff48d5235f1573fd525635531951763386c83068eb6861`.

The complete configuration integration suite passes 241 cases, zero ignored, in 1.72s,
including both snapshot-buffer boundary cases and the minimal default snapshot.
The first invocation failed to compile because the new fixture helper inherited
`error_stack::Result` through its parent import; its return type is now explicit
`std::result::Result`. Logs are `target/first-release-config-integration-current.log`
and `target/first-release-config-integration-current-2.log`; the successful binary
is pinned under `target/first-release-config-current-native`.

Current IVM-only and retired-codec/Native compatibility guards pass; their logs
are `target/first-release-ivm-only-current.log` and
`target/first-release-no-legacy-codec-current.log`.

The 13-suite SoraFS release-script run completed in 8572.49s with 1,394 passes,
29 skips and four failures (`target/first-release-sorafs-script-tests-2.log`). Two
failures are stale shell/documentation contract assertions; two report unfinished
source TODOs. Skips and open-source gates are not qualification passes. Corrections
and focused reruns are recorded separately; the failed full run remains retained.

## Current schema, native opening and formal checks

All twenty canonical schema-registry tests pass on the revised public conviction
model (`target/first-release-schema-gen-current.log`). This verifies registry
closure; generated schemas still require a fresh Kagami build and deterministic
double regeneration. The pinned test binary and direct source identities are
under `target/first-release-schema-current-native`.

The unused 344-weight canonical-reopen mirror has been removed after tracing its
absence from consuming proof equations. The original 344 source commitments,
authenticated first pass and real 1,376-owner native same-opening relation remain.
All 75 selected source/opening/algebra/storage/same-opening controls pass with
zero failures or ignored cases. Their exact source manifest, copied binary and
five separate logs are under `target/first-release-mkhe-reopen-removal-native`;
manifest SHA-256 is
`eee0c02553ea5af80be15012f7bd2c6e43739c149215b7e6aa96266da48f35d9`.
This removal does not establish a genuine 40-limb source, consuming production
composite, whole-proof resource bounds or independent cryptographic review.

The unchanged canonical in-flight TLC runner completes its positive finite
exploration and all 25 required mutation controls. The source-binding preflight
and 23 focused source/ownership controls also pass after scoped binding repairs.
Failures, exact scopes and remaining formal obligations are retained in
[multilane formal validation](multilane-formal-validation.md). In particular, the
18-step Apalache gate is still unpassed.

Core's current test-harness compile initially rejects one obsolete `errors()`
fixture call and one snapshot initializer missing the new read-buffer limit.
The fixture now reads canonical `failed_outputs()`; Kiso and the analogous
Torii test fixture use the actual default buffer limit. No old method alias or
configuration fallback was added. The failed log is
`target/first-release-core-current-build.log`; the rebuild is recorded separately.

The live SoraFS C05/C06 and source-audit acceptance rows now use the same
authenticated signer wording as their governing software-custody contract.
Their obsolete hardware-only wording is removed; independent signatures,
finalized operation verification and all rollout requirements remain open.
Dated historical hardware-checkpoint evidence is preserved as historical scope.
