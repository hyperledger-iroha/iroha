# Deferred handoff carrier validation

The six structural carrier predicates in
[`SumeragiV2AsyncNetwork.tla`](../../../formal/sumeragi_v2/SumeragiV2AsyncNetwork.tla)
replace eager membership in the full deferred-handoff function set. The original
raw candidate, proposal, evidence, causal-origin and handoff sets remain intact.
Recognition preserves independent fields, the exact inactive sentinel, complete
candidate identity and validator-map domain. It does not add the stronger
candidate ownership or leader correlations.

The applied model SHA-256 is
`1a6a4dac5e6f22b83fccbca0b3aa70248dbe121113d0b58394a23f1747fec1e0`.
The application packet is
`target/first-release-deferred-handoff-applied-20260921/identity.json`.
Only the two existing whole-model identity seals changed alongside the reviewed
model, source checker and maintained mutation controls. No proof receipt,
invariant list, fairness assumption, configuration or release status changed.

The target-only equivalence packet contains seven finite positive profiles and
71 intentional mutations with expected counterexamples. These compare the raw
products, union branches, normalized images, constructor identity and maps.
They are component checks, not protocol runs or an unbounded proof.

The canonical maintained controls passed:

```sh
target/first-release-tools-venv/bin/python -m pytest -q \
  pytests/scripts/sumeragi_v2_proof_ledger_test.py \
  -k 'deferred_handoff_raw_type or candidate_causal_origin_type'
```

Result: **232 passed**, 5,917 deselected, 125.33 seconds. The selection includes
the prior normalized-origin controls and the new raw-carrier source/mutation
controls. Canonical multilane preflight also passed.

## Full liveness retry remains failed

The subsequent canonical retry retained seed 463837, 100 traces, depth 100, one
worker, set cap 1,000,000, all ten invariants and four temporal properties. It
used pinned TLC 2.19 with JAR SHA-256
`936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88`.

TLC exited **255** after 6 minutes 32 seconds. It reported one initial state
and then attempted to enumerate `Nat` in a set difference. No simulation
progress or successful liveness result was produced. The exact command, log,
input copies and failure checks are in
`target/first-release-tlc-liveness-deferred-handoff-20260921/`.
All recorded model/configuration/tool inputs remained unchanged. The production
source manifest was identical before and after:
`55706aa67adf0eaed6bf51cee178d4e88b98a8222d8564054775e7e5db66c4c8`.

The subsequent exact-clause diagnostic identifies
`AsyncLeaderWireLifecycleTypeInvariant[2]`: subset membership in the raw
16-field lifecycle constructor image attempts to enumerate `Nat` even for the
empty lifecycle set. It exits 255 after 782.56 seconds with unchanged copied
inputs and tools. The preserved packet is
`target/first-release-deferred-handoff-applied-20260921/natural-domain-clause-diagnostic`.
This diagnostic is separate from qualification.

## Raw leader-wire image repair

The reviewed successor recognizes the exact constructor fields and original
input domains pointwise. It changes only the two observed raw-image consumers;
the original image, all stronger lifecycle conditions, actions, fairness,
configurations and unbounded natural ordinals remain intact. Its model SHA-256 is
`4f64a75d66efecc01f9c69a6e59151457621bb4b71d858beda61e3f78d4c05b4`.
The exact four-file application and unchanged index are recorded in
`target/first-release-leader-wire-carrier-applied-20260921/identity.json`.
The two model pins identify the new source; they are not deductive proof receipts.

The reviewed target mirror passes 293 maintained source/mutation tests. A finite
comparison checks 4,631 record values, including ordinals 2,000,001, independent
contexts, zero physical coordinates and every missing field. All 25 semantic
mutations produce actual counterexamples. SANY passes. These finite controls do
not discharge the record-foundation and unbounded extensionality obligations.

Canonical multilane preflight passes after application. The subsequent full
liveness retry under `target/first-release-tlc-liveness-leader-wire-carrier-20260921`
retains the same seed, traces, depth, invariants and properties described above.
It passes initial-state checking, then exits **151** after 6 minutes 26 seconds
because `AsyncFreshLeaderWireLifecycleAdmissionsForNodeThisStep` reads
`asyncLeaderWireLifecycles'` before it is assigned during action enumeration.
Exactly one state is checked; all observed inputs and tools remain unchanged.
The log SHA-256 is
`6d8c32d1095181283fa74930fd4e6bfd25604823c443c95592344e037fa6969a`.

A target-only diagnostic moves the existing leader-wire transition before
the control-service transition in `AsyncNext`. Its conjunction clauses are
otherwise identical, and the canonical model remains unchanged. The packet is
`target/first-release-leader-wire-action-order-diagnostic-20260921`.
The diagnostic exits **255** after 811.44 seconds with one initial state and
`WrongInvocationException: Attempted to construct a set with too many elements
(>1000000)`. Its copied inputs and tools remain unchanged; log SHA-256 is
`b3280e4c33e46a5fda7fa42e4c67866453245fd97c5eaf75acde75ddd11b03cb`.
This is a failed diagnostic, not successful canonical liveness evidence.

The ordering review also identifies primed-frame reads in both activation
actions. The three-action repair preserves every original conjunct and awaits
its maintained source/mutation run. A separate copied-model probe retains each
original body in both branches of fifteen `PrintT` entry wrappers to locate the
new forced Cartesian carrier. The complete domains, configuration, seed and
invariants remain unchanged. Its first packet,
`target/first-release-leader-wire-cartesian-probe-run-20260921`, exits **150**
during SANY because the diagnostic omitted the explicit TLC import needed by
`PrintT`. This instrumentation failure is preserved; it supplies no model result.
The corrected copy in
`target/first-release-leader-wire-cartesian-probe-run-2-20260921` changes only
those imports. It exits **255** after 941.01 seconds with one initial state and
the same million-element set construction exception. Copied inputs and Java
remain unchanged; log SHA-256 is
`fe26bd42ca872b5f276d5ccb9fd9d316384bd71162dd1e91adbb484b1653f008`.
Its entry markers reach candidate services, candidate/evidence carriers and
route-neutral candidate evidence; they do not identify an exact failing
operator or establish successful liveness.
TODO: identify and repair each forced enumeration with its exact equivalence
argument, discharge the deductive bridge on the pinned prover, and retain all
remaining failures without calling component checks a full liveness pass.

## Deductive and tool boundaries

Normalized-image equivalence requires the existing finite-wire typing premise.
The comparisons concern function/record-valued coordinates; they do not prove
that `DOMAIN` is total on arbitrary atomic TLA values. The Stage2 structural-to-
carrier bridge must derive these premises and original carrier membership.
Neither stronger ownership assumptions nor finite component tables discharge it.
The target-only conditional bridge parses with SANY but has no TLAPS receipt.

The official pinned TLAPM installer fell back from the unavailable release asset
to immutable source commit
`3ab43c7ff31db4ced850619d4746fa4c841a7681`. After fetching its pinned inputs,
installation stopped because the locked Z3 4.8.9 x86 binary cannot execute on
this ARM64 macOS host without Rosetta. A direct x86 runtime probe also failed.
`target/first-release-tlapm-install-3-20260921/` preserves that refusal. No
substitute solver, successful installation or deductive proof is claimed.

TLAPS, the unfinished 18-step Apalache obligation, Verus, the complete canonical
model matrix and production resilience/scaling qualification remain open.

## Scheduled-carrier diagnostic

The target-only finite pre-state scheduled union, guarded by the existing raw
carrier predicate, was independently reviewed as a conditional algebraic
restriction for three service comprehensions. It retains the raw/normalized
carrier proof obligations; no canonical model or seal changed. The complete
liveness configuration still fails with a set larger than 1,000,000 elements
after 854.06 seconds and one initial state. All copied inputs/tools are unchanged.
The packet is `target/first-release-scheduled-service-carrier-diagnostic-20260921`;
its log SHA-256 is
`f69681ece1d2ad9f9d879d034d03e891ddf6da64ebdaa9ab2ba53456215c7742`.
An unchanged-model rerun with JVM exception-stack recording is diagnostic work,
not formal qualification.

## Located terminal-discard enumeration

The unchanged copied-model Java exception diagnostic exits 255 after 838.27
seconds with unchanged inputs and tool bytes. Its local JDI observer identifies
`AsyncCandidateEligibleTerminalDiscardsThisStep` as the actual candidate-universe
enumeration, forcing the route-neutral authority image while forming a record
set. The retained expression-stack SHA-256 is
`59363310c67271c4c0c7fc4ca344784ad051716a54fdad80c60da8f483244f3e` in
`target/first-release-tlc-expression-stack-20260921`.

A separate copied-model diagnostic ranges that eligibility filter over the
already computed terminal-discard set. The complete configuration, one worker,
100 traces, depth 100, seed 463837, million-element ceiling, invariants and
properties are retained. Equivalence depends on the still-open raw/structural
candidate-carrier subset bridge; no deductive or canonical qualification is
claimed. Canonical formal source and source-fidelity seals remain unchanged.
