# Compile-time and code-bloat optimization goals

Separate compilation owners while preserving features, wire declarations,
deterministic execution and runtime performance. Migrate consumers directly: first release does not
require public compatibility aliases. Sumeragi remains owned by the user's
separate work; inspect current SCCP state before touching adjacent code.

| Goal | Status | Completion criteria |
| --- | --- | --- |
| O1: Finish compiler and Core ZK ownership | In progress | IVM's normal graph excludes the compiler; compiler/toolchain/VM and proof tests pass; source guards and external imports use their actual owners. |
| O2: Extract state-free privacy verification | In progress | Separate privacy crate, original proof/wire fixtures and all tests preserved; Core retains state authority; native/Python/JS consumers drop the validator execution graph. |
| O3: Extract state-free timed-OVN verification | Complete | Public evidence, casting archive and TLE verification have a separate owner; authenticated constructors, state reads and signers stay in Core; bridge tests preserve authorization and replay checks. |
| O4: Isolate executable build metadata | In progress | Thin daemon/CLI packages supply compiled identity to libraries whose builds are free of executable build metadata; executable names/features remain coherent; metadata-only changes leave libraries fresh. |
| O5: Remove production parsing of large test bodies | Complete | Four cohesive P2P/compiler/model test modules moved without dropping tests or fixtures; source guards and each preserved runtime harness pass, including the transparent model assertions. |
| O6: Complete codec and merged-source validation | In progress | Norito/IVM-only, feature, dependency and target guards, focused tests, merged workspace check and applicable lint/test gates pass; actual exit codes and unresolved external failures are recorded. |
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
Twelve rewritten private-terminal controls map to original field-mutation and
forgery coverage. Recorded normal
native/JS/Python and ordinary daemon/CLI frontend checks pass; the reviewed binary
inventory now admits 105 declared targets and 23 defaults, including the installed-context
developer tool. Recorded IVM-only,
feature-hygiene, dependency-boundary and retired-codec pattern guards pass.
The normal production-feature Core ZK frontend and both test-feature harnesses
have recorded builds, limited by concurrent source changes. Component journal
tests pass. Focused issuer-key, corrected enrollment-floor and expired-preparation
custody controls pass in the retained default-feature harness. The Guard-generation
fixes compile and both extended regressions pass. MintFold and protocol-label
controls and every remaining selected proof case pass in the retained harness: 33
selected controls in total, with source changes limiting current qualification. The scoped non-test real-proof-harness frontend check passes with all 22 focused
source pins unchanged. Broader source and merged-candidate qualification remain
incomplete. Unused
terminal reference helpers now compile only for tests, preserving their assertions
and production private-product/local-AIR path. Primary-owner Privacy library
strict lint passes with defaults, `privacy-release-evidence` and
`privacy-release-evidence,test-utils` in an unchanged source/Git interval.
The current joint MAIN/CA and proof-instance candidate has a fresh debug-profile
Privacy libtest build. Its registry contains 2,479 tests, including 61 ignored
cases. All 22 selected controls pass; independent framing of all 29 native profile
fields matches the current source pin. The retained artifact has 475 source,
literal, manifest and build-control inputs, including 417 actual dep-info inputs,
unchanged across the build and selected runtime intervals. Current descriptors
include 192 endpoint, 17 key/digest, 20 SHA-union and 108 CA-link alpha phases and
the 39-relation RFC inventory. Genuine IO, Projection and CA proof checks, the
optimized constructor, full ordinary suite, upstream strict lint and merged-source
qualification remain open. Maximum-proof external time/RSS evidence and enforced
address-space limits remain separate cryptographic release gates.
Eight focused parameter tests pass with the scoped inline-policy
annotation.
Concurrent policy edits limit current-source qualification. Both metadata builds
compile with matching package/features; the freshness check fails with concurrent
source changes and library rebuilds. The last recorded workspace check reaches
Core test compile errors owned by the separate repair chat. That chat also owns
concurrent merge completion and the remaining protocol integration repairs.
Final validation still requires a stable merged source interval.
SDK Native custody and genuine production proving are mandatory even with SDK
defaults disabled; assembly tools remain explicit `dev-tools` targets. FASTPQ
belongs to existing STARK activation. The retained Native graph observation fell
from 418 to 403 packages and does not establish a current frontend result or
compile speedup. Exact CoreZK/Halo2 owner contracts cover default/TLS SDK selections
and the fixed Musubi, SCCP wallet and storage-client consumers, retaining their
runtime, P2P, compiler and test-feature denials. Feature hygiene, the ownership-guard
suite and all 21 configured dependency boundaries pass. Current source budgets
also pass with the updated developer-tool manifest fingerprint. Unchanged exact
ceilings include mandatory custody and proving costs; current native frontend qualification
remains open. Workspace validation, metadata freshness qualification
and observational warm timings remain pending. The
[current ownership and validation note](../docs/validation/compile-bloat-ownership.md)
describes the remaining controls. All evidence retains its scoped source and
load qualifications; full completion is not yet established.

Compiler-memory qualification remains the pinned-runner work described by
roadmap A5. Measured limits must also cover newly extracted owner/library units;
the empty `introduced_units` arrays do not admit them automatically. The last
retired Norito flag-name constant is removed after confirming its protected
consumer no longer exists. One Core ZK adapter is explicitly non-shipping,
gated by `iroha-core-tests`, while its user-owned Sumeragi test still awaits a
direct-owner import. Normal Core uses a crate-local owner alias.
