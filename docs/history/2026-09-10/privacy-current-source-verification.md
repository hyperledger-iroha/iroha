# Privacy current-source verification — 2026-09-10

These local checks used the resolved, uncommitted merge at HEAD
`641474bdeb14ebd573df0965d162397ad0228959`, with merge parent
`b6e815735015726ad7cafc050fee2b4d485f981e`. The staged tree remained
`04d5d289857424b008297cc6902768547cd268b6`. Each run captures its own complete
source input hashes and executable identity. These runs do not qualify a release,
hardware matrix, independent cryptographic audit or four-validator deployment.

## FASTPQ regressions

Two test corrections match the final implementation: the bundle policy accounts
for two 375-query segments, and a sole-leaf Merkle multiproof has an empty sibling
frontier. The latter rejects redundant siblings and an incorrect role with exact
errors. The merged codec already used explicit allocation policies for the
diagnostic full-geometry proof; no production limit was enlarged by these fixes.

The normal Cargo test build passed without compiler diagnostics:

```sh
cargo test --locked --offline -j 3 -p fastpq_prover --lib --no-run --message-format=json-render-diagnostics
```

The two corrected tests, full-geometry proof regression and complete shared
opening codec selection passed: **18 passed, zero failed or ignored**, in
855.14 harness seconds. All 19,880 captured tracked inputs and the executable
were unchanged during the run. The executable SHA-256 is
`d27687013adaae3458b2588fc82dd46c3d3fb864c38f85e3ebefc06bc8308cc6`.

The valid full-geometry shared frame is 3,994,624 bytes and charges 36,662,377
cumulative Norito allocation bytes. Its explicit diagnostic allocation limit is
64 MiB; the 32 MiB baseline still rejects it. The malformed loose-shape control
charges 54,812,414 bytes. Verification accepts the valid proof and rejects the
false terminal polynomial. The production 512 KiB proof ceiling remains unmet.

## State construction budget

The integrated budget measures complete source-entry bundles against all six
construction caps before private-tree work. It binds the finalized immutable
State inventory, preserves committed usage on failure or abandoned preparation,
and publishes usage only with successful materialization. Retrying replaces the
whole inventory usage instead of adding it twice. It does not grant execution
admission, authenticated runtime quotas or source finality.

The normal Core test build passed without compiler diagnostics:

```sh
cargo test --locked --offline -j 3 -p iroha_core --lib --no-run --message-format=json-render-diagnostics
```

The `state::fastpq_source_inventory::`, `fastpq::source_capture::` and
`fastpq::source_reservation::` selections passed: **140 passed, zero failed or
ignored**, including all ten new reservation tests, in 77.96 harness seconds.
All 19,882 captured inputs, including both newly added Rust files, and the
executable were unchanged. The executable SHA-256 is
`11707ce2e34b6bf6709d45cf67b4cf0001d494b703c7c2c3edb5b96c3935e37c`.

## Evidence and open work

Receipts live under ignored
`target/privacy-release-evidence/2026-09-07-recovery/`:

- `native25-current-prover-repairs/result.json` binds the 18-test run, its Cargo
  artifact graph, exact selectors, output hashes and source captures.
- `native26-current-state-reservation/result.json` binds the 140-test run and
  the separate integrated State source capture.
- `native25-current-masked-air-rebase-independent-review/review-capsule.json`
  records independent arithmetic and degree checks of a private candidate.
  Its strict non-test library build fails because the extension evaluators have
  no production caller. Passing test metadata is not a passing library build.

Private scalar PCS identity and constrained-state arithmetic checks are not
shipping proof verification. Full typed ordinary/AXT caller frames, final scalar
descriptor assignment and domain cutover, authenticated Core expectations,
initial transcript ownership, packed proofs, privacy/soundness arguments,
resources and external qualification remain outstanding. No private candidate
or diagnostic proof is registered as production-qualified.

## Coordinated consumer build and regressions

The subsequent default-feature graph compiled `fastpq_prover`, `iroha_data_model`,
`iroha_core`, `iroha_torii`, `irohad`, `iroha_cli`, `connect_norito_bridge`,
`sorafs_node` and `sorafs_orchestrator`, with library targets, the `iroha` binary
unit target and `cli_smoke`. The successful retry took 497.77 seconds and emitted
ten test executables. Its 16 actual compiler-message records are warnings
(CLI two, Torii two, daemon twelve); this is not a strict-lint pass. Both earlier
compiler failures remain retained. All 19,932 captured repository entries and
the raw index were unchanged during the successful build and its test phases.

The amended 97-test consumer contract and additional required names registered
in their actual executables. The root's 45 invocations then ran **101 tests:
97 passed, four failed, zero ignored**. The passes include:

- All 50 public-transfer tests, including the new allocation-free chronological
  pair-boundary root iterator and repeated-root occurrences.
- Both BFV full-bootstrap preflights. The valid material reaches the existing
  closed production-qualification gate; malformed key context rejects earlier.
  These are not successful production BFV bootstrap or engine activation.
- All eight compliance-feed transport tests and the cumulative-deadline test.
  Four new controls enforce the Zstandard history-window ceiling before reading,
  including an oversized window with tiny output, sub-1-KiB decoded limits and
  later concatenated frames. The separate decoded-byte limit remains enforced.

The four failures were two Core persisted-projection fixtures using approval
epoch one with the default retention epoch zero, a broker nonce fixture changing
its payload without refreshing the enclosing request digest, and a governance
ballot fixture expecting a conversion error where the current authority policy
returns `ValidationFail::NotPermitted`. Corrections now provide an explicit test
retention policy and its equality-boundary regression, rebuild the mutated
broker request through its canonical constructor, and assert the exact typed
ballot errors. The broker test preserves both the stale-envelope `Protocol`
rejection and the canonical zero-nonce `Rejected` outcome. These corrections and
the retained failures still need a fresh compiled rerun.

Separately, the Node focused run executed **99 passed, 14 failed, zero ignored**.
The fourteen cleanup tests fail before their intended assertions because two
WebAuthn fixture strings lack the mock authenticator's required valid prefix.
The corrected literals and the exact typed ballot-policy assertions are applied
but have not yet passed a fresh build/test run. Production authentication,
network policy and canonical-frame checks were not weakened.

The orchestrator focused run passed all 41 selected tests. Its complete native
run reports 223 passed, zero failed and two ignored. The evidence driver's
outcome parser rejected interleaved test/log output. A separate independently
checked classification reconstructed exactly six split serial test records from
the pinned raw log, with no missing or extra outcomes; no test rerun was used.
The original failed parser receipt remains unchanged. The two ignored tests
remain unexecuted. That historical classification is in
`current-native27-orchestrator-full-classification/classification.json`, SHA-256
`afe26bea7206ebc01dc94d81e5d91b55e66b4920af15b43a62fb5e27d1d8bc10`.

A separate same-source FASTPQ run passed **24 exact tests**, zero failed or
ignored: 20 scalar/canonical-prefix/parallel hash and arithmetic controls, and
four empty-batch CPU proof/explicit-GPU-unavailable controls. Each selected one
test from the 984-test inventory, with 983 filtered. The executable hash is
`1310fc55bb03e92539e4f99b8650ad9b80bbdee1025fa6c61ea014f78de02432`.
The graph has no `fastpq-gpu` feature. These checks execute no Metal or CUDA
kernel, and a mode-named test using an unchanged CPU implementation is excluded
from device evidence. Complete GPU proving remains unavailable.

Additional ignored evidence records bind these exact scopes:

- `native27-coordinated-build-retry2/result.json`: successful build with warnings.
- `native27-current-privacy-sorafs-validation/root-required-retry2/result.json`:
  root 97-pass/four-failure execution, SHA-256
  `31adf8e54f98fefe361b24730acc9217d3f5d746875b9ac6a76c906315262a63`.
- `native27-m1-six-lane-parity-run-private/run-capsule.json`: the 24 CPU checks,
  SHA-256 `c396702bdf6e3ee688ee95fe5460a475eedfa85d06e77d5e2d599bdfd6fb5b94`.
- Under `target/evidence/sorafs-v1/`, the `current-native27-*` Node and
  orchestrator run directories retain their distinct failures and results.

The Kaigi SDK getter corrections are included in this source snapshot, but this
build contains neither its integration harness nor an optimized daemon binary.
The corrected four-validator lifecycle requires fresh compilation and execution;
the previous height-one-only network failure does not satisfy that gate.

The follow-up compiler cleanup applies platform guards matching existing
Linux-only callers, keeps the shared cross-platform `Seek` import and bounded
log-test helpers, and elides the single-use worker lifetime. It changes no
function bodies or production configuration and suppresses no warnings. Fresh
compilation is still pending. Three inspected merged owners already exceed
source-file budgets: CLI reset host 23,986/5,000 lines, CLI Soracloud
38,146/23,091 and daemon Soracloud 36,305/27,084 before this cleanup. The added
guards do not repair those inherited structural violations; no limits are
raised and no source-budget or Linux-runtime qualification is claimed.

The complete source-budget check also fails: 228 findings across 11295 checked files. Its unchanged configured limits and complete report are retained in `native28-source-amendments/source-budget.json`. These broader merged-tree findings are not repaired or suppressed by this scoped patch. `cargo fmt --all`, `git diff --check`, the retired-codec guard and historical archive verification pass.

## Native28 failure and subsequent source revision

The six-package test build with the Kaigi evidence feature failed after 672.91
seconds on Rust 1.93.1. Its complete source and index captures were unchanged.
The compiler emitted two errors and six warnings: the suggested anonymous
lifetime inside `impl Trait` is unstable (E0658), with a consequent inference
error (E0282). The compiler's earlier `MachineApplicable` suggestion was invalid;
this was not a successful warning cleanup. The new feature also exposed five
distinct unused Core evidence declarations. Raw diagnostics and the failed
classification remain in `native28-coordinated-test-build/`.

After that build finished, separate merges advanced the clean checkout to
`73e64d276d3c17e7535d72373e40c789ebcde3db`. The Native29 baseline captures 20,003
source entries: 810 existing entries differ from Native28 and 81 are new.
`native29-baseline-reconciliation/` retains the exact comparison. Earlier native
results do not qualify this new source revision. A fresh test graph and release
daemon are required before executing the eight Kaigi preflights and actual
four-validator lifecycle. The local-release profile is not used for that
deployment evidence.

The current-source correction expresses the scoped worker lifetime through a
named iterator bound, preserving worker settlement and removing the lint
suppression. Exact Rust 1.93.1 metadata probes reproduce the faulty suggestion
and compile the corrected signature with warnings denied. The unused Bootle
payload accessor is removed; one DER accessor and three descriptor/bound
expectations retain their existing test-only callers. Production relation and
readiness checks remain present. Their native regressions await the new build.

Separately, the private conditional scalar PCS composition passed all 109 tests
in both its author run and an independent fresh compilation/replay. It owns the
first six complete transcript moves, all 923 independent AIR coefficients, the
685 canonical claims, and the checked trace-to-quotient handoff. Root verified
all 364 sealed artifacts. The independent review capsule is
`native28-scalar-first-six-independent-review/review-capsule.json`, SHA-256
`c7c7b0b2e6994110dd82b037246ff3912db45f0d90cf53d2dd5befb1cd2ad729`.
Its direct-rustc facade uses pinned older owner libraries; this is not a current
Cargo graph or a coherent complete proof with every query derived from the
transcript. Authenticated caller/AIR binding, final descriptors, production
reachability, registration and security qualification remain open.

## Native29 current graph

The fresh six-package test graph failed after 963.67 seconds with 17 warnings
and one error. All 20,003 captured source entries and the index remained
unchanged. The earlier warning cleanup and scoped-worker signature compiled;
the remaining warnings belong to merged consensus lifecycle code and
an unused Kaigi harness import. The integration error is the harness's attempt
to frame the payload-only `KaigiRecord` directly. Its diagnostic must hash the
existing canonical `Json` metadata frame instead. No complete build, regression
run or network pass is claimed from the three emitted partial test artifacts.
The full failure is retained in `native29-current-source-test-build/`.

At the subsequent source boundary, the existing DER unit module was extracted
to a same-directory test file. The runtime owner now has 4,124 lines, within its
unchanged 5,322-line cap; the test file has 1,179 lines. All 17 direct tests and
both included test files keep their original module paths and code. Independent
reconstruction and formatting reproduce the previous inline source byte for
byte. Native execution of the extracted module is still pending.

During compilation, 19 inactive incremental compiler caches older than 24 hours
were removed after process and open-file checks showed no incremental users.
Free disk space increased from 5,597,134,848 to 110,934,556,672 bytes. Source,
index, dependency artifacts, executables and retained evidence were not removed;
the active test build disables incremental compilation. The scoped cleanup
receipt is `native29-inactive-cache-cleanup/result.json`.

Before the retry, the Kaigi diagnostic was corrected to frame its existing
`Json` metadata representation. The scoped consensus cleanup narrows two
ledger-record methods to their actual module owner and removes unused query
wrappers and comparison methods. The original Decision WAL seal, exact binding
checks and consuming Apply publication remain. Four test callsites now consume
the same output-settlement yield predicate as the production runner, retaining
their original assertions and turn bounds. Those fixtures require fresh native
execution because a newly completed output consumes another bounded turn.
The older formal direct-serve source checks still expect obsolete callsites;
this cleanup does not alter those checks or claim formal validation.

Further cross-owner review identified a mismatch in the private scalar trace
functional: its derived order-65,536 generator differs from the actual AIR
catalog's row generator. The previous 109-test private results did not expose
that mismatch and do not establish the actual AIR relation. A correction and
nonconstant-witness cross-owner check are being prepared before any final
parameter identity or complete-proof qualification.

The Native29 retry1 test graph subsequently passed in 749.73 seconds with no
compiler diagnostics. All eight test harnesses and the ordinary CLI companion
were retained, and all 20,004 source entries and the index stayed unchanged.
The source capture is `abf28c1829bc45b89e1f0b9b0009949ce7fb3fb0b2463994ecff0c8688c1cdd6`;
the build receipt is `native29-current-source-test-build-retry1/result.json`.

The prepared 75-test run stopped before executing tests because its exact
namespace check found three nested finalized-provider frame tests omitted from
the source inventory. The corrected inventory preserves every original leaf
and includes those three. Its actual 78-test run finished with 67 passes and
11 failures. All 48 finalized-provider query tests passed. Ten lifecycle tests
stopped at the existing signer/peer equality assertion: their live fixture
installed roster index zero while its owner signed as the selected leader or
Set-A validator. The other failure tried to construct a 16,383-byte `Name`,
exceeding its actual 255-byte limit, before checking Soracloud response lengths.
The stopped inventory and every test outcome remain in
`native29-retry1-corrected-regression-run-01/` and `-run-02/`.

At the next source boundary, the existing lifecycle fixture helper now takes
the explicit local validator. All ten call sites migrate together; the two
role-selected callers pass their actual index and the other eight retain zero.
The signer assertion, moved body-store identity, capacity and replay assertions
remain intact. The Soracloud fixture uses a maximal valid `Name` and places the
16,383-byte boundary in its existing String field, preserving both exact size
assertions and the 16,384-byte state key and payload. These corrections require
a new native build and rerun before the later replay paths can be qualified.
The obsolete DER 5,322-line exception is removed: its 4,124-line runtime owner
now falls under the normal 5,000-line limit. Other source-budget findings remain.
Five existing direct Sign/WAL tests are also extracted at their original include
position, reducing their test parent from 3,221 to 2,616 lines with a 608-line
child. Byte-exact reconstruction and independent recursive source assembly
preserve their bodies, attributes and module paths. The source manifest gains
only that child and its digest is updated. Shared index contents are preserved;
the indexed-source check remains distinct from the pending native rerun.

SoraFS Node on the retry1 graph passed all 129 focused tests. Its full 1,545-test
inventory then finished with 1,543 passes, no failures and two ignored tests in
581.22 seconds. Both ignored cases require an actual local Kubo runtime for IPNS
or signed-head publication, restart and tamper checks; those external lanes are
still unqualified. Full source and index captures remained unchanged. Receipts
are under `target/evidence/sorafs-v1/current-native29-retry1-node-focused-01/`
and `current-native29-retry1-node-full-01/`; the full result SHA-256 is
`261b47219b59a13343a694e746a847356ae1911b35573f9f6f4f580aa1331e9c`.

The isolated scalar AIR-root correction passed all 112 private tests, including
three new checks against the actual catalog generator and nonconstant witness.
Recompiling the original expression causes those three checks to fail as
expected. Both compilations emitted no diagnostics and retained all pinned
inputs. This uses an explicitly historical dependency closure and a test facade,
not the current Cargo graph or a complete production proof. Its runtime seal is
`native29-scalar-air-root-alignment-validation-private-retry1/execution/runtime-seal.json`,
SHA-256 `20165dcc20a0e4290bd6ec150b3f4f08817362607511333c553d9d9238eae4b6`.

## Retry2 lifecycle, response framing and local Kubo results

At HEAD `73e64d276d3c17e7535d72373e40c789ebcde3db`, the next six-package
Cargo test build passed in 360 seconds with no compiler diagnostics. All eight
test harnesses and the ordinary CLI companion were retained. The 20,005-entry
repository capture is
`575818e752ce38610106fe667fecb998c6b1c0e1991fb7db8dc6b90fc7063f3f`;
this bounded file/symlink inventory does not capture a complete compiled
source closure. Source entries and index remained unchanged during the build.
The build receipt is `native29-current-source-test-build-retry2/result.json`,
SHA-256 `f169c9b3874b2ea2572e68509d0e57ba0675c0455e088305c455c5b15b1e35bd`.

The complete corrected selection ran 112 tests: **100 passed and 12 failed**.
It preserves the prior 78 and adds all 26 affected fixture leaves, five moved
Sign/WAL tests and three response-bound tests. All 48 finalized-provider query
tests, five Torii tests and the CLI test pass. The nine Core failures concern
released Apply ownership/publication, historical rejection reports, cold replay,
view progression and a crash fixture's invalid certificate. The three daemon
failures compare actual response frames with payload-only size quotes: the
quotes omit the 40-byte Norito header and 8-byte response alignment padding.
The exact failures and source/index/artifact checks remain in
`native29-retry2-corrected-regression-run-01/result.json`, SHA-256
`22565c250076c476d2ea5ed744764314b57851a4bf34c3ebf24cf90a117c3d4f`.
Source corrections and test diagnostics require a subsequent compiled graph;
these failures have not been promoted to passes.

Both previously ignored local Kubo tests now pass against the retry2 Cargo
artifact, using the pinned Kubo 0.42.0 executable and fresh owner-only temporary
repositories. The IPNS lane takes 22.74 seconds; the signed-head lane takes
43.46 seconds. Each runs exactly one test with zero ignored outcomes. Observed
child PIDs are absent and both empty temporary roots are removed. The runtime
receipt is `target/evidence/sorafs-v1/current-native29-retry2-kubo-lanes-01/result.json`,
SHA-256 `891f3f616913dd92d9715941ed714942ab2dbf08f7171a2338656b958762e5b4`.
This verifies local publication, restart and tamper handling only. It does not
establish global process cleanup, hardware custody, regional independence,
replicated signed RS16 DA/RBC or four-validator deployment qualification.
