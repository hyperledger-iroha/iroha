# Compile-time and code-bloat optimization goals

The optimization preserves the current compilation and ownership boundaries. Preserve features, wire declarations, deterministic execution
and runtime performance. Migrate consumers directly: first release does not
require public compatibility aliases. Sumeragi remains owned by the user's
separate work; inspect current SCCP state before touching adjacent code.

| Goal | Status | Completion evidence |
| --- | --- | --- |
| O1: Finish compiler and Core ZK ownership | In progress | IVM's normal graph excludes the compiler; compiler/toolchain/VM and proof tests pass; source guards and external imports use their actual owners. |
| O2: Extract state-free privacy verification | In progress | Separate privacy crate, original proof/wire fixtures and all tests preserved; Core retains state authority; native/Python/JS consumers drop the validator execution graph. |
| O3: Extract state-free timed-OVN verification | In progress | Public evidence, casting archive and TLE verification have a separate owner; authenticated constructors, state reads and signers stay in Core; bridge tests preserve authorization and replay checks. |
| O4: Isolate executable build metadata | In progress | Thin daemon/CLI packages supply compiled identity to build-script-free libraries; executable names/features remain coherent; metadata-only changes leave libraries fresh. |
| O5: Remove production parsing of large test bodies | In progress | Four cohesive P2P/compiler/model test modules moved without dropping tests or fixtures; affected source guards pass; moved Rust harness validation remains in progress. |
| O6: Complete codec and merged-source validation | In progress | Norito/IVM-only, feature, dependency and target guards pass; focused tests, merged workspace check and applicable lint/test gates remain; actual exit codes and unresolved external failures are recorded. |
| O7: Measure improvements | In progress | Warm same-package check timings compared with the September 27 baseline; source/toolchain and competing build load recorded. Optional further extraction/router work requires measured benefit. |

The recovered baseline checked `irohad`, `iroha_cli` and `iroha_kagami`
after changing the data-model root, without incremental compilation: 526
seconds overall; Torii 187.0 seconds, data model 126.1 seconds and Core
119.5 seconds. Concurrent changes and builds must be distinguished from this
optimization's measured effect. New compiler-memory limits require actual
pinned-runner measurements; source moves alone do not qualify them.

Scoped component evidence covers: 1,014 IVM, 32 compiler-surface and 51
toolchain library tests; nine timed-OVN owner tests; 286 formal/feature Python
tests; 48 dependency-budget tests; Cargo feature hygiene, workspace target
inventory and every configured feature-resolved dependency boundary. Separate
source/fixture suites retain their own logs and counts. The normal graphs
exclude the compiler from IVM/Core and full Core/P2P from native/JS/Python.
The [current ownership and validation note](../docs/validation/compile-bloat-ownership.md)
describes these boundaries and remaining controls. These are scoped
development checks during concurrent source changes; full completion is not
yet established.

Compiler-memory qualification remains the pinned-runner work described by
roadmap A5. Measured limits must also cover newly extracted owner/library units;
the empty `introduced_units` arrays do not admit them automatically. The last
retired Norito flag-name constant is removed after confirming its protected
consumer no longer exists. One Core ZK adapter is explicitly non-shipping,
gated by `iroha-core-tests`, while its user-owned Sumeragi test still awaits a
direct-owner import. Normal Core uses a crate-local owner alias.
