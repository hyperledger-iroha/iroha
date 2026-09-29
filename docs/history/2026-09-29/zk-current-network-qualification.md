# Current-source ZK network qualification

Status: coherent component build passes; signed SDK fixture execution exposes a
FASTPQ lane/manifest authority defects. Four-validator runtime controls remain unexecuted. This record
replaces the retired replay-binding network plan for the current APIs. It does
not qualify a complete IVM execution proof or current production release.

## Fixture migration

The shared proof fixture uses `ConfidentialProver` to create a real one-note full
unshield, with the canonical key, schema, circuit identity and proof-byte cap.
Its synthetic network and commitment root are self-consistent public statements.
Generic `VerifyProof` records verification outcomes; these synthetic statements
do not authorize a ledger spend or change the confidential tree.

The native negative clones the valid proof and flips exactly one `PROF` byte.
The test requires its `I10P` bytes and all envelope identity fields to remain
unchanged. The network-binding control checks all eight actual public columns:
the network tag and active nullifier change, while other columns stay fixed.
Fresh proof randomness alone cannot satisfy that assertion. The full-capacity
wallet's wrong-role negative uses the retained confidential-transfer relation.

The proof-record scenario quotes the exact prepared transaction, checks the
payer and gas bound, installs the quote and signs once before submission to all
four peers. Fee failures cannot serve as proof-verification evidence.

The current `IvmProved` case first executes an ordinary contract call and checks
the resulting counter. It then requires the exact unavailable-execution-relation
rejection at every ingress, both with an empty overlay and changed nonempty
overlay. Early typed admission rejection and durable committed rejection have
separate evidence checks; an early rejection need not fabricate a committed
record. Independent Applied barriers and all-peer counter reads establish
progress and unchanged state. Transport, timeout, expiry and fee errors cannot
satisfy the rejection control.

## Captured candidate and plan

The managed `zk-current-network` checkout starts at
`24789950586efd7fa3a6cc73a96f4acd5d760bf9` and contains 32 exact local amendments
across a 20,543-file source census. Capture completed without source drift.
The source manifest SHA-256 is
`bec3119129d80d4587a665df0e93a16f52ab5191f3f8eb17bb65e7d6cc893b92`.
The candidate's inherited Git root was corrected only in its worktree-specific
configuration; the shared repository configuration stayed byte-identical.

The reviewed normal-Cargo runner builds the daemon, CLI and attachment sanitizer,
plus the `queries_and_proofs`, `events_and_triggers` and `core_api` harnesses with
the actual Halo2/STARK features. It retains all six compiler-reported binaries,
checks their hashes before every invocation, checks exact source manifests, and
retains the executed runner. There are eight exact native controls followed by
six sequential four-validator scenarios: proof records, events, queries, ordinary
transfer simulation, the full-capacity wallet and unavailable IVM execution.
Mandatory-network mode rejects skipped execution. Historical successful replay
tests cannot substitute for this current candidate.

Independent source review accepted the relation/key/schema migration and the
fee, public-column and single-byte-corruption corrections. Scoped Rust formatting,
diff checks and the retired-codec guard pass.

## Normal build failure

The ordinary debug production build, with two Cargo jobs and no profile overrides,
exited 101 after 316.19 seconds. All 20,543 candidate files remained unchanged.
Core reports 23 errors: 22 unresolved imports from the unfinished incoming
Sumeragi migration and an incomplete lane `Executor` implementation missing
`build_control_witness`, `drive_control` and `receive_application_control`.
The same migration failure is documented in the [merge validation](merge-validation.md).

No daemon/CLI/sanitizer bundle was captured, and neither the eight native controls
nor six network scenarios ran. Their ZK fixture migration remains uncompiled.
Qualification requires the migration to be completed on a new captured source;
restoring retired consensus modules or adding no-op trait methods would not
satisfy it. The frozen candidate stays intact for standalone FASTPQ qualification.
The failed command, log, post-failure source census and receipt are retained under
`dist/zk-remediation/2026-09-29/current-network-20260929T0540Z/`.

Local retained evidence is under
`dist/zk-remediation/2026-09-29/current-network-candidate/`, with the selector plan,
review receipts and `run-current-network.py` in its parent directory. These are
local reproducibility records, not signed release artifacts.

## Coherent component retry and remaining compile repair

A newer candidate starts at `ce9a01dcdc32bcf9edaf3fcdd182872eb4d4ac51` and
uses a dedicated target whose local dependency outputs have retained source,
path and byte identities. The normal Core/model/Torii test build succeeds in
866.392 seconds. The following `consensus_and_da` harness build fails in
647.665 seconds with two test-source errors: an absent `FindAccountById` import
and a stale `retransmit_interval_ms` diagnostic variable. All 153 emitted local
artifacts satisfy their provenance checks; both source guards remain unchanged.

The two-file repair imports the actual account query, reports the real block-sync
gossip duration already used by the unchanged observation calculation, and removes
an unused fixture identity import. Independent review passes. Every assertion and
time bound remains. Its normal retry awaits completion of the candidate's current
native readers; no four-validator result is claimed. The failure and amendment are
retained in `bfv-api-components-20260929T093117Z` and
`integration-idle-smoke-compile-repair` under the same dated local evidence root.

## Canonical SDK fixture prerequisite

The later read-only Kagami control rebuilt normally against the coherent
`ce9a01dcdc32bcf9edaf3fcdd182872eb4d4ac51` candidate and verified every reused
local artifact against the preceding compiler output closure. Its original
`sdk_fixture_retains_the_entire_verified_native_artifact_and_all_request_rows`
control fails after 14.867 seconds: the production executor rejects signed
fixture block 4 with `Invalid`, before export. No fixture bytes are generated;
source and retained executable guards pass. This reproduces the earlier failure
with verified dependency provenance, so stale artifacts do not explain it.

The initial diagnostic amendment called the existing actor-backed test logger
from the synchronous fixture constructor. Although compilation passed, the run
failed before fixture construction because no Tokio reactor was running. That
failed amendment is retained in `native-sdk-fixtures-diagnostic-20260929T104608Z`.
The reviewed correction uses a synchronous warning subscriber from the existing
workspace dependency, with the corresponding normal Cargo lock update. It leaves
admission, execution, signed work, assertions and output generation unchanged.

The corrected normal build passes. Its original control fails after 15.940 seconds
at block four with the specific production error: FASTPQ source lane 1 has no
frozen active incarnation. Source/executable guards and local artifact provenance
pass; no SDK fixture bytes are generated. The failure is retained in
`native-sdk-fixtures-synchronous-20260929T105259Z`.

Read-only diagnosis finds that native routing uses the committed
`World.sumeragi_lanes` records, while FASTPQ still captures nonzero lane identities
from the older Nexus catalog. A native lane created at genesis becomes routable
at global height four, exactly the failing boundary. The correction must freeze
the native record's exact identity under the same source-height admission rule;
seeding the old catalog in a fixture would hide the production defect. The
minimal correction and activation/closure/frozen-state regressions are in progress.
Kotlin fixture consumers and current network qualification remain pending.

## Native lane identity correction and remaining manifest failure

The production correction freezes exact nonzero identities from committed native
lane records admitted at source height h−1. Lane zero retains its authenticated
genesis identity; absent native records and closing lanes cannot borrow an old
Nexus catalog identity. The normal Core/Kagami build passes in 574.752 seconds.
All 13 State context, 12 source context and three routing controls pass, including
activation, closing, exact marked identity and frozen-state mutation cases.

The same binary passes 69 and fails 29 inventory controls. Their fixtures did not
bind original recorder ownership or materialize the configured manifest source.
The reviewed fixture amendment preserves the original assertions and acquires
recording before effects. A separate production defect is corrected: losing the
original recorder now latches terminal local capture failure through the existing
atomic guard without draining another execution's recorder. The new missing,
reset and foreign-recorder regression brings inventory coverage to 99 controls;
its normal native retry remains pending.

The unchanged signed Kagami control now commits block four but reports a failed
transaction. A one-file assertion diagnostic retains the actual result, height
and entry index. Its normal build passes in 208.606 seconds and the control fails
in 17.739 seconds with `lane 1 is absent from the installed manifest registry
snapshot`, at height four, entrypoint one. Thus the first identity repair exposes
a second authority mismatch; it does not yet qualify canonical SDK generation.

All these runs retain source, binary and local compiler-output guards. Evidence
is under `fastpq-native-lane-normal-20260929T110745Z`, its
`remaining-independent-controls`, and `kagami-execution-result-normal-20260929T112602Z`
in the dated local remediation directory. No canonical fixtures were generated,
and no production check or signed workload was weakened to obtain a pass.

The recorder amendment's normal Core build passes in 517.449 seconds with exact
source and dependency-output guards. Its launcher then fails before native tests
because Python 3.9 does not accept `zip(strict=True)`; the failed runner and error
are retained. A corrected, independently reviewed no-Cargo continuation uses the
same copied executable. The 13 State and 12 source context controls pass again.
Inventory improves to 94 passed / five failed / zero ignored. The new missing,
reset and foreign-recorder sticky-failure control passes. All five remaining
failures are in `owned_sources_tests`, whose source fixture lacks an authenticated
execution route; they now get past the earlier recorder/manifest failures.
No authority check is relaxed. The route split and exact signed-source fixture
migration remain in progress. Evidence is retained in
`fastpq-recorder-normal-20260929T113647Z/native-controls-retry1`.

The remaining three native routing controls and original-recorder ownership
control pass on that unchanged executable. The combined census is 123 passed,
five failed and zero ignored, with all 128 selected names executed exactly once.
`fastpq-recorder-normal-20260929T113647Z/combined-result.json` reconciles both
reader runs and retains the launcher interruption. The five failures prevent
claiming a passing inventory suite or canonical SDK fixture readiness.
