# Compile-time and code-bloat optimization goals

Separate compilation owners while preserving all features, canonical wire
formats, deterministic execution and runtime behavior. Consumers use the actual
owner directly: this first release has no compatibility aliases or fallback
implementations. Leave Sumeragi to its separate owner and inspect SCCP before
changing adjacent code.

| Goal | Status | Completion criteria |
| --- | --- | --- |
| O1: Finish compiler and Core ZK ownership | In progress | IVM's normal graph excludes the compiler; compiler/toolchain/VM and proof tests pass; source guards and consumers use actual owners; current default and proof-harness Core ZK checks and strict lint pass. |
| O2: Extract state-free privacy verification | In progress | Privacy owns state-free verification while Core retains state authority; original proof/wire fixtures and ordinary coverage remain; native/Python/JS graphs exclude validator execution; current repaired regressions and ordinary suite pass. |
| O3: Extract state-free timed-OVN verification | Complete | Evidence, casting archive and TLE verification have a separate owner; Core retains authenticated constructors, state reads and signers; bridge tests preserve authorization and replay checks. |
| O4: Isolate executable build metadata | Complete | Thin daemon/CLI packages supply compiled identity to metadata-free libraries; executable names/features remain coherent; the authenticated two-build metadata check preserves library freshness on its recorded source cut. |
| O5: Remove production parsing of large test bodies | Complete | Four cohesive P2P/compiler/model test modules moved without dropping tests or fixtures; source guards and preserved runtime harnesses pass. |
| O6: Complete current-source validation | In progress | Norito/IVM-only, feature, dependency and target guards, focused tests and applicable workspace lint/test gates pass on the current candidate; unresolved external failures are recorded. |
| O7: Measure improvements | In progress | One warm same-package check records actual timing, source/toolchain and competing load against the September 27 observation; further extraction requires measured benefit. |

Current development validation covers the resolved working tree with its merge
still uncommitted. The canonical Wallet Selection schema expectation and Cargo
feature guard now match their owners; all 65 feature-guard tests and the guard
command pass. The current source budget matches reviewed dependency costs
exactly. All 21 configured dependency boundaries pass locked offline Cargo
resolution with their manifest and lock inputs unchanged. The compiler source
guard seals 305 fixture includes and 605 test names; all 43 Python source-reader
controls pass. Surface/toolchain workspace lint inheritance and Surface public
Rustdoc still require correction. The recorded normal Core ZK library check
passes with its exact four default features, no compiler diagnostics and
unchanged captured source, Git, invocation
and selected managed inputs. Focused strict lint, the expanded Wallet registry,
current Privacy regressions and final workspace validation remain open. These
scoped checks do not qualify the final merged candidate.

The implementation separates compiler, Core ZK, Privacy, timed-OVN, SDK and
service ownership. Existing source guards retain the 107 declared-target and
24-default-target inventory, all 21 configured dependency boundaries, required
Norito/IVM-only constraints. SDK Native custody and genuine production proving
remain mandatory even with SDK defaults disabled. Earlier component passes
remain scoped to their recorded source cuts; subsequent source changes require
fresh qualification. The earlier ordinary Privacy suite ended with 52 failures;
none may be removed, ignored or replaced with a narrower successful subset.

The September 27 baseline checked `irohad`, `iroha_cli` and `iroha_kagami`
without incremental compilation after a data-model edit: 526 seconds overall,
including Torii 187.0, data model 126.1 and Core 119.5 seconds. A warm check on a
different source and toolchain is an observation, not a causal speedup or memory
measurement. Final timing awaits the merge owner's closed candidate.

The [ownership and validation note](../docs/validation/compile-bloat-ownership.md)
records component evidence and remaining checks. Pinned Mac/Linux resource
qualification remains roadmap A5: measured introduced-unit limits, the model
reduction target and the release memory ceiling require actual supported-runner
measurements. Source moves and debug component checks do not satisfy those gates.
