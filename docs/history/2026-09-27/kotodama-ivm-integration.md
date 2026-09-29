# Kotodama / IVM integration evidence — 2026-09-27

## Current checkout and evidence boundary

Since the September 27 checkout instruction, all ongoing work uses only `/Users/takemiyamakoto/soramitsudev/iroha` on `optimizations`. This record preserves earlier candidate results as historical evidence. The prepared changes are being reconciled with the branch; those earlier builds and test counts do not qualify the combined source. Retired binding-only interfaces remain removed and production proof/AXT gates remain closed pending the complete sound replacements.

This records scoped results for the changing integration candidate in
`/tmp/kotodama-v1-patch-preflight`. The accepted [completion goals](../../../specs/kotodama_ivm_completion.md)
remain open. First-release changes target one ABI V1; retired interfaces and
implementations are removed without compatibility paths.

## Core build and reader controls

The September 24 Core test build completed in **12m 30s**, producing
`iroha_core-c61c249a92349c75`. Its canonical-table capture filter ran ten tests:
**8 passed and 2 failed**. The failures were fixture assumptions:

- The 110-reader default-State test assumed every table was empty. Fresh State
  seeds three canonical SNS namespace-policy rows in `world.smart_contract_state`.
  Source now expects and captures all three.
- The aggregate-row-limit test passed the entire descriptor catalog while selecting
  the first three tables. Source now passes `&TABLE_MATERIALIZERS[..3]`, allowing
  the intended resource-bound check to execute.

The September 27 combined build passed in **19m 38s**, producing
`iroha_core-8fc7c21671b38bb7`, `irohad-a154648474931ea2`, and
`ivm-51c79353d70c49e2`. The original ten table-capture controls now pass.
The expanded table selection passed **11/12**: its new maximum-image artifact
fixture failed before admission because the CNTR section did not leave aligned
executable bytes. Source now uses a canonical empty LTLB section with explicit
padding and asserts the admitted code offset; that correction awaits rebuilding.
The large canonical oracle-history fixture passed.

The fresh Core leaf selection passed **12/13**. The remaining failure expected a
now-defined Musubi semantic schema to be unresolved. The fixture now checks that
its empty-table declaration binds the root without granting projection authority.
Streaming-work, cross-table-preimage and all other leaf controls passed. This
corrected selection still needs a rebuilt rerun.

On September 27, additional filters on that same Core executable passed:

| Filter | Result | Scope |
| --- | --- | --- |
| `finalized_source_resolver` | 3/3 passed | Read-only AXT source facts from finalized State/Kura and exact transfer occurrence. |
| `semantic_trigger_reader_rejects_changed_action_and_omitted_row` | 1/1 passed | Scoped semantic trigger capture, including changed and omitted rows. |

The rebuilt finalized-source resolver passes **3/3**. After replacing raw fixture
code with an admitted ABI V1 program, both branch interpreter/relation controls
pass **2/2**, including malformed program counters and unchanged registers/tags.
The native branch STARK control also passes **1/1** in **317.91s**, including
changed-statement and forged-row rejection. This is a bounded unregistered
relation result, not a complete execution proof.

## Allocation-lifetime controls

The September 27 IVM/MV source passed the following focused controls. These are
component results, not complete runtime-memory qualification.

| Filter or control | Result |
| --- | --- |
| IVM `call_frame::tests` | 15/15 passed |
| IVM `execution_memory::tests` | 7/7 passed |
| Funded memory propagates its original pool to root-frame preparation | 1/1 passed |
| IVM `contract_return_stack::tests` | 3/3 passed |
| MV move-only charged buffer drain and unwind | 1/1 passed |

Expected panics in the unwind controls were caught by the tests and did not fail
their results. The frame backing uses the existing charged buffer allocator;
charges survive borrowing and release only with the final backing owner.

## Combined-build component results

The new crypto executable `iroha_crypto-91e9b95ad58f1817` passes all 18
`ordered_range` tests, including the four digest-only controls, empty proof-header
bounds, complete interval adjacency, and the external-store range above the
scoped row limit. The fresh `fastpq_prover-4e485f265aebb9b7` passes
`anchored_axt_claimed_source_api_requires_the_full_proof_bound_occurrence` (1/1).
This verifies exact claimed occurrence binding; it grants no source authority or
spend admission.

The fresh daemon finality selection passes **18/18**, including real executed and
finalized high-water advancement, Kura ahead of State, and missing tip finality.
The local outbox mutation/restart-rollback audit also passes **1/1**.

## Reviewed source awaiting remaining combined tests

The revised large-value capture patch streams complete canonical value encodings
into paired lookup and ordered digest roots. Review corrections now retain ignored
writer-overrun attempts, preserve verified schema/domain context for typed preimage
checks, use an artifact at the actual 1 MiB image ceiling, bound both encoding passes
across all rows, and account for padded Merkle-node payloads. Missing crypto exports
were corrected. Empty-tree proof construction and verification now charge the fixed
proof header, including the corresponding full-value range path. The crypto range,
Core streaming-work, typed preimage and large oracle-history controls pass as
recorded above; the corrected maximum-artifact fixture still needs a rebuilt run.

The returned Musubi local anchor remains informational. No new signer,
outbox-opening, or Queue authority is enabled by the tested change.

The next source wave adds 82 native World readers, for **192/197** declared table
readers, and 13 populated mutation/omission controls. Three semantic Musubi readers
and the two specialized transaction-membership integrations remain absent; full
State-root capture stays closed. Cold runtime-template copies now prepay their
image, leaves/nodes, registers, tracking, private ranges and Core owner from the
original VM allocation pool, with six focused controls awaiting compilation.
An unregistered shift/rotation chip adds seven admitted opcodes and seven authored
tests; its algebraic and native-proof controls have not yet run. These edits and
two warning cleanups postdate the successful combined build.

## Follow-up integration on the same day

A second combined six-crate test build reached Core but failed on fixture imports:
`Metadata` belonged to `iroha_model_base`, the Musubi checkpoint fixture needed
Core's revision type, and issuer fixtures needed `Registrable`. The corresponding
inference error was downstream of that missing trait. All three imports are now
corrected; this failed build is not a test pass.

After fixing the cold-template test's mutable borrow before wrapping its owner in
`Arc`, the IVM-only build completed in 13.36 seconds. All **30/30** memory/reuse
controls pass in 22.64 seconds, including the three new funded-template controls.
The rounded HashSet clone-capacity control separately passes **1/1**. These used
`ivm-042c38d9971de01a`. Thirty unchanged test bodies now live in a cohesive internal
memory/reuse test module; eight FASTPQ anchored-source tests likewise move to a
separate module. The IVM and Core host source limits are ratcheted downward, with
no increase to an allowance.

The integrated current-issuer helper checks both signatures against one current
StateTransaction view. It refuses pending authorization transitions from the
current transaction or earlier accepted block transactions, preventing identical
republication from reviving an old generation before finalization. Its nine
freshness controls still await the corrected Core test build. It returns no spend
authority, and the production remote-spend gate remains closed.

A reviewed nested-return transport now reserves the public gas/ABI upper bound
plus the complete TLV envelope from the original State pool before child checkout.
Exact byte backing consumes that credit after execution and survives through the
guest copy. Eight authored transport/rollback controls await compilation; typed
return-record construction, journals and DefaultHost checkpoints remain unfunded.

Reviewed Musubi validation changes borrow and stream the exact attestation-set
preimage, count UTF-8 chunker handle bytes without formatting, and scan exact
version comparators without a set allocation. Seven authored boundary/equivalence
controls and the existing captured-schema controls await compilation. The three
semantic State readers remain absent: crypto caches/backend scratch, borrowed
cross-table validation and traversal admission still need actual lifetime owners.

The codec guard, compiler fixture inventory (302 fixtures / 591 tests), historical
archive verification (64,736 records / 67,311 occurrences), formatting and diff
whitespace checks passed before these last source additions. They require the
appropriate final rerun. The repository-wide source-size check found 279 issues;
249 were in files byte-identical to the preserved main worktree, while 30 were in
changed candidate files. The IVM/FASTPQ/Core-host extractions address their scoped
items; the broader check has not passed and its baseline was not broadly relaxed.

## Seven-crate rebuilt candidate

The subsequent locked, offline seven-crate library test build passes in **20m18s**:
`cargo test --locked --offline -j2 -p iroha_data_model -p iroha_crypto -p ivm -p iroha_core -p irohad -p fastpq_prover -p mv --lib --no-run`.
The local unsigned source inventory contains 21,958 paths, digest
`59ff5575155359e82b9b6d78b26c3b1e2d38ad92743d872c805e01077b2dbd32`.
This is component evidence, not signed release provenance.

Fresh Core `iroha_core-9bba4c0faf4be2d7` passes leaf **13/13**, table capture
**25/25**, issuer freshness **9/9**, cold-template/final-owner **2/2**, and nested
return transport/refusal/rollback **8/8**. The stale schema expectation, aligned
maximum-artifact fixture and missing fixture imports are resolved. Fresh IVM memory/reuse
**30/30** and rounded-template tracking **1/1** also pass. The catalog
remains 192/197; complete State-root publication and spend admission remain closed.

The fresh model binary passes the bounded Musubi validation group **7/7** and its
captured schema **1/1**. The refreshed FASTPQ binary passes all **8/8** anchored
source controls after the cohesive test-module extraction. These include the exact
proof-bound occurrence and complete public-set substitution checks.

The unregistered scalar segment composes canonical admitted-code fetch, all 256
public registers, shared ALU/branch/shift equations, full-width gas/cycle counters,
artifact cycle limits, and frozen padding across 1–64 completed steps. Its bounded
profile has 1,277 base columns and degree four, with a maximum encoded proof of
3,976,192 bytes. All **30/30** interpreter/relation controls pass (851.70 seconds).
Both fresh native proofs pass: shift/sign-fill with changed statement/stage rejection
(298.03 seconds), and the composed scalar segment with changed boundaries/artifact
rejection (424.84 seconds). Memory, calls, syscalls, private traces, complete invocation and
production verifier admission are outside this component.

Formatting, codec, source-fixture (302/591) and whitespace checks pass on this wave.
The source-size guard now reports **276** findings: 249 finding files are unchanged
from the preserved main seal and 27 differ. Its broader gate is still open; the
baseline has not been relaxed. Queued, independently reviewed patches are not part
of this build or these test results.

## Allocation lifetime and scalar extension integration

The tested preceding wave was copied into the implementation worktree only after
all 966 guarded paths still matched its original source seal. All 361 differing
paths were copied exactly, and the guard baseline was refreshed. Unrelated working
tree changes were preserved; no commit or release was created.

The next candidate replaces native artifact preparation's lossy error formatting
with typed local deferral, including the original pool's release observation. IVM
shared slices/metadata and ordered digest-tree owners now consume every private
strong reference through `Arc::into_inner`: original Arc backing is freed before
payload destruction may refund its metadata charge. Exact ordered digest row/key
backings use the existing State execution pool, retain charges through final clones,
and preserve typed refusal through Core. Ordered Merkle levels, lookup nodes,
proof buffers, Core staging and remaining active allocations still need funding.
Independent review caught and corrected the premature Arc refund and a fixture
borrow, and required explicit allocator-test registration. The actual-source IVM
strong-owner concurrency unit passes in isolation; the integrated actual-allocator
controls now pass **5/5** (three IVM owner cases and two ordered-digest cases).

Musubi validation now borrows bounded attestation records and provider evidence
instead of cloning full records. One ordered location cursor includes retired rows
and checks separate populated archive groups with an empty group between them.
Transaction membership exposes a borrowed current/rollback cut held by the original
writer, with visitation admitted before callbacks. No semantic Musubi or membership
reader is registered as complete State authority by these prerequisites.

The unregistered public scalar relation adds comparisons, MIN/MAX, NEG/NOT, GETGAS,
pure direct jumps, CMOV/CMOVI and POPCNT/CLZ/CTZ. Bit counts reuse the source-bit
constraints with 64 prefix columns; the profile is now 1,341 base columns and 2,637
profile constraints, degree four, with an encoded bound of 4,119,552 bytes under
the unchanged 4 MiB cap. Link-register calls, memory, syscalls, private traces and
complete invocation remain outside it. Independent source/algebra review passes;
the actual interpreter/relation suite passes 55/55. All five composed native
proof controls pass on this source wave.

The locked offline build includes eight library targets and the separately
registered `shared_owner_custody` and `ordered_digest_allocation_custody` integration
targets. It passes in **22m23s**. Its unsigned local inventory has 21,980 paths, digest
`7a959273dec0673edb4e5eb85c19109e0e302f64dd58776260e44869df2ff20b`.
Fresh Core `iroha_core-10f580156a521888` passes **63/63** focused tests: borrowed
attestations 7, current-provider lookup 4, provider set 1, live location cuts 3,
writer-owned current/rollback membership 8, canonical leaves 14, table readers 25,
and original-State-pool refusal/release 1. Fresh crypto ordered-range tests pass
**25/25**. Typed artifact errors pass **3/3**, the actual cold native preparation
and release-owner tests **2/2**, and IVM cache-memory tests **30/30**. The first
artifact filter matched zero tests; the runner detected this and the corrected
`contract_artifact::preparation_deferral_tests` filter ran both intended tests.
The expanded scalar interpreter/relation group passes **55/55** in 1,677.43 seconds.
All five composed native proof controls pass. The actual allocator integration
targets pass **5/5**. The full IVM library passes **984/984**, with zero failures or
skips, in 1,977.15 seconds. An end-of-run comparison confirms all 7,567 sealed
Rust/manifests still match, including already retired paths represented by null.
Only four documentation paths changed during these checks.

Formatting, codec, source-fixture (302/591) and whitespace checks pass. Strict
crypto Clippy fails three scoped findings: a Debug formatter omits private owner
credit, and two functions exceed the function-size limit. Reviewed isolated
corrections are queued; the candidate under test remains unchanged. The separate
IVM/artifact-admission lint run stops at 28 admission findings: 27 oversized error
returns and one redundant closure. A separately reviewed compact error preserves
exact ABI hashes and allocation-free original-pool deferral conversion; combined
qualification is pending. The source-size guard remains at 276 findings without broad baseline relaxation. No preceding-wave proof or test count
is claimed for this changed candidate.

## Wave 7: funded capture owners and integer machine relations

Fifteen independently sealed patches are integrated for a new qualification run.
They fund ordered levels/shared owners, paired staging, resident lookup nodes,
table-capture owners, runtime-template ownership, and Musubi live/universal
validation scratch through the original execution pool. State constructors and
startup restore/replay now carry that pool explicitly; local resource refusals
remain distinct from invalid state. Snapshot fallback refuses to reset locally
deferred execution or history admission. Compact artifact errors preserve exact
ABI hashes and original allocation-refusal custody without allocating an error box.

The unregistered public scalar relation now also constrains full-width machine
multiply, divide and remainder, including arithmetic faults and attempted
out-of-gas terminal behavior. Shared source bits reduce the base trace to 1,305
columns; the profile has 2,819 constraints, degree four and a 4,038,912-byte encoded
bound under the unchanged 4 MiB limit. These are incomplete public machine
relations, not private invocation or complete IVM execution proofs. The previous
wave's test counts do not qualify the changed implementation.

The reviewed composition digest is
`18aaa0b27e4f1f15e8ca9c13413e1bd5d6f637e9d665fffb980c337753050664`.
Root integration adds explicit module paths required by existing path attributes,
extracts four universal-validation test bodies without dropping assertions, and
removes two unnecessary mutable fixture bindings. The first locked build then
found two former Arc-specific node borrows; they now use the charged owner's
ordinary Deref. No compatibility adapter was added. The source-size baseline for
`ivm.rs` ratchets downward from 9,445 to 9,414 lines, with no broad relaxation.

The second build reached Core and failed four test-only function-pointer casts
under the workspace's denied trivial-cast lint. A typed cases array replaces all
four casts. The borrowed lifecycle predicate now returns the original World row
without cloning retired provider lists; three new controls preserve identity and
error order.

Review also found that newly funded Musubi scratch could notify execution-pool
waiters while direct commit or a retained carrier still held State writers.
An original-pool refund batch now retains only notifications across synchronous
validation scopes; credits become reusable immediately. State owns that batch
until all physical writers retire or journal capture completes. Direct commit
also defers the execution pool alongside the history pool. Independent source
review found no blocker; six MV controls and two actual State/Musubi callback
controls subsequently pass below. This does not close general active
allocation ownership.

The third build reported three E0596 errors in the new lifecycle tests: root had
incorrectly requested removal of mutable World bindings required by their direct
storage inserts. After Cargo exited, all three bindings were restored to the
originally reviewed lifecycle patch; production code was unchanged. No other
errors were reported in that attempt.

The fourth build compiled Core and Torii but found a test-only E0507 in the
daemon snapshot classification control. The test now clones its borrowed
ReleaseWait before consuming the wait, preserving the original pool identity.
The fifth build's unsigned local inventory has 22,019 paths, digest
`f4bb531680a496ecd280abb66617022cc9373eb1c6aebad5cc90b33e5d464ff1`.
The locked offline build includes eight libraries, the daemon binary and five
separately registered physical-allocation test targets; it passed in 4m 22s.
Formatting, codec, source-fixture (302/591) and whitespace checks pass before this
build. Earlier scoped Clippy corrections are integrated but not yet qualified. The
source-size guard still has 276 findings. No new production proof, State-root,
AXT or Musubi publication gate is open.

The first runtime pass has 67/67 ordered-state crypto controls, 37/37 artifact
and cache ownership controls, 22/22 physical-allocation controls, 30/30 MV
allocation controls and 11/11 MV doctests passing. The six new refund-batch
controls and actual State direct-commit callback controls pass. The snapshot
retry classifier passes 2/2 after correcting the runner to target the daemon
library; the binary target contained no matching tests and was not counted.

Core's selected groups expose eight failures: five new Musubi controls expect
unqualified fields although the existing error owner returns `world.{field}`;
two archive fixtures populate Kura before authenticating its physical geometry;
and the staged-snapshot hash differs from actual commit. The broader daemon
snapshot group passes 15/17, with two fixtures assuming a missing default Kura
baseline or mutable configured pre-genesis catalog. These failures remain open
until corrected and rerun. No production predicate is weakened to fit a fixture.
Dependency-inclusive strict Clippy stops at 43 Concread findings; direct-crate
Clippy with `--no-deps` is checked separately and cannot qualify that wider gate.
Direct MV checking reports 16 findings, and a separate direct crypto/IVM/admission
run reaches two IVM call-frame simplifications. Mechanical type aliases and
documented inline ownership expectations are under review; resource-pressure
errors must not acquire an allocation merely to silence a size lint. Full IVM
and expensive AIR/native proof runs await these corrections.

All 7,606 Rust and Cargo manifest/lock paths still match the fifth build's source
inventory after those runtime groups. The staged snapshot failure is an actual
serialization omission: only `WorldBlockFields` skips the persisted
`domain_endorsements_by_domain` append-order index. The reviewed correction
removes that skip, leaves the existing hash assertion intact and adds complete
field-inventory plus nonempty current/predecessor byte/hash parity controls.

The next isolated composition also contains the reviewed original-owner
membership pair and closed grouped catalog, required Musubi source-work
admission, and typed packet-bus relation. Grouped membership accounts for
194/197 table identities while retaining its frontier and original source with
both roots; three semantic Musubi readers and complete State ownership remain
absent. Musubi geometry admission precedes unchanged semantic predicates;
parser/error/signature allocations and retained crypto caches remain open.
The packet bus has 262 base and 134 total auxiliary columns, degree four and
an exact 2,472,320-byte maximum native envelope at its component geometry. It
has independent equation/source reviews, but only an external-field/adapter
stubbed source harness has run. Its real native tests are pending, its capacity
does not cover a complete admitted machine image, and it grants no request,
initialization, frame, host, gas, fault or terminal authority.

## Open gates

No result here establishes a complete authenticated State root, complete native
STARK execution/private invocation, production anchored AXT admission, or production
Musubi publication. The production gates remain closed. Complete resource ownership,
exact-source native/SDK fixtures, full workspace/strict checks, required physical
hardware and gas calibration, signed PTX provenance, mixed-hardware four-validator
DA/RBC execution, and publication/recovery/soak qualification remain required.
Subsequent source changes require new evidence; earlier binaries and focused passes
do not qualify an unchanged release candidate.
