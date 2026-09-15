# Core first-release regression validation

This repair follows the reported Core run with 13,623 passing tests and 973
failures. It targets the current first-release contracts. No compatibility
decoder, obsolete instruction alias, consensus bypass, or new ignored test is
introduced.

## September 15 block-construction stack overflow

The default-stack `merge_entrypoints_commit_in_canonical_carrier_membership`
regression aborted while reserving the stack frame for
`State::block_with_pristine_stage`, during fixture genesis construction after
Kura stored height one. The debugger identified a 741,304-byte constructor
frame, with large state values also retained by its callers.

Four existing lifecycle transaction phases now run in separate, non-inlined
borrowed helpers. Their execution order and transaction apply/drop boundaries
are preserved. The measured constructor frame is now 581,496 bytes in the
same unoptimized Linux build. The original test passes without a stack-size
override. A new pristine-stage failure regression verifies skipped lifecycle
work, discarded writes, released locks and rollback when a successful scope
is dropped.

Validation used `scripts/cargo_fast.sh --stable-local-metadata --incremental --
test -p iroha_core --lib` with exact test selection. The reported case and 34
additional checks pass, covering normal/replacement source-context ordering,
merge membership, privacy schedules and expiry, confidential transitions,
sponsor activation and governance sweeps. Workspace formatting and diff checks
pass. The Parliament source-check failures found during this run are repaired
and validated below. Full workspace tests were not run.

## September 15 Parliament source/model bindings

The checker now follows both sortition registration transitions into their
shared admission helper and broker dispatch into the consensus attestation
handler. Certificate supporter ordering tolerates Rust whitespace changes
while still requiring strict ordering in certificate validation. Broker checks
bind Parliament operations 124/125 and their explicit framed schema identities;
adding unrelated operations no longer invalidates this contract.

The full source/model checker passes. Its Python selection passes 80 tests and
218 subtests, including new complete-checker coverage and mutations of ordering,
canonical candidates, shared admission, broker operation IDs, framing, routing,
attestation and requalification. All 134 Core tests selected by `parliament`
pass with four test workers. These changes update structural validation only;
four-validator network execution and a new TLC run were not part of this check.

```sh
python3 scripts/formal/check_sora_parliament_source_contract.py
python3 -m pytest -q scripts/tests/check_sora_parliament_source_contract_test.py \
  scripts/tests/check_sora_parliament_broker_source_contract_test.py \
  scripts/tests/sora_parliament_lifecycle_corridor_source_test.py
scripts/cargo_fast.sh --stable-local-metadata --incremental -- \
  test -p iroha_core --lib parliament -- --test-threads=4
```

## September 14 privacy and lifecycle contract repair

The eighteen reported failures exposed incomplete first-release protocol and
fixture updates. The SHA3-384 outer-hash migration changed compiled manifests,
transcript challenges, native proof vectors and private-note/PQ-MASP profile
bindings without regenerating every pin. ZK-ACE and X.509 now pin the exact
current manifests, independently checked with Python's SHA implementations.
The private-note adversarial test now locates DEEP and terminal fields after
opaque 48-byte roots and checks every extension coefficient in all five DEEP
regions against the modulus and `u64::MAX`. Canonical field rejection remains
enforced by the existing decoder.

The P2P admission fixture derives waiter capacity from the production semantic
classes and exercises admission, drainage and reuse at capacity one. The timeout
test checks both validator and observer roles; the WAL-consumer worker regression
uses one consistent frozen validator identity across its runtime layers.

The live ledger fixture consumes its startup publication witness before later
transactions, matching production owner construction. Its regression checks the
unchanged live coordinator and exact persisted successor. Source guards now
cover the authenticated pending-Kura terminal transfer, recovered Decision
authority at activation and timeout-body retirement. The publication inventory
retains an exact count and explicitly checks the added retirement path's
authentication, cancellation, validation, publication and readback order. Ten
negative source mutations cover these boundaries.

Validation rebuilt Core and P2P with:

```sh
cargo test --locked -p iroha_core -p iroha_p2p --lib \
  --features iroha_core/expensive-telemetry,iroha_core/iroha-core-tests,iroha_core/sumeragi-main-loop-tests \
  --no-run
```

The rebuilt Core harness passed 790 tests in 581.72 seconds with zero failures
and four existing diagnostic ignores (`RAYON_NUM_THREADS=2`, eight test threads).
The selection covered the complete shared aggregate/transparent/private-note
STARK modules, ZK-ACE engine, X.509 DER/engine/readiness modules, privacy profiles
and lifecycle coordinator, plus the four reported X.509 proof/profile cases,
three reported adapter/worker cases, three reducer role controls and MAIN
assembly scrubbing. An exact-name audit confirms all eighteen reported failures
passed. The final P2P admission-class and handle-update selection passes all
60 tests, including capacity-one actor reuse with canonical BLS identities and
the existing retransmittable actor payload:

```sh
cargo test --locked -p iroha_p2p --lib --features test-fixtures --no-run
target/debug/deps/iroha_p2p-9e3318caaa99f24d \
  network::admission_class_tests:: network::handle_update_tests:: --test-threads=8
```

The complete 55-case source-contract census and all six Python asset
tests, including the ten new mutation controls, also pass. Workspace formatting,
diff checks, codec-retirement guards and historical archive verification pass.
The builds retain warnings in unchanged code; full Core/workspace tests were
not rerun.

The current protocol is the sole accepted V1 path. These corrections do not
qualify ZK-ACE or X.509 activation, full-size proofs, hardware parity or a full
workspace release.

## Follow-up from the 27-failure admission run

The reported full Core run passed 14,951 tests, failed 27 and ignored 32.
The failing fixtures constructed transactions with wall-clock timestamps but
validated their signed TTL against the process-wide network clock. NTS advances
from a monotonic UTC anchor; wall-clock corrections during a long run can make
these clocks diverge. The near-identical expiry deltas across otherwise unrelated
admission checks are consistent with that mismatch. NTS unit tests keep their
service, sampler and output-floor state local.

Signature, byte/attachment limits, gas, sequence, height-expiry and validation-fee
fixtures now use an explicit admission instant taken from the signed fixture.
The three tests exercising live cache/decoded ingress construct transactions
from the same network clock used by those entrypoints. Both positive and negative
validation-fee helpers use fixture time. The height-expiry test name now states
that height expiry is optional; signature-bound wall-clock TTL remains mandatory.

Regressions cover acceptance at the exact TTL deadline and rejection one
millisecond later, plus historical short-TTL fee fixtures reaching acceptance
and rejecting a signature from a different payload. Production clocks, TTL
limits and validation order are unchanged.

The fresh harness rebuilt without warnings in 9 minutes 57 seconds:

```sh
cargo test --locked -p iroha_core --lib \
  --features expensive-telemetry,iroha-core-tests,sumeragi-main-loop-tests --no-run
RAYON_NUM_THREADS=2 target/debug/deps/iroha_core-32e940501cee3ff7 \
  tx::tests:: validation_fee_admission_tests:: time::tests:: --test-threads=16
```

The selection passed all 442 tests with no failures or ignored tests in 74.30
seconds. An exact-name audit confirms all 27 reported failures passed, including
the renamed height-expiry case. The filters also cover related transaction
history and runtime tests. Changed Rust sources remained byte-identical throughout
execution. Scoped Rust formatting, diff checks and historical-archive verification
pass. Full Core/workspace execution was not rerun.

## Group 03 mandatory transfers and IVM fixtures

The reported group passed 157 tests, failed seven and ignored two. The numeric
movement planner selected typed mandatory-debit exceptions for outbound controls,
but its later balance precheck reapplied ordinary outgoing availability. That
precheck now receives the same typed control policy. Retained mandatory debits
preserve their outgoing exceptions; receiver availability and holding limits
remain enforced except for the existing finality-owned staking/moderation custody
exceptions. Scope, source authority, custody, usage, privacy, precision and checked
balance arithmetic retain their separate admission checks.

The other six failures were fixtures asserting or constructing obsolete inputs:

- ZK referendum guards receive exact ballot grants and check absent, proposed,
  closed, too-early and too-late referenda without mutating election state.
- Composite state keys decode as canonical `NoritoBytes(StatePath)`, including
  a key exceeding the `Name` size bound.
- CoreHost rejects low-level polynomial-opening envelopes at decode admission.
  A real registered Pallas proof checks successful verification and curve-policy
  rejection; Goldilocks cannot be admitted as an IPA registry group.
- Host asset definitions carry explicit owning domains. Shadow/native parity
  initializes incarnations at genesis and asserts the actual minted balance;
  insufficient transfers assert the precise balance error and unchanged funds.

Regressions exercise every typed movement policy's outgoing availability,
incoming availability, holding limit and insufficient-balance behavior. Oracle
integration controls distinguish mandatory penalties from ordinary transfers and
preserve receiver restrictions.

A related Group 02 parallel rerun exposed direct slash/restitution fixture writes
entering another test's global FASTPQ witness capture. Those fixtures now hold
the existing execution-witness guard through direct execution and transcript
draining. The mixed direct/signed-block test releases that non-reentrant guard
before signed validation acquires its own. Runtime recording policy is unchanged.

Validation rebuilt the shared Core feature graph:

```sh
cargo test --locked -p iroha_core --lib \
  --test iroha_core_group_02 --test iroha_core_group_03 \
  --features expensive-telemetry,iroha-core-tests,sumeragi-main-loop-tests --no-run
target/debug/deps/iroha_core_group_02-d19bd85425b1bbb1
target/debug/deps/iroha_core_group_03-eed19256d2d47a07
target/debug/deps/iroha_core-cb6ba39648f33023 \
  smartcontracts::isi::asset:: smartcontracts::isi::oracle:: \
  smartcontracts::isi::world::isi::tests::direct_zk_ballot \
  smartcontracts::isi::world::isi::tests::direct_plain_and_low_level_zk_ballots_require_exact_scoped_permission
```

The complete Group 03 run passes 165 tests with two existing ignores, including
all seven repaired cases (the invalid disabled-Goldilocks fixture is replaced by
`core_host_enforces_registered_ipa_curve_policy`). After witness isolation, Group
02 passes all 27 tests in three consecutive runs with normal parallel execution;
the focused Core selection passes all 105 tests, for 297 distinct passing tests.
Workspace formatting, diff checks, codec-retirement guards and historical-archive
verification pass. The builds retain warnings in unchanged code. Full Core and
workspace runtime suites were not rerun.

## Group 01 integration fixtures

The reported group passed 56 tests and failed 11. Its fixtures and assertions
still assumed implicit asset ownership, zero-amount mints, unscoped data-trigger
registration and execution-ordered block hashes. The production paths enforce
the current first-release contracts; this repair updates their tests:

- Asset definitions carry explicit owning domains. Domain teardown removes
  owned definitions and their balances while preserving universal accounts and
  unrelated domains.
- Holder-query fixtures require zero-mint rejection, observe a valid positive
  mint, and exclude the holder after a full burn updates the holder index.
- Manifest activation rejects missing or wrongly scoped global data-trigger
  permission, then succeeds with a direct capability on the activator scoped
  to the actual trigger authority, including the default contract subject.
- Scheduler tests observe independent account metadata events and compare them
  with independently sorted entrypoint hashes. Serialized hashes and results
  retain payload order; a new regression moves a failed transaction through
  every payload position and checks its result remains attached to that index.

The longer scheduler run also exposed concurrent fixture contamination of the
global execution-witness recorder: the full group failed FASTPQ source ownership
while the isolated scheduler and serial group passed. The two direct FASTPQ
transfer tests and five governance bond/citizenship tests now hold the existing
execution-witness guard through their direct execution and transcript draining.
Typed governance movements can publish transcripts even without a transaction
call hash. Normal block validation already owns this guard.

Validation passes with the shared Core feature graph:

```sh
cargo test --locked -p iroha_core --test iroha_core_group_01 \
  --features expensive-telemetry,iroha-core-tests,sumeragi-main-loop-tests
```

The complete group passes all 68 tests with no failures or ignored tests,
including all 11 reported failures and the new result-index regression. The
Cargo run and three further executions of the same harness all pass with normal
parallel test execution (10.62, 10.94, 11.12 and 11.29 seconds). Workspace
formatting and diff checks pass. Full Core/workspace execution was not rerun.

## Follow-up from the 57-failure Core run

The next reported full Core run passed 14,770 tests and failed 57. This repair
addresses the current startup, execution and certificate contracts:

- Empty journal replay keeps admission closed until exact State/Kura
  reconciliation completes. The runner's lane-evidence repair fence now accepts
  that closed empty replay and rejects an already-open gate. Fresh workload
  fixtures complete their empty journal receipts before admission; recovery
  fixtures retain their replay quarantine and exact FIFO assertions.
- Predecessor fixtures stage their ordinary execution frontier before applying
  verified finality. The Native AMX receipt binds the actual first coordinator
  slot and its exact ownership proposal. Finality alone still cannot create an
  unexecuted frontier.
- Lane-history retention authenticates exact canonical carrier coordinates
  against signed finality and the complete merge entry. It can therefore
  preflight a missing reverse-index repair without requiring that index first.
  Conflicting retained records and missing or malformed finality still reject
  the frontier, and authentication performs no repair writes. Regression tests
  cover both an evicted carrier body and corrupt retained inline bytes.
- A late validation failure protected by the timeout certificate's Prepare QC
  retains the exact certified-body report. Applied ordinary lane completion
  checks historical predecessor evidence and the current applied frontier.
- Relay status publication stays under the lane lifecycle fence so retirement
  cannot prune the cache before a delayed publisher reintroduces the envelope.
  The identity-drift test observes the process-global cache in an isolated
  subprocess, preserving its state and status assertions under parallel tests.

The reviewed harness rebuilt without warnings in 6 minutes 52 seconds using
`cargo test --locked -p iroha_core --lib --features expensive-telemetry,iroha-core-tests,sumeragi-main-loop-tests --no-run`.
Its focused selection passed 1,477 tests in 712.26 seconds, including every one
of the 57 reported failures, all 55 consensus source contracts, the relay
publication race regression and both new retention regressions. The remaining
Kura modules passed 261 tests in 127.59 seconds. These disjoint selections cover
1,738 passing tests, including all 1,208 Kura tests, with no failures or ignored
tests. The 24 changed Core inputs and copied executable remained byte-identical
throughout this validation.

An earlier diagnostic build completed a broader block, Queue and consensus
selection with 1,571 passes and ten failures. All ten were among the reported
57 and pass on the reviewed build; that earlier run found no additional failing
cases. Its high-volume Queue barrier case also passed.

Workspace formatting, diff checks, codec-retirement checks, six source-asset
tests and historical-archive verification pass. The focused Python mutation
test rejects both empty-replay quarantine bypasses. The optional positive
autonomous-terminal source-contract check remains blocked by an existing Kura
include inventory omission (`tests/14_pipeline_and_lane_frame_owners.rs`);
the starting revision and repaired source have the same 105 include paths.
Full Core/workspace execution and four-validator qualification were not rerun.

## Follow-up from the 23-failure Core run

The subsequent full Core run reported 14,764 passes and 23 failures. Its retained
executable predates the source correction that binds each validation fixture's
service peer and private signer to the same selected validator. That correction
keeps the signer identity assertion intact and covers ten reported failures.

The remaining repairs address these causes:

- FASTPQ lane startup changes the process-wide digest acceleration state. Every
  lane test now holds the shared acceleration guard through worker completion,
  matching the digest tests' exclusion and restoring state on exit. The stale
  pending-input assertions still require CPU fallback without a GPU fault.
- Kura's intentional prune panics poison the process-wide consensus transition
  gate. Clearing poison after catching a panic leaves a race with unrelated
  tests. All five crash fixtures now run in separate exact-test subprocesses;
  recovery assertions and production fail-stop behavior remain intact. The
  duplicate wrapper around the same crash matrix is removed.
- DER and RFC 5280 descriptor pins still hashed the previous geometry after the
  descriptor strings were updated. Their SHA-256 pins now bind the current
  masking, composition and compact-CA geometry. Terminal and capacity assertions
  remain unchanged; this does not activate ZK-X509.
- A delayed output belonging to a protected Ready Apply retains Completion
  priority even when Apply dispatch is blocked. Draining Runtime still requires
  the exact predecessor proof. The outer ingress regression also checks that
  Producer work cannot take that priority.
- Lifecycle recovery tests distinguish a missing inherited Validate owner from
  a fresh standalone Prepare owner, and require an applied successor to consume
  its authenticated Validate retry seal. They no longer demand a NoSuccessor
  tombstone when a real Apply successor exists.

The fresh harness built with the shared feature command below. An exact-name
rerun of all 23 reported failures passed with 16 test workers in 93.49 seconds.
The broader selection passed 1,755 tests with no failures in 253.87 seconds;
one existing retained-ledger diagnostic requires an external incident file and
remains ignored. The two runs cover 1,757 distinct passing tests. The changed
Rust sources and copied executable remained unchanged throughout validation.

Using the copied harness as `CORE_TEST_BIN`, the broader command was:

```sh
RAYON_NUM_THREADS=2 "$CORE_TEST_BIN" \
  fastpq:: kura:: \
  sumeragi::v2_effects::tests::certified_body_fence_supersession:: \
  sumeragi::v2_lifecycle_coordinator::ledger::tests:: \
  sumeragi::v2_lifecycle_coordinator::work_registry::tests:: \
  sumeragi::v2_runner:: --test-threads=16
```

This includes all 184 FASTPQ and 1,197 Kura tests. Workspace formatting,
diff checks, codec-retirement guards and historical-archive verification pass.
Full workspace execution and hardware qualification were not rerun.

The optional `python3 scripts/formal/check_sumeragi_v2_multilane_models.py` check
still reports 233 existing source-binding errors in Kura, QueuePlan, lane
planning and two test anchors. The affected items were compared with the
starting revision; none of these errors was introduced by this patch. The
changed runner binding is current. This is not a full formal qualification.

## Follow-up from the 10-failure Core run

The reported full Core run passed 14,967 tests, failed ten and ignored 32.
All ten failures reproduce in the retained test executable. Their causes are
incomplete fixture authority and an incorrectly scoped source assertion:

- Unlocked certified-view retention refreshes merge candidates immediately.
  Its fixtures now persist the exact canonical parent chain and signed finality
  in Kura, matching the committed State parent. The shared sidecar-server
  fixture provides that same durable foundation for rollover and recovery.
- Candidate-provider fixtures install the runner's exact unlocked reducer view
  before admission. Multiroute geometry is established before genesis; the
  single-route QueuePlan fixture freezes its authority before voting journals
  open. Repeated retries retain all FIFO and autonomous-ownership assertions.
- A new regression exercises both candidate providers with absent and stale
  reducer directives. Both defer without closing output or publishing proposal
  ownership, then resume after the exact view is installed.
- The Apply ordering assertion is scoped to the Apply-owned recovery branch.
  The earlier decided Validate-sidecar drain has its own sealed permit. The
  assertion still requires Decision cleanup, terminal output reconciliation
  and the open-ingress permit before the Apply drain.

Production frontier, signing, quorum and recovery guards are unchanged. No
compatibility path or ignored test is introduced.

The fresh Core harness built without warnings in 9 minutes 57 seconds:

```sh
cargo test --locked -p iroha_core --lib \
  --features expensive-telemetry,iroha-core-tests,sumeragi-main-loop-tests --no-run
```

The exact ten reported failures and the new reducer-view regression pass with
16 test workers in 25.29 seconds. Using that executable as `CORE_TEST_BIN`, the
broader selection passes all 455 tests, with no failures or ignored tests, in
221.50 seconds:

```sh
RAYON_NUM_THREADS=2 "$CORE_TEST_BIN" \
  sumeragi::v2_lane_work:: \
  sumeragi::v2_lifecycle_coordinator::launch:: \
  sumeragi::v2_runner:: \
  sumeragi::v2_apply::tests::merge_frontier_ \
  apply_barrier_handoff_retires_exact_live_proposal_and_lane_losers \
  --test-threads=16
```

This includes all shared sidecar-fixture callers and the eight State/Kura
frontier tests for publication races, missing or contradictory parent headers,
damaged indexes and retained signing authority. The changed Rust inputs and
executable remain byte-identical throughout the broader run. All nine focused
lifecycle source tests also pass independently without Core dependencies;
six temporary source mutations confirm that the Apply assertion still rejects
missing handoffs, reordered recovery and missing ingress authority. Changed-file
formatting, diff and historical-archive checks pass. Full Core/workspace and
four-validator network suites were not rerun.

## Changes

- Fixtures use registered universal authorities, exact four-validator
  committees, signed revision-4 DA, committed genesis identity, and complete
  asset, contract, audit, and snapshot provenance. Negative cases retain their
  intended validation boundary and state-preservation assertions.
- Kura recovery reads an authenticated relabelled storage pair and preserves
  uncertain staged claims until their durability is known. Bounded generic
  atomic sidecars are discarded before startup repairs reserve capacity;
  named protocol temporaries retain their authenticated recovery paths.
  Closed lanes retain their committed, pinned close committee for global
  drain certification while ordinary proposal authority remains closed. FASTPQ lane
  extraction validates both transcript and source-capture inventories before
  removing either. SCCP admission checks retained route and replay state before
  expensive proof work and stages replay mutations until settlement succeeds.
- The Initial executor admits the native asset-escrow lifecycle through its
  ownership and transfer-control checks. Metadata limits measure JSON text.
  SM helper failures during gas quotation now record their failure metric.
  Soracloud shared-lease admission reserves the complete audit sequence before
  writes, including a queued renewal's additional event.
- Unsupported citizen-bond instructions are removed from the instruction
  API, registry, and generated identity fixtures. Canonical record identities remain
  documented in [the schema contract](../specs/norito_schema_identity.md).
- Proof fixtures and known-answer vectors bind the current network, circuit,
  transcript, and STARK geometry. Test verifier dispatch checks supplied key
  bytes through the same circuit-specific cache validation as production.

ZK-X509 diagnostic registration uses the shared 136-query, eightfold-LDE
geometry. Its combined maximum proof projection is 19,156,074 bytes, exceeding
the unchanged 9,437,184-byte production ceiling. Full MAIN proving rejects this resource
preflight before witness preparation or entropy. Activation remains unavailable;
no soundness certificate is installed. BFV arithmetic diagnostics likewise do
not confer production qualification. See the
[privacy closure record](../specs/privacy_first_release_closure.md).

The provider-ingest failures were already corrected in the starting revision:
the fixture's approval epoch must precede its pin retention epoch, and changed
order windows must update both the outer record and canonical payload. Current
provider tests pass without changing their production validator or defaults.

## Validation

Core tests use one shared feature graph:

```sh
cargo test --locked -p iroha_core --lib \
  --features expensive-telemetry,iroha-core-tests,sumeragi-main-loop-tests --no-run
```

Compilation also passes with `privacy-release-evidence` added to that feature
list. Its five focused checks pass: the exact 54-artifact bound matrix, rejection
of the oversized X509 projection, the unchanged 9-MiB boundary, unavailable
profile/resource facts, and distinct protocol descriptors.

The resulting executable is copied before focused runs so subsequent Cargo
builds cannot replace a running artifact. Heavy proof selections use bounded
test concurrency; fresh selections use `RAYON_NUM_THREADS=2`.

Completed broad selections combined with subsequent exact failure reruns cover
the following scopes. These counts describe combined evidence, not one fresh
full-suite command; the scopes overlap where noted.

| Scope | Passing tests |
| --- | ---: |
| Queue | 724 |
| Gossiper | 121 |
| Executor | 192 |
| World, main ISI tests, and registry dispatch | 449 |
| IVM host | 334 |
| Snapshot | 125 |
| SNS and Musubi | 178 |
| Query | 512 |
| Kura, complete fresh run after startup cleanup change | 1,196 |
| Drain and merge authority regressions | 15 |
| FASTPQ source context | 12 |
| DataModel registry and generated-record identity, with governance | 366 |
| Privacy/FHE Core checks, including CA mutations, release evidence, ISI regressions, and complete proof KATs | 839 |

The first seven rows cover 2,123 distinct tests with no ignored cases. Twenty
additional focused SCCP admission/atomicity checks overlap the World row. The
AXT restart, Orchard dependency, and two tiered SCCP hydration regressions also
passed. DataModel coverage includes canonical wire roundtrips and rejection of
removed instruction names.

The IO and projection STARK vectors were regenerated after correcting their
Merkle-hash descriptor. Both complete KAT tests pass on the optimized executable,
including honest verification, wire length and cap checks, canonical byte
re-encoding, uniqueness of all 136 query indices, and the corrected expected
digests. That fresh two-test run completed in 3,011.50 seconds.

`cargo fmt --all -- --check`, `git diff --check`, and
`scripts/check_no_legacy_codec.sh` pass. The shared BFV identifier fixtures pass
the focused Kotlin tests and JavaScript helper test. Swift is unavailable in
this environment; the full JavaScript native cases require an absent host addon.

The exact-name audit covers all 973 failures in the original report, including
source-verified test renames. All 973 map uniquely to passing test results: 923
have numbered immutable-artifact evidence and 50 have earlier focused execution
evidence from this repair. Combined focused evidence covers 7,664 distinct Core
tests. Completed subsystem runs and subsequent exact failure reruns are separate
evidence; an earlier failing artifact does not qualify a later source correction.
Full workspace execution, strict workspace
Clippy, hardware qualification, and four-validator network qualification are
separate gates.

The broader protocol selection finished with 1,009 passes, two failures from
its earlier artifact, and 12 existing ignored tests. Both failures were the lane
Commit-vote fixtures that omitted their prerequisite Prepare QC; their corrected
tests pass on a subsequent rebuilt artifact. No other failure remained in that
selection.

The broader State selection finished with 1,539 passes and 39 failures from its
earlier artifact. All 39 corrected cases pass on subsequent rebuilt artifacts.
The last was the full-capacity hosted-service restore regression, which passed
in 1,512.93 seconds with all 4,096 reporter checkpoints and the complete audit
history.

The final exact-name audit also reran the three AXT CoreHost and two trigger
regressions. All five pass on the rebuilt executable. Their repairs advertise
the actual single committed fragment, require the replay-ledger entry to exist,
and check typed policy errors and the complete trigger store declaration.

The final snapshot run uses temporary command-line optimization overrides for
Core and its serialization dependencies, keeping the normal test profile's
debug assertions and overflow checks. The complete KAT run used level 1 for the
three serialization dependencies; the final snapshot run uses level 2:

```sh
cargo --config 'profile.test.package.norito.opt-level=2' \
  --config 'profile.test.package.iroha_model_base.opt-level=2' \
  --config 'profile.test.package.iroha_data_model.opt-level=2' \
  --config 'profile.test.package.iroha_core.opt-level=1' \
  --config 'profile.test.package.iroha_core.codegen-units=64' \
  test --locked -p iroha_core --lib \
  --features expensive-telemetry,iroha-core-tests,sumeragi-main-loop-tests,privacy-release-evidence \
  --no-run
```

The final build passes without changing repository profiles. The full-capacity
hosted-service snapshot regression constructs 4,096 reporter
checkpoints and replays their complete audit history. Repeated whole-lease
commitment encoding makes this test expensive; CPU activity during that work is
not evidence of a lock deadlock. The final passing run restores the canonical
rollover and rejects deletion of its opener. Its final assertion
checks the exact lifecycle-reconstruction rejection and audit field, which occur
before the later rollover-specific check.
