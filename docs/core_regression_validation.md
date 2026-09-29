# Core first-release regression validation

These records describe focused repairs against the current first-release
contracts. They do not establish full workspace or release qualification.

The `sumeragi-main-loop-tests` feature and the previous consensus runtime's formal
scripts and source-compaction tests named in the historical commands below were
removed with that runtime; rerun those builds without that feature.

## September 20 Anonymous-PGC proof decoding stack overflow

The isolated `verified_pgc_payment_replaces_complete_table_atomically_and_replay_rejects`
test aborts on the default libtest stack in the original executable. The debugger
places the fault in nested Norito decoding of the sender's positive range proof.
Each unsigned range embeds 5,152 bytes of fixed arrays; the sender embeds two
ranges, and the complete payment embeds the sender. By-value decoder results and
temporaries multiply that storage across the call chain: the unsigned-range
decoder alone reserves 98,664 bytes, above the test's roughly 393,000-byte frame.

Both payment and bootstrap range arrays now use a private heap owner with exact
compile-time dimensions. Serialization and decoding delegate to the canonical
fixed-array codec, and decoding charges the owned array against the allocation
budget. Provers retain their existing vector allocation when converting to the
fixed owner. No additional accepted encoding or enlarged thread stack is needed.
The rebuilt unsigned-range decoder frame is 7,032 bytes. The original test passes
on the default libtest stack, and the new full-payment decode/verification test
passes with a 512 KiB stack.

All 52 focused regressions pass on the final executable in 160.45 seconds with
eight test workers and `RUST_MIN_STACK` unset: all Anonymous-PGC engine and array
tests plus the nine PGC bootstrap/payment instruction tests. Coverage includes
both unchanged proof known-answer vectors, maximum 64-account inputs, malformed
and tampered proofs, allocation limits, atomic updates and replay rejection.
The allocation test measures the canonical decoder's existing temporary costs
separately, then checks the exact additional heap charge and one-byte-short
budget rejection.

The final test build passes without diagnostics:

```sh
cargo test -p iroha_core --lib \
  --features iroha-core-tests,sumeragi-main-loop-tests,expensive-telemetry --no-run
```

Changed-file `rustfmt --edition 2024 --check`, `git diff --check`, and
`scripts/check_no_legacy_codec.sh` pass. `cargo fmt --all -- --check` reports
pre-existing differences in `state/canonical_runtime_tests.rs`,
`state/carrier_preparation/physical_publication.rs`, the test network's
`production_beacon_prepare.rs`, and the daemon's `external_software_signer.rs`.
Those files are outside this repair. Full workspace tests were not run.

## September 19 CLI output API and carrier warning repair

CLI settlement verification uses canonical network inputs and authenticated typed
execution outputs. Fixtures construct current block headers, output receipts and
Merkle proofs; a regression rejects substitution of another input's successful
output. The public-input genesis fixture installs its manifest baseline and
authenticated configured Kura geometry before startup catalog projection.

The combined test build is warning-free:

```sh
cargo test -p iroha_cli --bin iroha -p iroha_core --lib --no-run
```

Focused execution passes 90 Core tests, 51 CLI tests and the four-peer P2P
crossed-dial/restart regression. The existing expensive end-to-end settlement
proof test remains ignored. Edited-file formatting and the retired-codec guard
also pass. Full workspace
tests and the broader source-contract mutation matrix were not completed.

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
three reported adapter/worker cases and MAIN
assembly scrubbing. An exact-name audit confirms all eighteen reported failures
passed. The final P2P admission-class and handle-update selection passes all
60 tests, including capacity-one actor reuse with canonical BLS identities and
the existing retransmittable actor payload:

```sh
cargo test --locked -p iroha_p2p --lib --features test-fixtures --no-run
target/debug/deps/iroha_p2p-9e3318caaa99f24d \
  network::admission_class_tests:: network::handle_update_tests:: --test-threads=8
```

Workspace formatting,
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

## Changes

- Fixtures use registered universal authorities, exact four-validator
  committees, committed genesis identity, and complete
  asset, contract, audit, and snapshot provenance. Negative cases retain their
  intended validation boundary and state-preservation assertions.
- FASTPQ lane
  extraction validates both transcript and source-capture inventories before
  removing either.
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

ZK-X509 retains the shared 136-query, eightfold-LDE geometry. Joined MAIN
commitments, current-row queries with complete current/next Fp4 DEEP checks,
and paired FRI leaves now yield a combined codec bound of 9,204,362 bytes
under the unchanged 9,437,184-byte ceiling. Native proof and mutation tests for
this layout are pending; codec geometry does not establish actual maximum-shape
proof generation or resource qualification. Activation remains unavailable
pending independent soundness/hiding and final-candidate evidence. BFV arithmetic
diagnostics likewise do not confer production qualification. See the
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

The preceding candidate also compiled with `privacy-release-evidence` added to
that feature list. Its five focused checks covered the exact 54-artifact bound
matrix, the then-oversized X509 projection, the unchanged 9-MiB boundary,
unavailable profile/resource facts, and distinct protocol descriptors. Those
results do not validate the new joined X509 layout; its fresh checks are pending.

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
