# ZK first-release completion goals

Set: 2026-09-26. Execution resumed: 2026-09-30. Reviewed: 2026-10-03. Overall status: **Active**.

This record owns the remediation requested after the current-source ZK critique.
It supplements [first-release completion](first_release_completion_goals.md) and
[privacy closure](privacy_first_release_closure.md). Implementation remains in the
existing `/Users/takemiyamakoto/devstuff/iroha` checkout on `optimizations`.
All further implementation and validation use this checkout; earlier separate
candidates are historical evidence only. Unrelated changes remain.

The outcome is useful, safe proof algorithms with simple developer APIs. Renaming,
disabling, status objects, source hashes or relaxed caps do not complete an algorithm.
Dedicated review uses independent derivations and adversarial controls bound to
the actual artifacts. Implementation tests alone do not establish cryptographic
qualification; physical-device and signing claims require their actual runs.

The [September 30 continuation](../docs/history/2026-09-30/zk-completion-continuation.md)
retains earlier native/SDK provenance and validation boundaries.
The six unfinished outcomes remain ZK03 through ZK08; no running build, source
review or partial test selection closes one of them.

## Goals and acceptance criteria

| ID | Status | Owner / outcome | Completion criteria |
| --- | --- | --- | --- |
| ZK01 | Complete (implementation) | Vega / secret-safe proving | Every private commitment uses public work dimensions and constant-time secret arithmetic; no raw uncleared witness copies cross the MSM boundary. Differential arithmetic, zero/partial/full row, failure/cleanup and worker-bound tests pass. Report resource limits only when actually enforced. Review other reachable secret hashing scratch, including ZK-ACE. Target timing and independent qualification remain ZK06. |
| ZK02 | Complete (implementation) | Confidential circuits / optional inputs | One owned note can be fully redeemed at maximum tree capacity. Absent inputs require no empty-leaf membership or caller-created dummy witness. Ownership, nonzero/duplicate-nullifier, range and conservation constraints remain enforced. Regenerate all changed circuit keys, digests and dependent fixtures; reject superseded keys. Positive and adversarial circuit/native/SDK tests pass. |
| ZK03 | In progress | Core / honest proof semantics | Generic verification cannot confer a stronger guarantee than its compiled relation. Retire IVM binding-only circuits, registrations, keygen, service routes and SDK/CLI consumers. Production IvmProved admission requires the complete native execution relation and authoritative finalized State anchor; replay or caller-supplied commitments cannot substitute. Implement the complete RAM-LFE program relation before enabling proof receipts. |
| ZK04 | In progress | FASTPQ / bounded private verification | Implement a sound source-state-bound relation with reviewed trace/composition masking and bounded verifier work; fit unchanged proof and total resource limits. Produce and verify real maximum-shape proofs, negative source/witness/statement cases, and CPU/accelerator parity. Full replay and unmasked offline compact proofs do not satisfy this goal. |
| ZK05 | In progress | ZK-X509 / complete bounded credential proof | Redesign or compose the full certificate, CRL, disclosure and ownership relation below 9,437,184 bytes without reducing supported coverage. Account for all segments, recursion, openings and prover resources together. Regenerate fixed profiles and produce actual positive/negative proofs before activation. Arithmetic size projections alone are insufficient. |
| ZK06 | In progress | Cryptographic qualification | Obtain independent artifact-bound soundness, zero-knowledge, Fiat–Shamir/qROM, digest/multi-target, arithmetic and side-channel evidence for the selected release protocols. Keep explicit protocol-specific blockers; do not infer qualification from implementation markers. |
| ZK07 | In progress | SDK / simple developer workflow | One typed prepare/prove/verify workflow per actual capability, actionable errors, early availability/resource checks, secure randomness and private witness ownership by default. Callers do not construct dummy inputs, choose transcript internals, or infer guarantees from backend names. Maintain executable Rust examples and equivalent supported SDK entrypoints, with public guidance in `iroha-docs`. |
| ZK08 | In progress | Validation / reproducible candidate | Reconcile current failing Halo2/note-STARK source contracts by preserving their substantive assertions. Run changed-function tests, real proof/adversarial suites, serialization guards, formatting and applicable SDK tests; then the workspace and four-validator qualification on a fixed candidate. Record exact passes, failures and unexecuted checks separately. |

## Execution order

1. Repair secret arithmetic and optional-input constraints with regression tests.
2. Make guarantees explicit at verifier/developer boundaries; migrate callers and remove stale claims.
3. Use one allocation-free resource plan at developer preflight and actual prover
   admission. Resolve the mathematical construction before widening execution.
4. Complete FASTPQ and X509 constructions against fixed statement and resource
   contracts, then regenerate profiles, keys and fixtures.
5. Qualify end-to-end SDK/native/network paths and independent cryptographic
   evidence. Close each goal only with its acceptance evidence.

## Current implementation and evidence

The Core/SDK merge and reviewed repair cohort are committed as
`ab6d83b7216f4e8ec00b29a31baeed0637929600` on `optimizations`. The commit
changed Git identity during a native producer's terminal check while leaving
all source bytes and modes unchanged. That failed terminal check is retained.
Follow-up repairs and fresh validation bind this commit, the exact working
source and semantic index; historical runs do not qualify the combined candidate.

The repaired sixteen-target Core/Kagami compilation passes with stock Rust
1.93.1. A typed `ProposalContentId` mutation preserves the negative authorization
fixture. Unrelated documentation changes fail the wrapper's later source guard;
the actual successful compiler output and all sixteen immutable artifacts are
retained. Their selected controls and a fresh lifetime doctest remain required.
The preceding fixed candidate's sixteen-target Core/Kagami build and original
lifetime compile-fail test pass. Its 3,520 planned executions end with 3,393
passes, 32 fixture failures and 70 controls left unexecuted by a default-stack
overflow. All 757 selected bridge controls pass. Reviewed repairs retain original
authorities, capacity checks and assertions; their fresh native replay remains
required. Full Mint/Guard/receiver, genuine proof/export and full-State authority
remain incomplete; the unresolved runtime-verifier schema is not replaced by a
diagnostic schema. Governed standby retirement and original-context publication
repairs and paired native retirement fixtures are applied. A separate current
fixture captures all 105 typed codec owners and 1,555 nominal identities; the
1,551 still-active historical rows retain their exact hashes. The new ordinary
checks, reduced-feature checks and native publication replay remain pending.

An earlier fixed candidate passes 78 native private-dispatch controls and
seven public MEAN controls, 137 ordinary RFC controls, both actual ignored RFC
auxiliary oracles, current profile/IO/Projection/schedule pins, maximum geometry
and degree-four inventory. Current private ISQRT and DIV_CEIL relations reuse the
original 21 ports and existing banks. The preceding private namespace records
86 passes and three fixture failures; two opcode inventories are stale and one
padding mutation leaves an existing one unchanged. Repairs are applied. All
144 preceding RFC controls, five inverse-window controls, two temporal controls
and both ignored RFC auxiliary oracles pass. The complete 210-record inverse-window
experiment preserves every column but runs 1.535 times slower than the former
scalar path, so it remains test-only; the fixed-work successor needs a fresh
comparison. Two public MEAN tests pass natively but fail procfs
observation; exact repeats pass the unchanged resource gate while retaining the
original failures. The FFT benchmark completes 13 cases and 156 records; its
parser correction preserves the original inventory failure. The applied shared
coarse CPU scheduler still requires native and complete-proof validation. The
first fixed-work inverse retains a compiled private-gate branch. Its successor
is applied, masks before validation and passes extracted native semantic controls.
Integrated AArch64 inspection confirms that helper repair; surrounding private
recurrences and whole-prover evidence remain open. The inverse-window collector
uses the same masking order. Its rebuilt artifact passes all ten
parity/arithmetic/resource prerequisites and all 210 benchmark records; the sum
of per-span medians improves from 64.412 to 32.902 seconds (1.958 times).
Integrated AArch64 inspection covers the repaired collector. The bounded window
is selected in the source with one shared clearing owner; fresh production-route
controls and a complete bounded proof remain required. The original
documentation-drift wrapper failure is preserved. These results establish
component evidence, not quiet-host or whole-proof performance.
The phase-aware X509 resource repair compiles. Its native producer captures all
27 selected outcomes: 22 passes, four stale assertions and one native success
rejected by the resource observer. Numeric and digest pins are regenerated from
actual emissions. All original controls, the private-note profile check and a
complete bounded proof remain required under unchanged limits. Earlier immutable full-suite processes finish naturally and retain
source-drift outcomes; no earlier result qualifies this combined source.

The last complete X509 maximum remains the source17 diagnostic: 9,412,912 bytes
with positive, fresh-verifier, wrong-genesis and corruption controls passing,
but 2,275.987086 seconds against 300. The old RSS-consistency failure is retained.
The new observer takes the maximum of actual live, high-water and terminal
measurements. The unchanged limits remain 9,437,184 proof bytes, 12 GiB RSS,
32 GiB enforced address space and 300 seconds. Current profile
`294649842da2b565e94b022ce59c7a7c662aa2bf70502d8a554abb510d487852`
has 1,945 residues of degree at most four. Scheduler and inverse experiments
cannot substitute for a complete bounded proof.

FASTPQ's maximum ordinary and AXT producers and fresh verifier replays pass on
the preceding fixed candidate, as do 52 local native/SIMD/Metal and 13 entropy,
preflight and owned-erasure controls. Conditional adaptive-QROM review now
includes explicit witnessless/conditional sampling and honest-conflict bounds.
Concrete hash/entropy, whole-prover side-channel, other hardware and finalized
network qualification remain separate unfinished requirements.

The recorded script suite passes 718 tests and 163 subtests; all 58 typed workflow
controls pass. The preceding Kotlin/JNI run records 1,824 passes and one stale
bootstrap fixture failure among 1,825 tests, with all six full-tree native
controls passing. Android client managed tests record 421 passes and one
signing-vector fixture failure; both corrections are applied. Actual client
host JNI passes 95 controls, wallet host JNI passes one and wallet managed tests
pass 91. JVM tools pass 30 controls and the native example passes. The compiled JNI census
also rejects four Android-only methods absent from the Darwin library; correct
target-specific native coverage remains to be established. The preceding
ABI-25 host library loads all 120 required symbols; all ten genuine generator
modes pass twice with identical paired outputs. Paired Kagami, quantity, native
IVM and public-codec captures and selected consumers pass on their recorded
source. The possession-journal directory and missing Swift test-endpoint method
are repaired. Fresh complete consumers, the typed Swift incoming workflow and
the complete Cash-owner lifecycle remain required. Apple packaging for five
native targets and three XCFramework slices, full Swift tests, physical devices
and signed releases remain open.

The preceding whole-workspace all-target check ends with six Torii test API
errors; strict all-target Clippy finds 43 diagnostics. Reviewed corrections are
applied and require replay. The daemon, CLI and all three integration harnesses
build. Four-validator transaction commitment passes. The lane restart scenario
stalls after lane height 1 commits while global height remains 3; an exact-source
repeat reproduces it. Initial payload construction now includes a nonempty lane
merge section's canonical time floor, avoiding a second parent-certificate build.
Exact-wire/build-count controls and the unchanged network scenario still need
fresh execution. FASTPQ and wallet network scenarios fail before their proof
assertions on asset-domain and initial-executor admission. Their repairs retain
proof validation and require fresh network execution. The authored OpenAPI
copies include the same closed retirement schema; source-guard and native route
validation remain required. The complete workspace,
selected controls, authentic fixtures and all four network scenarios remain
required. Historical failures are preserved. No goal ZK03–ZK08 is closed by
partial tests or review.

### ZK01 — secret arithmetic

Vega private commitments use fixed-window secret MSM, public dimensions and
clearing scalar owners. ZK-ACE borrowed private preimages and reachable hashing
scratch clear. Accelerator allocations clear only after observed completion;
unknown completion quarantines their owners and blocks further private admission.
Compiler-created copies, device timing and independent review remain ZK06.

### ZK02 — optional confidential inputs

Absent inputs require no empty-leaf path. One owned note can be fully redeemed at
the 65,536-leaf maximum, or redeemed with private change. Ownership, range,
conservation and nullifier constraints remain enforced. Canonical replacement
keys reject superseded keys. Same-candidate network, distribution and device
qualification remains open.

### ZK03 — complete proof semantics

`zk::verify_for_relation` admits only an explicit role supported by its compiled
relation. IVM replay-binding circuits, registrations, keygen, service routes and
associated SDK/CLI consumers are retired. Production `IvmProved` admission stays
closed until the complete native transition relation and authoritative finalized
State binding exist; G3 in the [IVM goals](kotodama_ivm_completion.md) owns this work.

RAM-LFE registration, activation, restoration, evaluation and receipts reject the
insecure signed/proof BFV modes before private work. The exact-lift profile loses
public-key noise modulo 257. The diagnostic interpreter's clearing tape, bounded
initializer, reduction and commitments are prerequisites, not an execution proof.
Secure encryption replacement, exact refresh, malicious-key/input validation,
circuit privacy, the complete program relation and integrated qualification remain
open under the [replacement contract](ram_lfe_encryption_replacement.md).

The candidate [packing](ram_lfe_plaintext_packing.md) and
[semantic](ram_lfe_semantic_commitments.md) interfaces preserve private length,
ordered scalar output including 256, clearing owners and explicit public
policy/key/profile/context binding. They authenticate no submitted key or proof.
Input and proof-envelope caps remain independently 1 MiB, with the configured
proof cap at 192 KiB. Input and output ciphertexts do not share the same input
frame; complete replacement proof encoding and total working memory remain open.
A separate fixed-basis native RNS key-switch component passes ten native tests,
nine complete public-fixture parity cases and 16 adversarial/public controls;
independent review verifies its public roots, primes and exact division. Its
3,031,040-byte owned allocation bound applies only to that component. Native
policy/key custody, complete refresh, secure parameters and the full
program relation remain open; the component is not a qualified encryption
construction. The proposed full-ring N=131072/QP2725 refresh point fails its
ordinary-LWE quantum screen at about 122.165 bits against the 128-bit target. A
smaller mixed-width QP2589 candidate satisfies the conditional arithmetic bounds.
Its original nonfinite BDD run remains a failed screen; certified numerical
repair completes the twelve original attack cases, but the expanded ChaLoy21
quantum screen fails at 127.472 bits. The exact QP2571 successor passes twenty
ordinary-LWE model cases, with a minimum 128.4063 bits, and all five selected BDD
minima pass full-factor interval checks. Scoped ring-primal and small CRT
projection bounds do not establish joint ring, algebraic or related-key security.
The return key targets an embedded old-ring secret, requiring separate analysis:
its modulo-special-prime screen is incomplete, while the disjoint-digit model
has finite estimates but leaves its multimodal error structure unqualified.
The direct-bootstrap research candidate satisfies a conditional positive-width
refresh bound; the earlier public-key bootstrap construction fails that bound.
Its adaptive-key joint-randomizer reduction requires a separate Gaussian-RLWE
assumption, extractable common-secret admission, authoritative entropy and finite
sampling evidence. These are mathematical prerequisites, not implemented or
qualified encryption. The two-secret cycle, full refresh, circuit privacy and
complete program relation remain open. No candidate is selected or activated by
these screens.

The held native canonical-input/digit successor passes 19 component tests,
including nine new controls. It owns both complete canonical ciphertext components,
derives balanced digits without an external digit stream, shares the original
allocation ledger, and applies each selected Galois automorphism to that retained
input. Direct residue parity covers both identity-role input components and three
Galois powers; independent integer review reconstructs all 32,768 coefficients and
131,072 digits. The combined owned allocation bound is 4,079,616 bytes; the test
process reaches 44,597,248 bytes RSS. Neither is complete application qualification.
Immutable policy/key custody, common-secret validation, secure parameters,
relinearized assembly and the full program/refresh relation remain absent.

The historical IVM library selection passes 1,026 controls, and all four selected
frame-initialization controls pass. The repaired memory-bank selection passes all
26 controls: seven initialization, ten LOAD and nine STORE. STORE fixtures now
capture the VM's initialized register values and preserve byte, log, gas, PC and
cycle assertions. The separate scalar/AIR
proof controls have no complete current-source result. Reviewed LOAD/STORE banks
remain unregistered prerequisites. Memory authority, initialization/history,
execution sequencing, integrated resource accounting and finalized State binding
remain open. All 15 private lifecycle/history/access controls pass in the
normal optimized native build. This component binds original private
dispatcher, Owner and Initialization ports and retains canonical inactive packets;
the added private descriptor/copyback and partial-dispatch relations pass their
26 selected controls before the merge. Both genuine native capture producers pass twice on source17 with identical
bytes, and all four original AIR consumers pass on those captures. These remain
component equations, not a complete private invocation proof.
Governed runtime availability now
has a distinct local deferral path, and publication checks the authenticated
registry and immutable cache without transferring local key ownership into
consensus state. Full instruction/region semantics, masking integration and the
original finalized State joins remain open.

The private dispatcher now constrains the original owner depth to the native
0..=1,024 range, with exact push/pop transitions and root depth zero. The native
limit has one shared owner; the reviewed change preserves its value. The private AIR depth controls and full-depth native LIFO/slot-reuse control
pass on source17 with default stacks.
The private dispatcher also has original-port scalar arithmetic, bitwise,
signed/unsigned/equality comparison and all six conditional-branch equations.
Branches use the same constrained predicate for the PC, require both native
operand tags to be public and preserve the original 21-port history join.
Prepared-contract admission checks both successors. Scalar, branch, alias/r0,
gas/cycle, degree and shared-bank controls await native execution on the combined
source. The same original ports constrain all seven native shift/rotate
operations, low-six-bit amounts, operand tags and native gas costs, plus NOT,
NEG and signed MIN/MAX. The comparison relation charges the native two gas.
All 65 private-dispatch controls pass in the retained optimized Linux artifact,
including coherent wrong-gas forgeries, seven unary/select controls, four
multiplication controls and four bit-count controls. POPCNT, CLZ and CTZ reuse
the original zero-prefix equations over canonical source bits with native
six-gas, one-cycle behavior. CMOV/CMOVI now bind the full public condition and
only the native conditionally read source and written destination; native controls
pass. DIV/DIVU/REM/REMU reuse the original arithmetic bank and preserve public
trap-sensitive operands, native gas and the original 21 packet ports. The GETGAS, ABS and signed MEAN/shared-bank controls pass in the retained
78-test private-dispatch namespace; all seven shared public MEAN controls also
pass. MEAN owns exact 65-bit signed addition and truncation toward zero, 44 result
cells, 21 original ports and native two-gas/three-cycle behavior. The newly applied
ISQRT relation constrains the 32-bit floor root and exact remainder in the existing
product bank; DIV_CEIL constrains signed correction in the shared MEAN bank.
Their original tags, history ports and native gas/cycles are retained. All 97
private-dispatch controls pass on the rebuilt artifact, including the repaired
inventories and padding control. The newly applied successful LOAD64 and STORE64
relations bind the original memory, initialization, frame, control and history
ports in four phases. Closed opcode selection rejects other running opcodes;
the new nineteen LOAD/STORE and inverse-route controls need fresh native execution.
Complete instruction/region semantics and finalized State binding remain
open; these components do not enable IVM proof admission.

### ZK04 — FASTPQ

The public Quantity facade uses the masked bounded DEEP producer and independent
verifier. Expected statements derive from canonical authenticated context; proof
consistency alone does not authorize remote spending. Development replay APIs and
required-Metal readiness preserve their existing boundaries.

On the preceding fixed candidate, all 16 ordinary controls, both actual maximum
producers and both fresh verifier replays pass. The ordinary artifact is 972,623
bytes (SHA-256 `c6bc0bb2d94e49fd5743995c767c121d313933805ca8dc517ce7ff1126ffdfba`),
with 1,494,040,576-byte RSS; the AXT artifact is 1,020,465 bytes
(SHA-256 `10f633d4409cb54bf07608c0da49e8519f8cad34d242e532aebd38d6e22292b5`),
with 1,508,605,952-byte RSS. Actual child times are 836.194 and 874.408 seconds;
fresh replay times are 0.316 and 0.348 seconds. These are observed durations,
not quiet-host comparisons. Both fit the unchanged 1 MiB artifact, per-segment
2 GiB charged-payload and 2^42 work caps; payload accounting is distinct from RSS.

The same source passes 52 actual local hardware controls, including nine
required-Metal controls, independent hash vectors, NEON state/scratch clearing,
exact-root transform parity and both private-next affine AIR guards. Thirteen
actual entropy/preflight/owned-erasure controls pass separately. Production
mask sampling uses fresh fallible OsRng and bounded unbiased canonical-field
rejection; no test seed is admitted by the public Quantity facade. This covers
the available M1 Ultra, not every target or whole-prover side channels.

[FASTPQ readiness](fastpq_production_readiness.md) owns unchanged resource limits.
Independent ideal-QROM review now covers explicit public simulator/conditional
completion kernels, adaptive request channels and honest-conflict accounting.
The retained bound is below 2^-91 for at most 108 attempts and 2^32 oracle queries
under its explicit ideal assumptions; this is not a 128-bit work-factor claim.
Concrete Keccak/entropy, complete zero-knowledge/side-channel, remaining hardware
and finalized-source network qualification remain open. Changed candidates need
fresh integrated evidence; historical seeded CPU/Metal parity is not inherited.

The applied ordered-source census charges one row for every original input,
including empty and rejected entries, and binds the original canonical frame,
context and execution usage. The source17 Core selection passes its ordered-source controls. The newly applied
retained quota-ledger identities/generations and sticky publication checks still
need native validation. These are source-custody components; complete finalized-source
admission remains unavailable.

### ZK05 — ZK-X509

The current relation retains all 49 MAIN registrations, compact CA, the full
certificate/CRL/disclosure/ownership shape, key/digest byte joins and the original
masked trace owners. The joint protocol removes all 320 remaining public terminal
values through original-polynomial equations. Both auxiliary roots precede active
composition challenges; both composition and FRI-mask roots precede the common
DEEP point. All joint openings precede local mixing. The
[joint relation contract](zk_x509_joint_private_relation.md) records the exact
schedule, geometry and still-conditional algebraic ledger.

The source17 Linux CPU maximum proof is 9,412,912 bytes, under the unchanged
9,437,184-byte cap. Its SHA-256 is
`8170e2b6ae2bd4c0f504c67943b077d66d4944c61ca573a150265269ab8883ce`.
Native production/self-check, separate fresh verification, wrong-genesis rejection
and tampered-proof rejection pass. Both actual proof and verifier children run
under the unchanged hard and soft 32 GiB address-space limit; sampled producer
virtual size reaches 9,950,617,600 bytes. This is a sampled lower bound on peak,
not a substituted adjusted memory figure.

Maximum resource qualification fails. Proving takes 2,275.987086 seconds against
300 seconds. Composition takes 671.01 seconds, DEEP/FRI 291.24 seconds and query
openings 841.22 seconds, including 639.87 seconds in selected query transforms.
The corresponding initial joined transform takes 124.63 seconds. These stages
and nested counters must not be added twice, and timings from the older macOS
Metal run are not a same-hardware comparison. The producer observes 20 Rayon
workers; no complete worker allocation bound is claimed.

Terminal wait4 reports 8,431,575,040 bytes peak RSS, while live procfs RSS/HWM
reaches 8,432,234,496 bytes. Both are below 12 GiB, but the 659,456-byte discrepancy
fails the observer's consistency gate; the original failed result is retained.
The proof, receipt, fresh replay and observations are in
`dist/zk-remediation/2026-09-30/epoch19-linux-source17-maximum`.

Retained-query replay reuses original masks and authenticated Merkle cuts while
checking original roots and allocation capacities. The selected-coordinate
transform is correct in the current native controls and complete proof, but its
CPU performance needs repair. The new required 32-byte X509 instance nonce binds
dynamic MAIN/CA/joint hash contexts and consumes a separate checked RNG prefix.
Fixed schedule metadata stays unchanged. Its derived maximum is 9,412,944 bytes;
this is not a native maximum result. Authentic profile/proof/pre-aux values are
regenerated and independently checked, and the four primary diagnostics pass.
The merged descriptors enumerate the current 192 endpoint, 17 key/digest,
20 SHA-union and 108 CA-link alpha phases, retaining the claim-free joint wire
and 39 RFC lookup relations. Their current profile pin and deterministic
IO/Projection goldens bind the sole merged descriptor. Complete current-candidate
qualification remains required; earlier descriptor passes do not qualify it.
The scoped DER/profile fixture repairs now pass the 137 ordinary RFC controls
and both actual ignored auxiliary oracles on the preceding fixed candidate.
CPU/accelerator parity and a new maximum proof remain required.

The applied composition repair preserves public coefficient extents through
zero values and cancellation, with full initialized-allocation erasure. Arithmetic
fixed preprocessing now fills its already-owned public matrix once per row,
reducing full row constructions from 44,564,480 to 2,621,440 across the five
maximum registrations while retaining eight-column IFFT batches. This is a
work-count reduction, not a measured proof speedup. A test-only Horner comparison
preserves the original full selected-query baseline. Native parity, cleanup and
Linux one/four/twenty-worker timing remain pending for these changes.

Full semantic, soundness, zero-knowledge, transcript and side-channel qualification
remain open. Conditional classical-ROM/masking derivations do not establish
adaptive shared-transcript simulation, quantum hiding, complete relation
correspondence or resource compliance. Soundness/resource activation pins stay zero.

### ZK06 — cryptographic qualification

Dedicated independent source reads, derivations and adversarial controls are
recorded against exact protocol artifacts. Their scope is explicit: component
arithmetic, conditional soundness/hiding bounds and authentic native verification
do not establish complete soundness, zero-knowledge, transcript security,
side-channel resistance or release qualification. RAM-LFE construction and full
IVM semantics remain absent. X509 now has a verified joint maximum proof, but
complete relation/transcript/hiding review, time compliance and RSS observer
reconciliation remain open. The revised nonce candidate requires fresh evidence.

An independent bounded review accepts the source15 X509 classical-ROM argument
recorded in `epoch18-zk-dag-root3` and `epoch18-zk-dag-review-sdk3` under
`dist/zk-remediation/2026-09-30`. Its bound is below 2^-161 for at most 2^64
classical oracle queries, conditional on the reviewed algebraic/masking
prerequisites, a fixed statement/authority/geometry, one atomic proof, ideal
random oracles and independent random bytes. It does not establish adaptive or
multiple-proof security, qROM security, concrete hash/entropy guarantees,
side-channel resistance or failure-channel privacy. Subsequent source changes
require a fresh applicability review. Physical hardware and authenticated final
release artifacts require actual runs.

### ZK07 — developer workflow

Typed wallet workflows own clearing private inputs, canonical keys and circuit
selection; callers do not supply dummy inputs or transcript internals. JavaScript,
Python, Swift, Kotlin/Java and C# use the shared native owner with bounded inputs
and asynchronous custody. Managed strings do not promise erasure.

Historical host ABI-25 controls pass 62 tests with 397 expected exports, plus ten
top-up executions. Kotlin records 1,562 passes and six full-tree controls; Android
host/JNI consumers pass 291 tests. C# records 6,015 passes. Installed Python passes
4,370 tests plus 281 subtests and five genuine maximum-tree wallet controls.
Normal Kotodama generation and all 57 official sample mappings pass.

The last tested pre-merge host library passes a normal locked Rust check, test build
and dylib build. The actual loaded image reports ABI 25, exactly 359 exports and
117 Kotlin JNI symbols; all 38 retired Java-Android aliases are absent. All 674
ordinary bridge controls pass on unchanged source after the fixture repairs.
Two genuine top-up fixture-producer executions are byte-identical to each other
and the canonical fixture. Ten authentic Kotlin fixture modes also run twice
with identical output. The subsequent Kotlin consumer run refuses the changed
source before starting Gradle. Current merged native, Kotlin, Swift and device
qualification remains open.

Historical evidence covers all five Apple slices and ABI-25 package/resource checks.
The current Apple builder uses stock pinned Cargo and the original authenticated
root lock; it rejects compiler/configuration overrides and retires the temporary
compiler wrapper. Its source and workflow owner controls pass. The merged Swift
fixture correction is present; the full Swift host suite and
same-source Apple package rerun remain pending. The historical
JavaScript suite records 3,634
passes and 15 failures in bundle accounting, manifest fields, scope and artifact
identity. Reviewed corrections retain strict parsing and unchanged bundle caps;
the reconciled manifest and bundle/replication proposals pass five and 35 focused
managed controls. Full integrated/native rerunning remains required.

Historical Android native arm64/x86_64 libraries and diagnostic APK packaging
pass source, native-payload, signature, manifest and six-method DEX census checks.
Dependency resolution recovered the original offline-cache packaging failure,
which remains retained. Physical testing requires rebuilt current packages and
a connected device. The diagnostic APK's debug signature is not release signing.

A genuine merged-base Core/Kagami finalized-execution fixture producer and public SDK
bridge consumer pass. Paired schema, query/native fixture generation and the
executable examples also pass. All 185 maintained golden-generation commands pass;
both sets of 61 outputs are byte-identical and match canonical files. The
top-up producer's two executions and eight consumer controls pass. Current installed
consumers and packages,
physical Apple/Android evidence and signed final artifacts remain open.
The candidate RAM interfaces and stable unavailable errors confer no encryption
qualification. Public guidance belongs in `iroha-docs`; this checkout's build and
validation do not depend on that sibling repository.

### ZK08 — current source contracts

The preceding fixed candidate builds all sixteen Core/Kagami targets. Its 3,520
planned executions end with 3,393 passes, 32 fixture failures and 70 controls
left unexecuted by a default-stack overflow. Reviewed repairs are applied; the
original substantive assertions and default stack remain required. The workspace
check reports six Torii test API errors and strict Clippy reports 43 diagnostics;
reviewed corrections need replay. Daemon, CLI and three network harnesses build.
Actual four-validator transaction commitment passes, lane/global merging stalls
reproducibly, and both proof scenarios fail their setup/admission before proof
assertions. Their reviewed setup/admission repairs require fresh tests. Earlier
completed selections below are historical obligations, not inherited passes.

The historical `051df111` build passes workspace all-targets checking and Core/Kagami,
Torii/bridge, CLI/daemon and both network-test targets. The default Core selection
passes 1,145 tests across 70 groups with no failures or ignored tests. The original
247-control map has 246 exact-name passes and three genuine producer/consumer
controls replacing its retired fixture control; all 251 later selected names
also pass. A separate normal optimized Core build and all eight AXT proof
controls pass on unchanged source, for a 1,153-execution union. The original
retained selection accounts for 1,152 executions; the extra current test covers
authenticated-genesis successor ordering and rejected-interval poisoning. The
earlier source-drifted run remains unqualified.

Both four-validator component controls pass on that unchanged source:
transaction commitment and lane transactions across a full network restart.
Each observes exactly four concurrent daemons. These are component scenarios;
finalized FASTPQ authority, wallet workflows and full release qualification
remain open. Compiler artifact recovery never converts a failed command into a
pass.

The pre-merge private SHA/RFC production and optimized test builds, three authentic
pins and all 1,061 selected ordinary controls pass. Genuine Core/Kagami/SDK fixture
production and all 674 ordinary native bridge controls also pass. The Core run
records 1,084 passes and 30 failures among 1,114 executed controls; its last group
overlaps the merge and 66 planned controls are unrun. Source guards refuse later
privacy, MV and Kotlin stages. The applied committed-marker and fixture repairs need
one fresh integrated source, genuine producers, expanded Core controls, workspace
checks and four-validator scenarios. Historical failures remain failures. The
unadapted release-evidence Python suite passes 130 tests on its recorded source.

## Remaining execution sequence

1. Replace the insecure RAM-LFE encryption construction, retire diagnostic public
   surfaces, and complete its semantic relation and current SDK/consumer controls.
2. Preserve the clean FASTPQ maximum ordinary/AXT production and replay evidence;
   qualify finalized-source network and hardware behavior without changing proof
   or resource limits. Rerun affected proofs after any protocol/source changes.
3. Complete X509 relation/transcript/hiding review, repair measured CPU time and
   reconcile RSS observation, with
   native parity, independent review and another complete maximum proof under
   unchanged coverage, byte, memory and time limits.
4. Complete IVM G3's native execution relation and finalized authority. Validate
   rejection of retired APIs and relation-confusion attempts in current consumers.
5. Capture one integrated source after the reviewed repair cohort; rebuild SDK artifacts,
   run applicable workspace checks and current four-validator tests. Preserve
   `3f + 1` committees, exact `n - f` certificates and no idle empty blocks.
6. Complete dedicated independent protocol/side-channel review and run the
   physical-device and authorized release-signing workflows. Each qualification
   claim requires its corresponding artifact-bound evidence.

## Evidence discipline

Normal builds, immutable executables and retained artifacts identify the tested
source. Distinguish a structural resource charge from measured process RSS,
a component proof from a complete credential, and local verification from
finalized network authority. Record failures and unexecuted checks explicitly.
Do not restore retired surfaces, bless changed source hashes, bypass native
provenance or relax proof/resource ceilings to obtain a passing result.
