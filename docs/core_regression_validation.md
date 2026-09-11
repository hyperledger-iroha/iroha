# Core first-release regression validation

This repair follows the reported Core run with 13,623 passing tests and 973
failures. It targets the current first-release contracts. No compatibility
decoder, obsolete instruction alias, consensus bypass, or new ignored test is
introduced.

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
