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
combined check. Must-use validation results now explicitly assert successful
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
source/doc files for this new candidate. Scoped formatting and `git diff
--check` passed. The following fifteen-test selection still requires execution;
neither the earlier delta/beta run nor its adjacent controls establishes a
pass for the newly wired source:

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked -p iroha_zkp_halo2 --lib prepared_small_signed -- --nocapture
```

The selection includes five prepared-value controls, three source-reader
controls, five original-session commitment controls and two retained-dispatch
controls. Selected boundary commitments and retained dispatch execute actual
MSMs, while earlier inventory points remain synthetic fixtures. Tiny encrypted
file reads exercise the actual confidential storage API but do not inhabit a
complete production replay owner. Full signed inventory execution, stored
opening tails, source-packing/membership proofs, Q-mask production, governed
source admission and composite/resource qualification remain open.

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
