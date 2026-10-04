# Compile-time ownership and validation

The current [optimization goals](../../specs/compile_bloat_optimization_goals.md)
separate compilation owners while preserving deterministic execution, proof
validation, wire declarations and runtime performance. First-release consumers
use their actual owners directly.

| Boundary | Current design |
| --- | --- |
| VM and compiler | IVM and artifact admission use the compiler-independent ABI/surface. `kotodama_lang` owns compilation; `kotodama_toolchain` owns compiler tools. Core uses the compiler only for tests. Torii's contract-source API and JavaScript's `compileKotodama` API directly consume the compiler, so compiler edits can rebuild the daemon and JS host graphs. |
| Privacy verification | `iroha_core_privacy` owns state-free engines, profiles, proof records and verification. Core retains committed-state admission and authenticated authority construction. Fixtures and negative source controls follow the moved implementation. |
| Timed OVN | `iroha_core_timed_ovn` owns public evidence, archive/casting data and TLE verification. Core retains state reads, authenticated constructors, opaque authorizations and signing. Public data construction does not grant authority. |
| Executable metadata | Thin `irohad` and `iroha_cli` packages provide compiled metadata to `irohad_lib` and `iroha_cli_lib`, whose extern crate names remain `irohad` and `iroha_cli`. Build scripts belong to executables. Version, source and wire identity diagnostics use the injected metadata. The CLI library always includes Core/node, crypto/consensus and Norito/node-codec; `cli` and `dev-tools` select binary targets. |
| Test parsing | P2P network, Kotodama compiler, model block and model proof each have one out-of-line `#[cfg(test)]` module. Production parsing skips their bodies. Test names, fixture bytes and ownership remain covered by logical-source readers and negative controls. |
| Codec layouts | Norito accepts fixed-width and compact-length layouts. Retired packed-layout implementations and the last unused flag-name constant are removed. Header/layout and malformed-frame rejection controls remain required. |

Core's execution-pool getter remains `pub(crate)`. Its unit regression checks
that cloned handles preserve the original pool and reservations. External lane
fixtures use the Sumeragi test-chain accessor; no additional State testing API
is retained. Core uses a crate-local ZK owner alias for every feature selection.
The committee-test owner migrated its protected import directly to
`iroha_core_zk`, and Core's temporary public test adapter is removed. The
development-only fee-evidence test imports `kotodama_lang::compiler::Compiler`
directly.
The signed-clock type path, Core ZK URL dependency and shared request-codec
derive are corrected. The recorded normal Core ZK library check passes with its
four default features, no diagnostics and unchanged captured source, Git,
invocation and selected managed inputs. Subsequent dependency edits require fresh
validation. The `dev-tools` fee target has a recorded build and two passing
ordinary tests. Canonical Wallet custody schemas and frame controls preserve
the original assertions; the Selection schema expectation now names its actual
owner. The expanded prospective registry contains 107 ordinary tests, including
15 direct custody controls, with no ignores. Its latest build passes, but
concurrent SDK source and generated-input changes refused registry and runtime
admission. The earlier 78-test pass qualifies only its original source cut.
The authenticated two-build executable metadata check passes on its recorded
source cut, with the four library owners fresh and all five executable targets
rebuilt; it does not qualify later source changes.

Scoped continuation runs cover the IVM/surface/toolchain, timed-OVN, P2P,
moved compiler/model and all six Norito grouped harnesses. Twelve
rewritten private-terminal controls map to the original field-mutation and forgery
coverage. Core authority and bridge controls retain their recorded passes and
source bindings.

Recorded normal native/JS/Python and ordinary daemon/CLI frontend checks pass.
The native consumers use the state-free owners without Core/P2P in their normal
graphs. Current target-inventory validation admits 107 declared binaries and 24
defaults. The canonical `ivm_artifact_admit` executable independently verifies
contracts and creates their manifests without the node CLI; its exact shipping
owner uses the existing final slot under the unchanged 24-default ceiling. The
installed-context developer tool remains non-default. Certificate, attestation,
preparation and SDK inventory assembly tools require
explicit `dev-tools`. Shipping native custody does not enable the SDK assembler. Recorded
IVM-only, feature-hygiene and dependency-boundary guards pass, as does the
retired-codec pattern check. SDK Native custody and genuine production proving are
mandatory even with SDK defaults disabled; assembly tools remain explicit
`dev-tools` targets. FASTPQ is selected by the existing STARK feature. The retained
Halo2-only Native graph observation fell from 418 to 403 packages; it does not
establish a current frontend result or build speedup. Current owner-boundary checks
admit the exact CoreZK/Halo2 profiles and SDK paths for default/TLS selections and
the fixed Musubi, SCCP wallet and storage-client consumers, retaining their runtime,
P2P, compiler and test-feature denials. Feature hygiene passes all 65 controls
and its guard command. All 21 configured dependency boundaries pass locked
offline Cargo resolution with all 120 manifest inputs and the lock unchanged.
The current source budget passes with exact reviewed normal, optional and
test-owner declaration costs, preserving ownership denials without growth
headroom. All 230 pure dependency-guard tests and both actual Cargo boundary
regressions pass. The compiler source guard seals 305 fixture includes and
605 test names; all 43 Python source-reader controls pass, including the
ordinary duplicate-fixture regression. Surface/toolchain workspace lint
inheritance and Surface public Rustdoc still require correction.
The normal production-feature
Core ZK frontend and both test-feature harnesses
have recorded builds, limited by concurrent source changes. Component journal
tests pass for the one first-release boxed-challenge layout, including canonical
bounds and retained challenge/scope checks. Its payload changes without a
compatibility decoder. Focused issuer-key, corrected enrollment-floor and expired-
preparation custody controls pass in the retained default-feature harness. The
Guard-generation import, layout-comparison and recovery-phase fixes compile, and
both extended regressions pass. All 33 selected proof controls, including MintFold,
protocol labels and reciprocal claim-carrier binding, pass in the retained harness;
concurrent source changes limit current-candidate qualification. The
scoped non-test real-proof-harness frontend check passes with all 22 focused
source pins unchanged. Broader source and merged-candidate qualification remain
incomplete. Unused terminal
reference helpers now compile only for tests, preserving their assertion bodies
and the production private-product/local-AIR path. Primary-owner Privacy library
strict lint passes with defaults, `privacy-release-evidence` and
`privacy-release-evidence,test-utils` in an unchanged source/Git interval.
The recorded joint MAIN/CA and proof-instance candidate has a fresh debug-profile
Privacy libtest build. Its registry contains 2,479 tests, including 61 ignored
cases. All 22 selected controls pass; independent framing of all 29 native profile
fields matches the recorded source pin. The retained artifact has 475 source,
literal, manifest and build-control inputs, including 417 actual dep-info inputs,
unchanged across the build and selected runtime intervals. Recorded descriptors
include 192 endpoint, 17 key/digest, 20 SHA-union and 108 CA-link alpha phases and
the 39-relation RFC inventory. Genuine IO, Projection and CA component proof checks
pass once each in a sequential run at the closed merged revision, with the original
475 inputs unchanged through all runtime endpoints. The CA check binds a synthetic
paired MAIN record. The optimized constructor, full ordinary suite, upstream strict
lint and merged-source qualification remain open. Maximum-proof external time/RSS evidence and enforced
address-space limits remain separate cryptographic release gates.
The latest focused Privacy validation lists 2,628 tests: 2,560 ordinary cases
and 68 ignores. It preserves all original 65 ignores, including the original 61;
the three additional ignores are reviewed cost diagnostics. Renamed scalar
tests retain ordinary coverage without retired aliases. All five corrected
GETGAS/scalar-history regressions pass, with captured source, Git, toolchain,
managed inputs and artifacts unchanged through their build and runtime intervals.
The earlier ordinary suite closed naturally with 2,424 passes, 52 failures and
65 ignores; its source changed during execution. All 52 failed cases remain
ordinary tests and require current-source regression validation. The current
source inventory additionally contains one reviewed shared-public-FFT cost
diagnostic, bringing prospective ignores to 69. Neither a source inventory nor
a focused subset qualifies the full current ordinary suite or final candidate.
The derive library, strict JSON and UI regressions pass, preserving diagnostics.
Eight focused parameter tests pass with the scoped inline-policy annotation,
resolving the observed enum-size compilation frontier in that harness.
Concurrent policy edits limit current-source qualification. A later workspace
check passes on its recorded source cut. The last strict workspace result
identified 135 Core ZK diagnostics and two sample-owner diagnostics; the reviewed
repairs require fresh strict validation. The current merge has no unmerged
entries and remains uncommitted; its separate owner handles the merge. Current
workspace checks, applicable strict lint, Privacy regressions and observational
warm timings remain open. Each earlier component pass retains its own source
and feature scope; ongoing SDK and proof changes require fresh input guards.
Exact commands, exit codes and logs belong in PR Testing or CI artifacts.

Two additional numeric controls reconstruct canonical frame, CRC and SHA inputs
in both Pasta fields. They exercise a test-only reference after the current
integrity-stream integration and pass in the retained default-feature harness.
They do not qualify the live lease or proving relation.

Final moved-model controls must enable `transparent_api`, because several
preserved block assertions are feature-gated. Reuse the warm `finish` lane
sequentially:

```sh
scripts/cargo_fast.sh --target-slot finish -- test --locked --offline -p iroha_data_model --features transparent_api --lib block::tests
scripts/cargo_fast.sh --target-slot finish -- test --locked --offline -p iroha_data_model --features transparent_api --lib proof::tests
scripts/cargo_fast.sh --target-slot finish -- test --locked --offline -p norito --lib \
  --test norito_group_01 --test norito_group_02 --test norito_group_03 \
  --test norito_group_04 --test norito_group_05 --test norito_group_06
```

Norito disables automatic integration-test discovery, so validation names all
six ordinary grouped harnesses explicitly. The grouped harnesses
cover adaptive/default flags, flag-state restoration, derive codecs, header
rejection and bare/current-payload framing. Preserve source negative controls
when validating their source readers.

Compiler-memory limits remain the pinned-runner qualification in roadmap A5.
Include newly extracted owners and libraries in that measurement. Dependency
inventory costs describe the selected source graph without speculative growth
headroom; working-tree observations do not qualify a staged subset. Compare
warm timings with the recovered baseline while recording source, toolchain and
competing build load. Comparisons across changing source or load are
observational; attribute improvement only to measurements that isolate the
optimization. Executable metadata freshness, observational warm timings and final merged
validation remain under qualification.
