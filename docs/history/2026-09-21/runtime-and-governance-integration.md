# Runtime and public governance integration validation

This continues the [September 20 integration checkpoint](../2026-09-20/first-release-integration.md).
The active candidate remains an uncommitted integrated worktree, not an immutable
release. No release goal is closed by the following scoped checks.

## Core runtime results and corrections

The corrected Core library harness compiles in 6m21s with the `iroha-core-tests`
feature. The copied binary, exact collection, source observation and commands
are under `target/first-release-core-current-native`; compilation logs are
`target/first-release-core-current-build{,-2}.log`. The first attempt failed on
a retired fixture method and a missing snapshot-budget field; neither was
restored as a compatibility path.

The `snapshot::tests::` substring selection passes 134 tests in 66.68s. It includes
all 102 storage snapshot tests and 32 other query/receiver/stake snapshot tests;
it is not a 134-case storage-only claim. All eight new charged-buffer cases pass.
The three separate MV geometry projections, four startup-recovery controls and
three signed Kura snapshot controls also pass. SnapshotMaker composed retry,
Strict initializer error/unwind, and concurrent shared-pool Strict/GC coverage
remain outstanding as indexed in the snapshot review packet.

The same copied harness passes four signer-fixture controls, ten exact signer
Checks and seven durable-finality controls. All 91 Apply/recovery controls pass
in 188.55s. All fourteen retained-validation controls and 36 physical publication
controls pass; the latter run takes 72.42s. Seventeen original-journal controls
and the repaired public artifact-fee test also pass. The initial retained-owner
selector matched no tests and was recorded as unexecuted; its corrected nested
module selector is separately collected and executed. Zero collection is never
counted as qualification.

Two PLAIN restore tests fail because their voter is the same account as the
configured default escrow/slash account. The production refusal is correct.
Their fixtures now use distinct custody identities; the aggregate test also
checks that its single maximum-width lock is individually valid before testing
overflow. The original failures remain in `06-tests.log` and `09-tests.log`;
The next build passes in 2m42s (`first-release-core-current-build-3.log`). Its
copied harness under `target/first-release-core-restore-repair-native` passes the
aggregate boundary and three adjacent restore controls. The malformed-corpus
test reaches its next assertion and fails because `InvalidParameter` Display
does not include the nested owner reason. The repaired assertion now checks the
exact typed `SmartContract` error and separately checks the public projection's
Display correspondence; it does not accept an arbitrary rejection. Its rerun is
included in the later successful Core/Torii harness described below.

The complete Core governance group 02 collects 25 tests and runs with two workers
and no `RUST_MIN_STACK` override: seventeen pass, eight fail. Six failures are
the same invalid voter/custody fixture; two are stale rejection assertions. The
actual funded restitution regression passes, including aggregate-capacity
refusal, unchanged custody/lock state, successful retry and retained closed
result. The failed full group is preserved under
`target/first-release-core-governance-group-native`; subsequent fixture repairs
must preserve exact rejection causes and real funded transfers.

The repaired fixture rebuild passes in 2m04s. All 25 group-02 tests now pass in
10.69s; all four citizenship tests, one opening-event test, one open/close test
and two unlock tests also pass. The copied binaries, exact nonzero collections,
fixture source manifest and runtime logs are retained in
`target/first-release-core-governance-fixture-native`. Production escrow/authority
checks remain unchanged. Disabled/missing-reference tests assert exact rejection
variants and unchanged balances; the custody accounts are selected explicitly
before funding and freezing the context. These are public-governance controls,
not anonymous-election qualification.

## Tally observation binding

Review found that Torii copied a governance World view, then separately read the
live committed height and hash, and read the live height again for expiry. A
concurrent publication could therefore mix generations in one response. Tally
projection now accepts one captured `StateQueryView`: selector authority,
referendum, corpus, expiry cutoff, height and hash all come from that owner.
The locks/referendum readers likewise use their one World view for selector
refusal and record projection. There is no alternate response path.

A new regression captures an open corpus and its block-hash journal, publishes
a later fixture corpus with increased bonds, then verifies both old and new
projections retain their own exact totals and anchors. This is a projection
fixture, not proof of asset conservation or finality; funded Core lifecycle
tests own those separate obligations. All seven current native Torii tally
controls now pass, including this captured-view regression.
The direction/overflow controls now establish a valid projection before
corrupting the direction or adding a second maximum-width lock, and require the
exact canonical Core invariant error. Tally path rejection also exercises the
same malformed selector inventory as referendum lookup.

The first combined Core/Torii test build fails on 167 OpenAPI macro lookup
errors: the shared macro include occurred after the finality/application test
consumer. The one canonical include now precedes its consumers, and a duplicate
caller-location attribute is removed. The exact 24 contract tests and their
assertions are preserved; the four source guard tests and four subtests pass.
The failure log remains `target/first-release-core-torii-tally-build.log`; it is
not native runtime evidence.
The second compilation reaches type checking and exposes one missing map borrow
in an OpenAPI test call; its direct call-site correction preserves the macro's
existing input convention. The third compilation passes in 6m49s, with the six
existing Core dead-code warnings still present. It is not a strict-Clippy pass.
Logs remain `target/first-release-core-torii-tally-build{,-2,-3}.log`.

## Reconstructed bond minimum

Review found that persisted public positions validated unit conversion and
custody but did not check their original bond against the frozen minimum.
Admission requires the minimum, updates cannot reduce the bond, and slash and
restitution transfer equal amounts between `amount` and `slashed`. Consequently
`amount + slashed` must still meet that minimum. The validator now enforces this
sum, preserving valid majority/full slashes. New regression controls cover
invalid unslashed, partially slashed and fully slashed originals, open tally and
restore, immutable closed results with malformed retained locks, and an explicit
zero-minimum zero position. Source-adjacent documentation records the invariant.
The reviewed patch and original hashes are retained under
`target/first-release-plain-minimum-bond-patch`.
Both reconstructed-bond/malformed-corpus controls now pass. The three revised
restitution tests, four opening/closing/unlock controls, all 25 group-02 tests,
seven Torii tally controls and both selector controls pass. The existing
`double_vote_slashes_plain_lock` fixture fails before its slash assertion because
its unchecked Network block has no exact initial genesis admission. The failure
is retained and needs a canonical genesis/RS16 fixture repair, not an admission
bypass.

The full native OpenAPI selection runs 126 tests: 105 pass and 21 fail. Failures
include static authority/catalog drift, an inexact floating-point u64 bound,
authentication/effect metadata and obsolete field expectations. The actual
current runtime DTOs and authenticated route catalog govern reconciliation;
retired fields cannot be restored to satisfy old assertions. The copied native
binaries, all nonzero collections, source observation, complete logs and exact
failures are under `target/first-release-core-torii-tally-native-3`.
The reviewed reconciliation is now applied: seven actually mounted routes and
their response schemas, 110 exact u64 maximum tokens, current DTO fields, live
alias query parameters and the completed-trigger operator effect. The authored
authority and both mirrors are byte-identical. Stale assertions now reject the
retired header/VRF fields and removed draft route, and preserve current public
authentication alternatives. One new native regression checks exact uint64
minimum/maximum tokens recursively. The compact contract guard passes four tests
and four subtests; the full 127-case native OpenAPI rerun is pending its fresh
build. The reviewed authority packet and producer references remain under
`target/first-release-openapi-authority-repair-20260921`.
That build fails with E0425 because one new parent test calls a private child
module helper. The call now uses the existing parent strict-object helper with
an empty optional-field set, preserving exact properties, required fields and
unknown-field rejection. The original failure remains in
`target/first-release-openapi-reconciled-build.log`; a separately recorded second
build and source observation precede the native rerun.

The second rebuild succeeds in 6m18s. Its native OpenAPI selection passes 119
tests and fails eight; all fifty selected adjacent offline/game/catalog/security
and matched-route tests pass. Remaining failures identify retired positive route
expectations, stale activation/runtime-pressure fields, exact credential-delete
headers, equivalent alias digest constraints and operator-signature effect
metadata. The reconciled source removes the retired expectations and schema,
requires the actual six delete parameters, and marks all thirty affected
operator-signature routes with their current operator effect. The same closed
alias digest language is expressed consistently. All three authority mirrors
have SHA-256 `c9df5bf32b3989c95d3c5c911c683bc4c47d6c29e2abb8ed0e4ca1a0eb14e281`.
The compact contract guard passes four tests and four subtests. A fresh integrated
native rerun is required; the failure packet remains
`target/first-release-openapi-reconciled-2-native`, and the exact edits and
semantic inventory are in `target/first-release-openapi-followthrough-20260921`.

## Daemon signing, deadlines and startup

The current daemon library harness builds in 12m35s with the six existing Core
dead-code warnings; this is not a strict-Clippy result. Its copied binary has
SHA-256 `234ffd064e17298f1aa1f255385b055ff7ff2b48b86bdf4c38fcee4405f8bd14`.
Nine account/observer final-promotion signing controls and all eighteen snapshot
error/startup-policy controls pass, using two workers and ordinary worker stacks.
The initial deadline selectors matched no tests and are recorded as unexecuted.
The corrected nested platform selectors then pass all eleven absolute-deadline
controls and five exchange-deadline controls. These exercise the original
expiration across socket operations, unavailable/ambiguous outcomes and retained
session ownership; they do not establish production provider deployment.
The broader broker and signer-operation suites then pass all 265 and 104 tests
in 30.37s and 11.36s, respectively. Those runs include the focused signing and
deadline controls; they are not additional distinct tests. Together with the
eighteen startup-policy tests, this covers 387 distinct current daemon tests.
The broader packet is `target/first-release-daemon-broker-signing-native`.

The two packets are `target/first-release-daemon-focused-native` and
`target/first-release-daemon-deadline-native`, with exact collections, commands,
source observation and logs. The source observation was captured during the
already-running build and remained unchanged through these tests. It is not an
authenticated before-build seal or immutable release-candidate qualification.

The next signing change replaces inline release-manifest message assembly with
one private ceremony owner retaining the reviewed bytes, complete binding,
original verified custody, intent and reservation. Each message authenticates
the exact ordered prior signatures and the receiving operation before key I/O.
The existing staging, completion CAS and recovery path remain the sole receipt
publisher; the provenance anchor retains its original capture point. Seven
adversarial controls are added. The fresh daemon build passes in 10m29s, followed
by all seven new controls in 0.11s and the full 111-test signer-operation suite
in 11.20s. The seven are included in the 111, not additional distinct tests.
The selected source remains unchanged through both runs, with ordinary worker
stacks. Commands, exact collections, copied binary and source observation remain
in `target/first-release-release-ceremony-native`; compilation retains the six
existing Core warnings and is not a strict-Clippy pass. The reviewed patch is
in `target/first-release-release-ceremony-patch`.
Role13 production provider/native-authority admission remains closed; this
prerequisite is not deployment or final-promotion qualification.

## SDK and formal follow-through

The current Kagami binary builds in 6m55s; existing warnings remain and no lint
pass is claimed. Its canonical schema generator replaces the empty checked-in
`specs/references/schema.json` with 1,781 type definitions (595,051 bytes).
Two separate `scripts/tests/consistency.sh --update schema` invocations emit
identical bytes, and the subsequent consistency check passes. The schema's
SHA-256 is `662dcd50a90e543c75c126c20cf1f8f8126e423f1563a057e71b39fa0d5f8399`.
The copied binary, empty preimage, both outputs, commands and stable selected
source observation are in `target/first-release-kagami-schema-regeneration`.
This regenerates the current integration source; final candidate regeneration
and cross-SDK execution remain required.

Both Python clients now use the same exact tally model, signed request and
bounded response reader; the former coercing low-level decoder is removed.
Ninety-one parser/model/response-stream controls pass, and the pure SDK wheel is
sealed and preflighted. Authentic native/client parity is still pending in the
documented local wheel build at that checkpoint. The subsequent host rebuild,
strict installed verification and all 524 selected native-backed response and
transport tests pass; see the [Python record](python-governance-tally.md).
The documented build is development evidence and cannot satisfy the canonical
clean-source ABI/release gate.

The observed Homebrew JDK21 compiles Kotlin, its tests and the Java-source
consumers in 35s with the existing JDK8 API restriction intact. The selected JVM
client/Norito runtime run executes 472 tests: 362 pass and 110 fail. Every failure
contains the missing ABI-23 `connect_norito_bridge` validator error. The bridge
has no current host artifact, so a fresh native build and rerun are required;
no replacement address parser or fallback is admitted. Commands, original XML
results and unchanged 1,410-file Kotlin/fixture observation remain in
`target/first-release-kotlin-compilation-20260921`.

The authentic host JNI rebuild then passes in 7m18s. The retained library has
SHA-256 `1f709bb8a1c87cda7525be9c329644c753ee6a6bed00a662819aada3c8d87518`;
all 472 selected client/Norito tests pass against it in 71.64s. The full ordinary
JVM task subsequently runs 1,553 tests: 1,545 pass and eight fail, with no skipped
tests or observed source drift. Two failures expect the retired generic key
exception instead of the exact native `INVALID_PUBLIC_KEY` error; six lack the
explicit Rust fixture-generator executable. The test repair asserts the exact
native error and uses the Rust-authored valid secp256k1 fixture. All forty address
controls then pass in 23.39s. No production validator or native requirement is
weakened. The full retry requires a rebuilt `kotlin-fixture-gen`. The three
packets are `target/first-release-kotlin-native-runtime-20260921`,
`target/first-release-kotlin-full-runtime-20260921` and
`target/first-release-kotlin-key-runtime-20260921`. Android, other native targets,
CUDA/device execution and final candidate packaging remain unqualified.

The fixture generator rebuild then passes in 3m42s, with unchanged observed
Rust inputs. Its retained executable has SHA-256
`e2766a18f38c63a434fed3331b8dc9a0be3578bbfe5f7902cfc472d0d6a9607c`.
The full ordinary JVM rerun passes all 1,553 tests in 77.33s, with zero failures,
errors, skipped tests or observed Kotlin/fixture drift. It uses both rebuilt
native artifacts explicitly, including the Java-source consumer tests.
The generator and runtime packets are
`target/first-release-kotlin-fixture-generator-20260921` and
`target/first-release-kotlin-full-runtime-2-20260921`. This is current host
integration evidence; the full candidate/platform/hardware gates remain open.

The same retained JNI and generator also pass all 25 JVM attestation-tool tests
and `:tools:installDist` in 13.25s, without observed input drift. This additional
host result is retained in `target/first-release-kotlin-tools-runtime-20260921`.

The 17-case formal invocation completes its first five model configurations and
fails in liveness initialization when an empty candidate reads `asyncSentItems`
before that state variable is initialized. The original failure is preserved;
the minimal sentinel-first correction preserves actions, fairness, invariants
and bounds and passes 17 focused source/mutation controls. The exact canonical
liveness retry constructs its initial state but exits 255 after 14m10s when TLC
attempts to construct a set with more than one million elements while checking
initial invariants. No simulation traces complete. This is an evaluator resource
failure, not a pass or an invariant counterexample; the complete log/state and
unchanged source/tool identities remain under
`target/first-release-tlc-liveness-repaired-20260920`. An equivalent bounded-owner
quantifier representation is under investigation without weakening obligations.
Independent model inputs for the first five cases and the earlier 106-case
corpus are attributed separately; concurrent bound-source changes prevent a
whole-candidate qualification claim.

The causal-origin carrier type is now represented by an equivalent structural
predicate, avoiding materialization of its Cartesian product. Eight finite
equivalence comparisons, twelve mutations and ninety-seven source/mutation
controls pass. The exact canonical liveness retry then exits 12 after 6m30s:
`AsyncTypeInvariant` fails in the initial state. It is a genuine invariant
failure, with no completed simulation trace. The complete failed run is in
`target/first-release-tlc-liveness-structural-origin-20260920`. Target-only labelled
diagnostics are locating the original false conjunct; no invariant, fairness
condition or required bound has been removed. Formal qualification remains open.

The labelled diagnostic isolates the final clock-owner clause in
`AsyncSharedSchedulerOrdinalInjectionInvariant`. TLC parses its prior indentation
as requiring timeout ownership unconditionally, contradicting both the model's
empty initialization theorem and the runtime's two absent initial clock owners.
The reviewed correction explicitly groups both ownership predicates before the
implication. Finite TLC checks cover all eighteen combinations and reject active
ordinal aliases. The canonical model, exact source checker and mutation controls
are corrected together. This finite correspondence is not a liveness or TLAPS
pass; the unchanged full liveness configuration must run again. Inputs and the
reviewed source-hash chain remain under
`target/first-release-clock-ordinal-antecedent-20260921`.

The next canonical retry again reaches TLC's one-million-element set limit
during initial invariant evaluation (exit 255; Java time 13m39s), before any
simulation trace completes. Its packet is
`target/first-release-tlc-liveness-clock-antecedent-20260921`. The reviewed source
pin and mutation-fixture reconciliation subsequently passes all 158 maintained
Serve-scheduler/Commit-import controls and canonical preflight with unchanged
inputs. A target-only diagnostic retains the original bounds, all ten
invariants and four properties while labelling clause evaluation. These source
checks do not establish the required liveness result.

## Scope boundaries

The packing-evidence profile guard passes its two new matching/changed-profile
controls and the existing all-KAT-axis corruption control. Its copied binary and
source identity are in `target/first-release-mkhe-packing-profile-guard-native`.
It does not change parameters, produce fresh packing/security evidence or admit
a production native40 source.

The SDK response corrections and their current native-package validation blockers
are recorded separately. The 18-step Apalache result, whole workspace checks,
SDK/hardware parity, genuine deployment/soak/audit evidence and signed promotion
remain open. HSM access is not required.
