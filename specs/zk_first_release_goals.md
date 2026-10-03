# ZK first-release completion goals

Set: 2026-09-26. Execution resumed: 2026-09-30. Reviewed: 2026-10-02. Overall status: **Active**.

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

The current `optimizations` checkout includes the resolved Core/SDK merges and
reviewed fixture, private execution and X509 repairs. Fresh normal Rust 1.93.1
compilation emits all twelve selected Core/Kagami/IVM/storage/FASTPQ harnesses.
The genuine host bridge passes ABI-25, loaded-image and all 103 required-symbol
checks. Paired quantity and IVM producers return identical captures, and the
quantity consumer passes. The full selected native census contains 3,271 ordinary
executions; its run is incomplete. Fresh reruns pass the corrected issuer-key
deadline, both sticky-quota regressions, all 109 FASTPQ source-inventory controls,
seven MV publication controls and the Kagami export inventory. Six current mint
failures expose an exact-size bug: production requires the 384-byte transport
maximum instead of the 327-byte canonical envelope. The reviewed model repair is
applied together with recursive-consumer corrections. The remaining 66 control
groups finish with 2,870 passes, five failures and no skips. Three failures are
the known CoreZk fixtures; two private-export fixtures configured a different fee
asset from their signed genesis policy. The fresh rerun passes all 18 available CoreZk controls, both private-export
fixtures and the model mint geometry checks. One production-prover control needs
an explicitly enabled feature. It also exposes an ineffective clamped X25519
entropy mutation, two missing lookup configurations and a provider-validation
allocation from encoding solely to measure length. Reviewed repairs are applied;
the original failed results remain retained. Both complete active/inactive mint
binding controls pass on the retained native artifact. The complete mint relation
then exposes a production capacity failure: 282 SHA blocks require 133,448 rows
per Table8 lane, exceeding 65,527 usable rows at K=16. Limits remain unchanged.
The original-State Musubi source capture and all three remaining semantic table
readers are applied; their first compile exposed 14 missing read-trait
implementations and one obsolete test callback. The reviewed API repair clears
those compiler errors; a subsequent catalog enum-size lint is also repaired.
The next five-crate native run waits for the separately owned merge to finish.
Full-State capture still rejects the
unresolved runtime-verifier schema. All nine cryptographic admission controls
pass, including 44 actual fresh-process valid/malformed checks across all 11
enabled algorithms with zero Rust allocation requests. This does not measure
hidden C allocations or side-channel resistance.

Fresh Linux verifier, evidence and optimized privacy builds pass. Its full X509
namespace records 1,035 passes and one obsolete composite-constructor expectation;
all four original full-domain oracles and all 46 private-dispatch controls pass.
The latter include comparison gas, shifts/rotations and unary/select equations.
A coherent-output diagnostic reproduces missing DER source provenance in seven
RFC output roles. The five-SPKI-channel repair now passes all six native controls,
including the genuine maximum three-certificate fixture, and eight existing key
controls. The composite-constructor correction passes. Nine TBS/CRL/signature
data/length bindings are applied; native tests exposed an incorrect little-endian
assumption, and a reviewed repair restores the existing big-endian channel
contract. All eight variable-output controls and complete source-lookup replay
now pass. All six serial-source controls now pass. The selected disclosure source join
passes all five native controls. Its full RFC namespace records 119 passes, four
stale profile/descriptor/histogram failures and two ignored controls. Original
Name OID census, uniqueness and nonempty structure repairs are now applied; their
native controls, complete value/string policy and refreshed native profile pins
remain open. The public zero-suffix FFT optimization passes native parity and
all 72 relative cost comparisons; large-domain gains remain modest and do not
establish the complete proof's five-minute target. Four private multiply variants
now reuse the exact original arithmetic bank. Their first native run exposes two
obsolete fixture copy extents before arithmetic assertions; the correction is
applied and all 54 private-dispatch controls now pass, including four bit-count
controls. All ten original public multiplication controls and all eight public
bit-count controls pass, including their genuine maximum-segment proofs. The
last run's merge-state guard fails after native completion; its overall failure
is retained. Conditional moves are applied and await native validation.

Unrelated working-tree changes prevent one fixed-source qualification. These
fresh runs are artifact diagnostics with actual Cargo output and immutable native
executables; every source-observation failure and failed native result is retained.
They do not qualify the changing checkout or inherit historical release results.
All ten genuine Kotlin generator modes now produce identical paired outputs.
The authentic canonical public fixture and its Kotlin pin are refreshed, and
all five golden consumers pass. The first managed run completes all 1,756 test
cases without failures or skips, but Gradle fails when a shared output file
disappears before report completion. The isolated successor's Gradle task passes
all 1,766 cases with no failures or skips; its wrapper rejects valid repeated
parameter display names while reconciling reports. Exact multiset reconciliation now admits the 1,766 genuine executions. The
remaining run passes tools (25), Android managed client/wallet (409/52), actual
host-JNI client/wallet (95/1), the tooling distribution and the real 13,741-byte
redemption example proof. Four Android test files required a JUnit 5 import
correction before these passes. All original failures remain retained. All 57 current typed workflow/example
checks also pass as managed workflow coverage.
Exact C/JNI export inventories and SDK
source contracts pass. The latest
Apple retry compiles three real slices and stops on a Cargo.lock change before
publishing a package. Complete Swift packaging, host tests and physical-device
runs remain open. Fresh daemon, CLI and three network harness builds pass, but all
four selected four-validator scenarios fail Torii startup because the alias-index
route's mounted authentication marker disagrees with its required-signature
catalog and handler. The reviewed registration correction and production-mount
regression pass all ten native alias controls. The subsequent daemon build finds
a missing data-model type qualification in the account-removal repair; that
one-line correction is applied. Fresh daemon/harness rebuilding and four-validator
reruns remain required. The last
complete selected privacy candidate is source17, recorded at
`dist/zk-remediation/2026-09-30/epoch19-integrated-source17/source.json`
(SHA-256 `cf8c2dd49d1b315e5040b7f4db0e8daf039ac00607ba01fbef5ecd1e303739b5`).
Stock Rust 1.93.1 production and normal optimized test builds pass on macOS and
Linux. All three authentic profile/IO/projection pins pass, as do all 1,128
ordinary privacy controls. Linux also passes its required full-domain CPU parity
control; the separate macOS focused selection passes 35 controls including CPU
and required Metal parity. The CRL cleanup census and owned-archive fixture
repairs are validated on that candidate.

Its complete Linux maximum proof, self-check, fresh verifier, wrong-genesis and
tampering checks pass. Proof size and enforced address space meet their limits.
Proving takes 2,275.987086 seconds against 300 seconds. The two RSS observations
are below 12 GiB but disagree by 659,456 bytes, causing the existing observer's
consistency gate to fail. Resource qualification remains failed.

The source17 native Core/MV/concread/IVM run executes 1,242 controls: 1,240 pass
and two paid-alias fixtures fail. Both genuine IVM producers yield paired identical
captures; all four original AIR consumers pass. The paid-alias fixture repair,
retained quota-ledger custody, required X509 proof-instance nonce and genuine
Kagemusha TSV correction are now applied as 50 reviewed files. Native validation
of the new combined candidate is pending. Source20 passes stock verifier-only,
evidence and optimized test builds and all four genuine profile/proof/pre-aux
diagnostics. Its complete 1,187-control selection records 1,183 passes and four
X509 fixture failures. The exact historical descriptor restoration and two
independently regenerated scoped DER values are now applied; original failures
remain failures. All 648 macOS CPU cost/parity records pass at one, four and
twenty workers, with pruned transforms faster than the full-transform reference.
Fresh Linux comparison and maximum-proof resource qualification remain required.

Source17 managed SDK results are 1,214 JVM passes and 394 failures, 25 tools
passes, 284 Android passes and one enrollment-fixture failure, and 40 wallet
passes. The tools distribution and all 57 typed workflow/example checks pass.
These results neither supply missing native/generator prerequisites nor qualify
the revised source. The quiet Vega timing attempt refuses the busy host before
running measurements. Genuine fixture production, full integrated controls,
workspace checks, current packages and four-validator tests remain required.
No historical result or partial component review closes ZK03 through ZK08.

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
All 54 private-dispatch controls pass in the retained optimized Linux artifact,
including coherent wrong-gas forgeries, seven unary/select controls, four
multiplication controls and four bit-count controls. POPCNT, CLZ and CTZ reuse
the original zero-prefix equations over canonical source bits with native
six-gas, one-cycle behavior. CMOV/CMOVI now bind the full public condition and
only the native conditionally read source and written destination; source review
passes and native controls are pending. GETGAS binds the destination directly to
the original gas port, preserves its native zero-gas tariff and one-cycle
transition, clears the destination's private tag, and ignores both encoded
source-register bytes. Register zero remains unwritable. The original 21 packet
ports and mandatory history joins remain unchanged. All four native GETGAS
controls, composed degree-four audit and expanded original-port history join
pass; the complete private-dispatch controls remain under validation. Complete
instruction/region semantics and finalized State binding remain open; these
components do not enable IVM proof admission.

The separate callable-lookup component derives frame sizes and exact argument
and result word counts once from the original admitted artifact's complete flat
schemas. Child selection uses the existing canonical fetch columns and binds
the loaded-image target separately from the executable-relative callable entry.
Return selection uses the original active frame generation's protected entry
read, not the return continuation. The same lifecycle packets feed child
descriptor publication, return operands and all 4,097 initialization/copyback
cells without duplicate lifecycle producers. The composed evaluator joins all
8,243 original packets and 54 reserved zero gaps to their actual clocks in one
immutable global history view. It derives every current/next row and fixed phase
from that view; caller-selected row lists and reclocked producer copies are not
accepted. This establishes the complete 8,297-slot window's consistency, not
initialization authority or a complete invocation history outside the window.
Ordinary constructors cannot supply independent callable truth fields. The
composed relation retains the dispatcher's degree-four bound; callable and scan
equations have degree at most three. Admitted compiled returns remain nonempty:
Unit occupies one word, and a zero-result schema is rejected by admission.

The scan retains one funded, move-only 11,012,736-byte allocation and wipes its
private fields before release. Equation output uses reusable 721-field scratch
instead of accumulating scan/history residues. The 66,376 history rows require
log 17 under the existing public diagnostic profile; a second such window exceeds
that profile's unchanged 16,384-slot cap. The internal relation view can borrow
exactly one or two original segment banks with the same per-segment log 13..17
geometry. It uses absolute clocks, global first/last masks and the actual next
segment's lookup state, counters and auxiliary products across the seam. One
borrowed permutation challenge family serves the whole view and every callable
window; no window can substitute a second family. The view retains only fixed
column descriptors and scrubbed row scratch, without copying the backing banks.
This bounded component does not enlarge the public adapter's degree-four,
136-query, 4-MiB, single-segment proof profile. Joint base-commitment/transcript
ownership, aggregate FRI and masking, exhaustive native packet production,
full-invocation sizing and complete proof-memory admission remain open.
Root entry is deliberately absent: the presence of a
private callable does not authorize a public invocation. Recursive typed
traversal, TLV/canonical payload and privacy relations, staged gas and exact
faults, authoritative initialization and terminal publication,
statement/code/finalized-State binding and whole-invocation masking remain open.
This component has no production prover, verifier registration or admission path.

A separate native producer component now owns a fresh ordinary IVM invocation
from the original admitted artifact, selected public entrypoint and initial gas.
Its sole constructor accepts no warmed VM, initial memory/register arrays, host,
raw packet data or caller clocks. Coverage is restricted to empty public arguments,
a Unit result, no child calls or syscalls, and bounded public scalar,
LDI64/LOAD64/STORE64/root JALR execution with at most 64 total instructions
including the return. Packet
backing for all 16,384 private slots is partitioned from
original parent credit before execution. Native commit hooks capture actual
register/control and frame state, successful physical memory/initialization
changes, both staged root and result-validation debits, all 4,097 return-scan
slots and ordinary ZK padding. Unsupported reached operations refuse the local
capture before their effects; failed or unwound attempts publish no owner.
Final-owner cleanup erases payloads and packet metadata before refunding the
original charge; allocation and reservation refusals retain their original
typed ownership.

The native initializer/instruction/history component moves this original owner into a sole
source, projects one scrubbed packet at a time, and streams every original clock
through the existing private history relation with one borrowed challenge family.
It requires exactly the complete 16,384-slot history, including both global
endpoints. All 64 initializer slots, including reserved zeros, have linear
constraints derived from the admitted public entrypoint, its callable schema,
public initial gas and the V1 stack policy. These bind staged root allocation and
frame gas, registers, heap bounds, generation and protected frame/return words;
absolute fetch PC and relative callable entry remain distinct. The initializer
never derives expected values from captured observations or private-callable
presence alone. Its bounded instruction workspace admits public scalar ALU,
comparison, shift, rotate, multiply and bit-count operations, LDI64, public-root
LOAD64/STORE64 and root JALR through
the existing fetch/control/scalar relation. Additional native-source
equations restrict the selected original code words to that subset and require
the original return parent to be zero; constructor refusal is not an AIR premise.
Every compact instruction
captures an actual protected-depth read; the root return captures the original
depth before and after the native pop. All 64 compact windows and the fixed
return window are evaluated. Inactive windows have only zero packets; the former
synthetic running write is removed. All 65 private witness rows are funded before
construction and erased before their final backing refund, while the bounded
public code stays inline. The sole source owns both native packets and derived
workspace, so composed callers cannot substitute another packet bank.

The public scalar subset is exactly ADD/SUB/AND/OR/XOR,
ADDI/ANDI/ORI/XORI, NEG/NOT, SLT/SLTU/SEQ/SNE, signed MIN/MAX,
SLL/SRL/SRA, ROTL/ROTR, ROTL_IMM/ROTR_IMM, MUL/MULH/MULHU/MULHSU
and POPCNT/CLZ/CTZ.
The native producer and constrained consumer share the immutable instruction
operand-shape lookup. Actual source reads occupy the original left/right slots,
including `r0` and aliases; immediate and unary operations never invent a second
register read. One atomic destination packet carries the original value and tag
transition. Native subset equations require public operands independently of
constructor checks; the broader private scalar diagnostic bank does not authorize
general Secret arithmetic. Shared canonical word, ALU, comparison and barrel-shift witness
algorithms populate the existing fully constrained banks without expected-output
inputs. Shift and rotate counts use the low six bits while authenticating the
full original register word; rotate immediates are zero-extended eight-bit code
values and never authorize a register read. SRA preserves the original sign bit.
The full 128-bit product bank constrains the low and unsigned high halves,
including bounded carries and signed-high corrections for the first or both
original operands. Bit counts consume all 64 canonical source bits; CLZ alone
reverses the prefix traversal selected by the immutable artifact word. Zero has
64 leading and trailing zeros. Unused operand ports remain entirely zero.
Comparisons and rotates consume two gas, products three, bit counts six and the
other admitted scalars one; all advance one cycle and four code bytes. The canonical public gas
descriptor supplies witness debit arithmetic; equations bind the actual original
debit. No packet, workspace or allocation geometry grows, and the composed
maximum degree remains four. Conditional moves, GETGAS, division, branching and
fault outcomes remain outside this native source's coverage.

Public-root LOAD64/STORE64 consume the original compact memory and initialization
packets. Checked 64-bit address/end arithmetic and comparisons prove containment
in the immutable public root stack frame or Unit result region; native preflight
acceptance is not a permission premise. The shared scalar payload selection and
initialization OR arithmetic bind the selected half-cell, preserve the other
half, and require public payloads and the original generation. Its 64 additional
422-field witness rows reserve exactly 216,064 bytes before private construction
and erase them before refund. All nine other compact slots are constrained to
zero, completing the 32-slot compact schedule without synthetic events or clock
changes. LOAD64 requires complete containment in the stack frame and all eight
selected initialization bits; the result table remains write-only to guest code.
Both read ports preserve their original values. The original atomic destination
receives the selected half and a public tag, with no manufactured second tag event.
An `r0` destination suppresses only the register write, preserving memory checks
and the three-gas debit. LDI64 resolves its full 16-bit index through the same
immutable prepared artifact's admitted scalar table; pointer entries cannot
supply scalar coefficients or confer pointer provenance. Its one-gas debit and
atomic destination remain in the same dispatcher and history. Neither operation
increases packet or witness geometry. Memory equations have maximum degree three;
the composed instruction relation retains the existing maximum degree four.

The Unit root return consumes its actual public-entrypoint callable, original
operands and every one of the 4,097 initialization-scan cells through the existing
return equations. Its additional 9,308,384-byte workspace is reserved before
private construction and erased before the original allocation is refunded;
scan packets remain solely owned by the native capture. The original NODE and
WORD debits are constrained separately to 1 and 8 gas with bounded subtraction,
and the original typed memory read must contain a public zero Unit word.
The three occupied typed-work slots are consumed directly, with the other return
gaps constrained to zero; the callable-window diagnostic's zero-gap
schedule is not used for this native composition.

The successful terminal bank consumes original return cycles and final Unit gas,
then constrains optional native padding at clocks 10,409 and 10,410. One 96-byte
zeroizing witness is admitted before private construction. Bounded delta bits and
an inverse require padding exactly when the public artifact's cycle horizon has
not been reached; checked limb subtraction charges one gas per padded cycle.
Disabled packets are entirely zero, and all 5,973 remaining packet slots must be
zero. Compact activity is a contiguous prefix of at most 63 non-return
instructions before the mandatory fixed root return. These new equations have
maximum degree two; the composed component retains maximum degree four.
Native gas/cycle summary getters are not expected-output coefficients. Final gas
is derived from the constrained original gas packets; no external output or
statement-authority constructor is added.

This closes bounded initializer, fetch/control/public scalar arithmetic and bit operations, admitted scalar literals,
public-root LOAD64/STORE64, Unit return, compact scheduling, successful padding and
original-source history joins only. The separate diagnostic ordered-register
load bank shares physical payload selection but does not replace the native
atomic destination. General private/wide memory, pointer literals and other
instruction semantics, general typed validation and its staged gas, faults,
terminal statement publication and masked transcript ownership remain open. Native output and history consistency
grant no signed-intent/finalized-State authority or production proof admission.

The focused native capture bridge is runnable with
`python3 scripts/check_ivm_native_frame_equations.py --ivm-test-binary <exact-ivm-libtest-path> --privacy-test-binary <exact-privacy-libtest-path> --output-dir target/<fresh-directory>`.
It runs both native producers and all four exact equation consumers, retains
owner-only captures/logs and records executed binary hashes in a bounded local
receipt. Supplied binary paths do not establish common source/build provenance.
The captures check native frame/call-method equivalence; they do not supply
original instruction-packet ownership or a complete invocation proof.

### ZK04 — FASTPQ

The public Quantity facade uses the masked bounded DEEP producer and independent
verifier. Expected statements derive from canonical authenticated context; proof
consistency alone does not authorize remote spending. Development replay APIs and
required-Metal readiness preserve their existing boundaries.

On historical source `051df111`, the library selection passes 1,312 ordinary controls,
and all 16 offline ordinary controls pass. A required-Metal maximum ordinary proof passes
production, self-verification and the native controls with unchanged source:
973,755 proof bytes and 1,561,083,904 bytes peak RSS. Its native production and
self-verification take 852.192443 seconds under concurrent work; this is not a
quiet timing comparison. The maximum AXT producer also passes on
unchanged source: 1,014,530 artifact bytes, 1,514,307,584 bytes peak RSS and
2,263.745643 seconds including self-verification. Concurrent work and a one-second
public stack sample preclude a quiet timing comparison. Both remain under the
unchanged byte, memory and work ceilings. Separate fresh verifier processes pass
both exact retained artifacts without reproving, with unchanged source and no
failed or skipped controls. Earlier source-drifted maximum runs remain failures
of source qualification.

Two actual-Metal controls on that source also pass scalar parity: all 63 continuation
prefix/body boundary combinations and leaf batches for all eight proof oracle
kinds. A fresh ordinary optimized build with `fastpq-gpu,simd` now passes 50
distinct local hardware controls, including nine required-Metal controls,
NEON state/scratch clearing on return and unwind, independent hash vectors,
exact-root transform parity, staging and failure ownership. This covers the
available M1 Ultra; it does not complete the hardware matrix or establish
side-channel resistance, quiet-host performance or fixed-source qualification.

Historical seeded CPU and required-Metal proofs produce identical 485,219-byte
artifacts and pass positive/negative controls. Historical known-answer controls
and maximum artifacts do not qualify a changed candidate.

[FASTPQ readiness](fastpq_production_readiness.md) owns unchanged proof, payload
and work limits. Dedicated derivations provide conditional classical-ROM and
ideal-QROM bounds under explicit query/attempt assumptions. They do not establish
concrete Keccak security, full zero-knowledge, device/side-channel behavior or
finalized authority. Finalized-source network behavior, broader workload shapes
and current hardware coverage remain open.

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
The expanded run exposed four additional fixture failures; their scoped DER and
historical-profile repairs await native rerun. CPU/accelerator parity and a new
maximum proof remain required.

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
