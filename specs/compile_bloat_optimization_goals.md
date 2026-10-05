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

The merge is closed, but final current-candidate qualification remains open.
Feature hygiene passes all 65 tests on its recorded cut. All 21 configured
dependency boundaries pass locked offline resolution on the current post-Journal
cut, with 119 manifests, 113 workspace members and all 141 protected inputs
unchanged. Reviewed exact source costs are applied without unused allowance.
The five post-Journal source guards pass with 464 protected inputs unchanged;
all 72 dependency metrics match their reviewed limits. The compiler source guard
seals 308 fixture includes and 616 test names; all 43 Python source-reader
controls pass with 323 captured inputs unchanged. Surface/toolchain inherit
workspace lints and Surface public APIs are documented. The shared ABI argument
and numeric codec extraction is applied; VM decoding, gas and memory custody
remain in IVM.
On its recorded coherent source cut, the full default-library run passed
4,841 Model, 210 ABI and 33 Surface
tests with 24,823 captured source inputs unchanged, retaining the 135 original
Model ignores and no filtering. The new codec, const-name and distinct-supply
Mint/Burn regressions pass. All 13 strict style findings have source repairs;
fresh strict checks and direct IVM/SDK consumer validation remain pending.
A subsequent AssetId JSON serializer source change requires fresh Model and
dependent native validation.
Shared operation journaling and its Foundation CI routing are implemented, and
all 88 selection controls pass. Native Journal/Wallet qualification remains
pending.
The recorded normal Core ZK check retains its exact
default-feature/source scope; current
strict Core ZK, toolchain, Wallet runtime, Privacy regressions and final
workspace checks remain open. These component results do not complete O1,
O2, O6 or O7.

The implementation separates compiler, Core ZK, Privacy, timed-OVN, SDK and
service ownership. Target guards admit 105 declared binaries and 23
defaults under the unchanged 24-default ceiling; the 21 configured dependency
boundaries and Norito/IVM-only constraints remain required. SDK Native custody
and genuine production proving remain mandatory even with SDK defaults disabled. Earlier component passes
remain scoped to their recorded source cuts; subsequent source changes require
fresh qualification. The earlier ordinary Privacy suite ended with 52 failures;
none may be removed, ignored or replaced with a narrower successful subset.

The September 27 baseline checked `irohad`, `iroha_cli` and `iroha_kagami`
without incremental compilation after a data-model edit: 526 seconds overall,
including Torii 187.0, data model 126.1 and Core 119.5 seconds. A warm check on a
different source and toolchain is an observation, not a causal speedup or memory
measurement. Final timing requires a qualified current candidate.

The [ownership and validation note](../docs/validation/compile-bloat-ownership.md)
records component evidence and remaining checks. Pinned Mac/Linux resource
qualification remains roadmap A5: measured introduced-unit limits, the model
reduction target and the release memory ceiling require actual supported-runner
measurements. Source moves and debug component checks do not satisfy those gates.
