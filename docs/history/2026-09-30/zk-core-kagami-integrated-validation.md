# Core/Kagami integrated validation, 2026-09-30

This record tracks native validation directly in
`/Users/takemiyamakoto/devstuff/iroha` on `optimizations`, starting at
`f7444cd4e4afbc9e41772264dbbde3173e5e2c48`. It does not establish release
qualification. Root prose may change concurrently; each run retains hashes of
every tracked code, configuration, SDK and fixture input, plus normal Cargo
artifact provenance, under the untracked `dist/zk-remediation/2026-09-30/` tree.

## Initial integrated compilation

The command is:

```sh
cargo test --locked --offline -p iroha_core -p iroha_kagami \
  --lib --bin kagami --no-run --message-format=json-render-diagnostics
```

The primary target is `target/zk-core-kagami-primary-20260930`, with
`CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, `RAYON_NUM_THREADS=4` and the
unmodified repository profiles. The host toolchain is Rust 1.93.1,
`aarch64-apple-darwin`. No stack-size, compiler-wrapper or profile override is
used. The build receipts verify source stability and authenticate every reused
local compiler artifact against a preceding recorded output hash.

| Run directory | Result | Native finding |
| --- | --- | --- |
| `primary-core-kagami` | Compile failed, 328.13 s | The private-note fixture leaf position `0x89ab_cdef` inferred an overflowing `i32`. |
| `primary-core-kagami-retry1` | Compile failed, 26.06 s | Three ZK library consumers could not call journal-prefix or terminal-record helpers hidden by `cfg(test)`. |
| `primary-core-kagami-retry2` | Compile failed, 205.68 s | Core tests retained three unused private relation imports, two unavailable snapshot fixture constructors and one direct private witness-field access after extraction. |
| `primary-core-kagami-retry3` | Compile failed, 190.70 s | The PQ-MASP fixture's nested note also needed its existing public accessor. |
| `primary-core-kagami-retry4` | Compile passed, 320.45 s | Source-state 15/15 and source-map 12/12 passed; inventory 98/99 passed and reproduced the fee-fixture failure below. Further scheduling stopped at a group boundary for coordinated source updates. |
| `primary-core-kagami-retry5` | Compile passed, 246.25 s; source guard refused controls | The fee fixture compiled. The reviewed compact-MDS validation-script pin changed during Cargo execution; it is outside all compiled local packages, but the strict whole-tree guard correctly halted every next test launch. |
| `primary-core-kagami-retry6` | Compile passed; incomplete native group | 129 controls passed in complete groups, then 81 individual output-producer controls passed before the duplicate capture-guard fixture deadlocked. |
| `primary-core-kagami-retry7` | Compile passed, 251.40 s; 250/251 selected controls passed | Focused periodic regression also passed, and all 90 output-producer controls passed. One policy-routing fixture has an invalid autoscale alias; its correction is prepared. Source/binary guards and exact test census pass. |

The leaf position now has its intended `u32` type, preserving all 32-bit tree
positions and all fixture assertions. The ZK caller audit found the recovery
archive and coordinator store also call those validation helpers in normal Unix
library builds. The final correction removes the erroneous gates from the
existing prefix-validation implementation and terminal-record validation
wrapper. It does not expose persistence failure injection or the unintegrated
complete-journal scan. The intermediate test-utils-only gate correction in
retry2 is retained as failed-build history, not the final candidate.

Core consumer corrections remove the three unused imports and use the existing
`PqMaspWitnessV1::inputs()` and `PqMaspInputWitnessV1::note()` accessors. The
adjacent input/output/note payload accessor surface and remaining explicit
Core/Kagami witness uses were inspected. The three related private-note snapshot
fixture constructors now have the same test-utils visibility as their FCMP
counterpart. That source amendment was coordinated with the X509 owner's two
native-derived profile hash updates after its first native binary was retained.
All eight Rust files changed by this validation task pass targeted Rustfmt checks,
and `git diff --check` passes.

The current native inventory failure was
`known_rejected_call_capture_and_typed_protocol_extra_remain_owned`: ALICE's
actual balance was `1,000,000,011`, while the retained assertion expected
`999,999,839` after both pipeline fees. The fixture selected an arbitrary local
coin as the network fee asset. `resolve_network_xor_asset_definition` requires
the committed network XOR identity, including admission with a zero Nexus fee,
so both calls rejected before pipeline fee collection. The fixture now selects
the canonical network XOR only in its `fee_and_protocol` branch. All balance,
rejected-call, custody-release and source-ownership assertions remain intact;
retry6 passes all 99 inventory controls with this correction. No production fee resolver changed.

## Periodic-trigger capture fixture deadlock

`primary-core-kagami-retry6` compiled normally and passed source-state 15/15,
source-map 12/12, inventory 99/99 and native-routing 3/3 with unchanged source and
retained binary hashes. The output-producer group completed 81 individual
controls before its repeated-periodic control stopped making progress; the
90-control group is incomplete and does not count as passing.

A read-only macOS `sample 5470 3 10` traced the blocked test directly into
`exec_witness_guard` and the nonreentrant capture mutex. The fixture already
retained the capture guard returned by `recorded_component_block`, which calls
`begin_exec_witness_capture` and starts the block capture. A second acquisition
inside `repeated_periodic_matches_bind_distinct_positions_and_use_time_actions`
therefore waited on its own live guard. The repair removes only that redundant
acquisition and its matching drop. Every assertion and the original capture
guard lifetime remains intact. The other callers of `recorded_component_block`
in the time, callback and producer fixture modules have no duplicate capture
acquisition.

The retained `source-at-hang.json` exactly matches the initial source manifest;
`result.json` records 129 passes in complete groups and 81 individual passes in
the incomplete group, its thread sample hash and exact native/runner process
identities. No process was killed or signalled. The runner's `STOP` interlock
prevents any later launch if the waiting native process ever exits. Subsequent
source amendments intentionally end this candidate interval, and a new native
build must run the exact periodic regression followed by all selected controls.

## Control and fixture census

The historical selected plan contains 246 Core controls and one Kagami complete
execution-artifact exporter control. The current source retired that Kagami
exporter and its SDK execution-artifact fixtures. Current finality-inspector
tests and model-owned status/lane fixtures are separate coverage; they cannot
substitute for an authentic finalized network execution corpus. The runner
retains both the historical plan and the actual binary test census, with any
additions or removals reported explicitly.

The genuine Rust `kotlin-fixture-gen` build and all ten output modes pass in
`primary-kotlin-fixture-retry1`. The retained executable SHA-256 is
`58bc6337cc288aa921f62febe0afebebc622fd0a77fd05066cd28fc23c6eb2aa`, and its
complete tracked-source manifest SHA-256 is
`1c2b00de71fcafe3293f8fc216cc934416cd112f75933f235184ae15f1548af2`. The command is
`cargo build --locked --offline -p kotlin-fixture-gen --features dev-tools --bin kotlin-fixture-gen --message-format=json-render-diagnostics`.
Its initial build completed normally in 228.90 seconds, but the whole-tree seal
correctly rejected the concurrently coordinated privacy fixture/KAT changes.
Those two files are outside every local package in the generator's compiled
closure. That closure check, unchanged root workspace/lock files, retained
artifact hashes and the successful 0.79-second warm build with a stable complete
source manifest are recorded explicitly.

All ten modes returned zero: account registration; global and scoped asset
transfer; identifier claim; contract lifecycle; hidden RAM-FHE program;
multisig accounts; FASTPQ balance keys; native Sumeragi status; native Sumeragi
lanes. Raw output hashes and byte counts are retained in `result.json`.
Generated model fixtures must still be consumed by normal Kotlin/Java tests.
They are not evidence that a four-validator network finalized the represented
execution.

## Current selected-control and supplemental outcomes

Retry7 captures 16,407 code/config/fixture inputs in `source-before.json`
(SHA-256 `0124477ae8a000e36b5289535243e742b5d3b94b04b27825d97cc51604153bd5`).
The native census is exactly 251 current controls: 246 retained Core controls,
two additional source-state controls and three current Kagami finality-inspector
controls. The retired historical exporter is recorded separately, without a
claim of equivalent replacement.

The focused periodic-capture regression passes in 5.22 seconds. Current grouped
results are: source-state 15/15, source-map 12/12, inventory 99/99, native routing
3/3, output producer 90/90, top-up 7/7, alias registry 5/5, original owner 1/1,
sealed route 1/1, policy source 13/13, policy route 1/2 and Kagami finality 3/3.
No selected control was ignored, and source/binary hashes remained unchanged.
The failed queue policy fixture uses `physical-elastic`, while existing
production validation requires `elastic-lane-{id}` before checking activity.
The one-literal `elastic-lane-1` correction is prepared in an ignored patch and
held for the active Apple source-seal window; the production resolver and all
height/overflow assertions remain unchanged.

The same retained binaries additionally pass the private-note/PQ-MASP atomic
rejection regression affected by the accessor repair, and both authentic Kagami
prepared/final-signed one/four-lane chain controls. Native execution-evidence
controls pass 6/7: removing the exact height-2 original context fails closed,
but the test expects a retired diagnostic string. Current production forwards
the original archive I/O error. A test-only correction is prepared to require
`ErrorKind::NotFound` from that exact record and equality with the live reader's
propagated error. The restoration and carrier-substitution assertions remain
intact; this prepared correction has not yet been applied or validated.

The canonical Apple builder's completed host release slice is reused through
`../sdk-qualification/current-host-jni/receipt.json` in the ignored evidence
directory. Its `privacy-production-enabled` library SHA-256 is
`5135d02bb3245e1815b8a7ae0a08d1ea05020abaf92e8c220d8611e3d19cf6c2`.
A fresh canonical source-seal verification against Apple snapshot
`1691761ec056e2adb23b5696af1fafb8c5591c8f600f37999c708c546f47c1d4` and a
native ABI-version probe both pass (ABI 25). The separate bridge test build uses
the same feature selection. It reaches an existing stale test call in
`qualified_enrollment_delegate/tests.rs`: `accept_challenge` now requires a
fourth qualification argument. That test already creates a verified qualification
immediately before the call. The prepared correction supplies `Some(&verified)`;
using `None` would incorrectly reject before the intended substituted-context
check and violate its retained provider-call-count assertion. This third
fixture-only correction is held for the coordinated source window. The
shipping host library is unaffected by this `cfg(test)` compile failure. SDK consumer and complete Apple package outcomes
are owned by their SDK validation record, not inferred from this host slice.

## Native FASTPQ controls

The current default-feature `fastpq_prover` test binary, retained under
`primary-core-kagami-retry7/fastpq`, has SHA-256
`62fdc5e369ffb25d817fa6929eb8f0da94a8b36e28ebcb75aba8f59542a70531`.
`cargo test --locked --offline -p fastpq_prover --lib --message-format=json-render-diagnostics backend::deep_ -- --test-threads=1`
passes 114 native controls with zero failures in 76.08 seconds of test execution
(313.91 seconds including compilation). The exact binary census records the two
unchanged ignored cases: the complete masked producer roundtrip and the captured
seeded-child diagnostic. These ignored cases are not counted as passes.

The runner deliberately stopped at the next selection boundary so the SDK owner
could repair one Android test. `primary-native-continuation1` captures exactly
that Kotlin test-only amendment; every Cargo input is unchanged. Its complete
source manifest SHA-256 is
`61f9037ba971f8a694b8649489a5f4f4349f7e18ffa9102df79bd1a8f076ba38`.
Normal native `fastpq_prover` field controls pass 15/15, and `fastpq_isi`
`poseidon_digest384` controls pass 51 with one existing ignored case. Exact
active/ignored names and binary hashes are retained with each selection.
Normal `iroha_core_zk` unit compilation also passes. All four private-journal
controls and all nine outgoing-operation-index controls pass, covering the
original prefix/custody/terminal-record helpers whose erroneous gates were
removed. The normal Rust `confidential_redemption` example passes in 57.07
seconds including compilation: it generates and locally verifies a
`ConfidentialFullUnshield` proof of 13,741 bytes from one real input note, with
no transaction submission.

The current `fastpq-gpu` native binary is retained with SHA-256
`60fba1a3cde8bc3300bdffa93b0e8bc3612f3cbc211d1aa4746fd2b342fcbd84`.
All 47 selected ordinary accelerator controls pass: secret storage 4, digest
executor 6, Digest384 GPU preflight 10, Digest384 batching 16, Metal ticket
lifetime 5 and Metal Digest384 custody 6. The actual native census corrected the
runner's source-file selector to `metal::ticket_lifetime`; no empty selection
was counted and the already passing groups were not repeated. Required physical
Metal parity is scheduled separately behind the GPU-use coordination interlock.
These results do not close the still-failing fixture controls or establish
independent cryptographic, device or finalized-network qualification.

## Outstanding validation

The three prepared fixture corrections and their complete current native reruns,
bridge unit regressions, required physical Metal parity and four-validator
native execution controls, and workspace checks remain pending. The complete goal must remain open
until these runs and the separately tracked proof/security/device gates have
their required evidence.

## Historical candidate isolation

Before the primary-checkout-only instruction, a frozen candidate at
`89efeb5f73` compiled and ran a subset of controls. That runner was prevented
from scheduling follow-on work using an output interlock after its in-flight
inventory test ended; no Cargo, rustc or agent process was killed or signalled.
Its 15 source-state and 12 source-map passes and inventory result (98 passes,
one fee-fixture failure) are historical only. They do not validate this primary
candidate. No further clone/worktree validation is authorized or scheduled.
