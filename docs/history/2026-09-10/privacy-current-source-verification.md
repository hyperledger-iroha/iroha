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
