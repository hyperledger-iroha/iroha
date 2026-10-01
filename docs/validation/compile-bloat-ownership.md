# Compile-time ownership and validation

The current [optimization goals](../../specs/compile_bloat_optimization_goals.md)
separate compilation owners while preserving deterministic execution, proof
validation, wire declarations and runtime performance. First-release consumers
use their actual owners directly.

| Boundary | Current design |
| --- | --- |
| VM and compiler | IVM and artifact admission use the compiler-independent ABI/surface. `kotodama_lang` owns compilation; `kotodama_toolchain` owns compiler tools. Core uses the compiler only for tests. Torii's contract-source API remains a compiler consumer, so compiler edits can still rebuild the daemon graph. |
| Privacy verification | `iroha_core_privacy` owns state-free engines, profiles, proof records and verification. Core retains committed-state admission and authenticated authority construction. Fixtures and negative source controls follow the moved implementation. |
| Timed OVN | `iroha_core_timed_ovn` owns public evidence, archive/casting data and TLE verification. Core retains state reads, authenticated constructors, opaque authorizations and signing. Public data construction does not grant authority. |
| Executable metadata | Thin `irohad` and `iroha_cli` packages provide compiled metadata to `irohad_lib` and `iroha_cli_lib`, whose extern crate names remain `irohad` and `iroha_cli`. Build scripts belong to executables. Version, source and wire identity diagnostics use the injected metadata. The CLI library always includes Core/node, crypto/consensus and Norito/node-codec; `cli` and `dev-tools` select binary targets. |
| Test parsing | P2P network, Kotodama compiler, model block and model proof each have one out-of-line `#[cfg(test)]` module. Production parsing skips their bodies. Test names, fixture bytes and ownership remain covered by logical-source readers and negative controls. |
| Codec layouts | Norito accepts fixed-width and compact-length layouts. Retired packed-layout implementations and the last unused flag-name constant are removed. Header/layout and malformed-frame rejection controls remain required. |

Core's execution-pool getter remains `pub(crate)`. Its unit regression checks
that cloned handles preserve the original pool and reservations. External lane
fixtures use the Sumeragi test-chain accessor; no additional State testing API
is retained. Normal Core uses a crate-local ZK owner alias. One explicitly
non-shipping adapter, gated by `iroha-core-tests`, still serves the protected
`integration_tests/tests/sumeragi_npos_committee_transition.rs` consumer. Its
source TODO records the remaining direct-owner migration.

Scoped continuation runs cover the IVM/surface/toolchain, timed-OVN, P2P,
moved compiler/model and all six Norito grouped harnesses. A fresh privacy-owner
harness passes 39 targeted repair and production controls; its registry preserves
all 2,075 original test names and their 40 ignored cases. Core authority and
bridge controls retain their recorded passes and source bindings.

Recorded normal native/JS/Python and ordinary daemon/CLI frontend checks pass.
The native consumers use the state-free owners without Core/P2P in their normal
graphs. The target inventory admits 98 declared binaries and 23 defaults;
certificate and raw-attestation encoders require explicit `dev-tools`. Recorded
IVM-only, feature-hygiene and dependency-boundary guards pass, as does the
retired-codec pattern check. The merged all-targets check stopped when a new
ordinary Guard composition source was unavailable during compilation. Subsequent
Core ZK and metadata checks exposed issuer-type and API/fixture compilation
errors. Source repairs are applied; fresh compiler and focused-test validation
remain pending. The resolved SDK merge remains externally owned. A merge-free
workspace retry, executable metadata freshness and observational warm timings
remain pending. Foreign source and HEAD changes qualify each result; workspace lint
and panic-inventory closure remain separate.
Exact commands, exit codes and logs belong in PR Testing or CI artifacts.

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
