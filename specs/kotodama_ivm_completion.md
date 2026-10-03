# Kotodama and IVM completion

G1–G8 remain active and incomplete. The table below defines their outcomes;
component implementation and historical test results do not establish release
qualification.

## Goals and dependency order

| Goal | Required outcome | Current work |
| --- | --- | --- |
| G1: language and ABI | Caller-owned argument/result word tables for every compiled function; 64 KiB / 8,192-word limit; initialized-count, ownership, alignment and lifetime checks; full-width integer helpers and structural equality. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G2: authenticated State | One State-owned commitment over every authoritative persisted execution table; bounded inclusion, absence and complete range witnesses; atomic finalized publication and persistence/recovery custody. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G3: private execution | One complete native STARK relation and `IvmExecutionStatementV1`; normalized intent, exact public gas, complete reads/effects, private-trace masking, full typed valcom constraints and native local proving. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G4: anchored AXT | One `AxtAnchoredSpendV1` path binding finalized successful execution and exact transfer occurrence, fresh issuer authorization, atomic durable nonce/budget/effects, complete FASTPQ state transitions. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G5: memory ownership | Allocation-lifetime charges shared once across prepared artifacts, analyses, operations, templates and borrowed/nested VMs; aggregate 64 MiB retention; separate admitted active reservations; zero retention; observable live/borrowed/evicted/peak usage. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G6: automatic acceleration | Standard target-appropriate SIMD/Metal/CUDA, runtime-loaded driver, process owner, per-device/kernel qualification, public cost selection, unchanged-input fallback/quarantine, ten reproducible signed embedded PTX families. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G7: Musubi publication | Daemon-owned authenticated private TLS runner, finalized recovery, software signing, provider coordination/readback, durable journal/clock and filesystem custody, configuration through `iroha_config`. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G8: candidate qualification | Regenerate ABI/schema/gas/SDK fixtures together; qualify one unchanged candidate across mandatory language/proof/state/AXT/memory/hardware/native/SDK/network/release checks. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |

Component owners are compiler/IVM/Core/SDKs for G1; State/Kura/consensus for G2;
the native prover, verifier and transactional execution owner for G3;
AXT/FASTPQ/Core for G4; VM/cache/allocation owners for G5; acceleration/build/CI
for G6; daemon/config/provider services for G7; and release/native/SDK/network
qualification for G8.

Freeze the canonical ABI, numeric semantics, proof statement, state commitment and
AXT shapes first. Language, memory, hardware and Musubi implementations can then
proceed independently. Complete finalized state publication/recovery before
integrating private proofs and AXT. Regenerate and qualify a single candidate
only after these paths are coherent.

## Fixed invariants

- This first release has one ABI V1. Replace unfinished interfaces directly and
  remove retired layouts, aliases, decoders, fixtures and backend registrations;
  no backward compatibility path is permitted.
- `r10/r11` carry argument-table address/count and `r12/r13` carry result-table
  address/capacity. Returns carry result address/exact initialized count in
  `r10/r11`. Caller-owned returned storage outlives the callee.
- Current `Secret<T>` information-flow restrictions remain. General secret
  arithmetic/branching is outside scope. Raw witnesses stay local and never
  enter transactions or Torii requests.
- Hardware changes speed only. Results, errors, gas, state, event order,
  commitments and verification decisions must agree across devices. Test
  entropy plus an identical witness yields identical proof bytes; production
  randomness is fresh.
- “Optimal” means the fastest qualified operation/workload path including
  transfer and launch cost. Selection uses only public workload geometry.
- Cache pressure changes local scheduling or retention, never transaction
  validity or gas. Eviction never releases a charge still owned by a borrower.
- Public ledger replacement is a separate deployment operation. Initial network
  evidence uses disposable networks with at least four validators and mandatory
  signed RS16 DA/RBC.

## Next integration boundaries

G1 callable metadata now carries complete flat preorder type schemas instead of
shallow word roles. Compiler emission and admission bind nested Sum payloads,
exact List capacities and element types, nominal products/errors, String versus
Blob, cursor key kinds and private leaves. Public records retain their existing
schema limits; private callable schemas retain 250,000 nodes, depth 256 and
8,192-word tables, with checked interior allocation sizes. Interpreter loading
prepares immutable traversal layouts once, reserving original-pool backing before
allocation; snapshots share that owner and warm calls do not reanalyze metadata.
Bounded traversal charges before node/word/pointer work, checks complete aggregate
heap footprints and validates only active branches and logical List elements.
Failed validation cannot publish result ownership. Full proof constraints for this
traversal and coordinated ABI/gas/artifact/native/SDK regeneration and qualification
remain open. No shallow-role compatibility decoder is permitted.

G2 has an exhaustive typed inventory of World, State, trigger and durable-history
owners, classifying canonical authority, derived indexes, authenticated history
and local policy. The World accumulator commits canonical World fields and their
schema identities; derived indexes do not become independent authority. The
complete-table catalog checks exact identities and materializers, but an unresolved
Kagemusha runtime authority schema still closes complete inventory admission.
Dependency validation borrows its traversal ancestors without allocating graph
scratch; this preserves exact cycle and non-authority errors. Schema-name encoding
and the remaining capture scratch still need their own allocation custody.
Semantic configuration consumed by execution must be authenticated through agreed
parameters or context. Snapshot JSON hashes and touched-write roots are not
substitutes for a complete State commitment.

The scoped domains capture checks `domains_by_owner` against complete retained
current and predecessor domain images before encoding those same canonical rows.
Its allocation-free scan charges every physical current/undo row and index member
against a local work bound; work exhaustion is a deferral, distinct from corrupted
derived membership. Original native publication identities and the State generation
fence reject overlapping publication. Checking preserves undo entries, including
redundant touches, and never repairs live World. This is a consumed scoped capture
foundation; complete derived-index checking and State/Kura publication remain open.

Scoped account-alias capture likewise retains the original account, alias and
reverse-index readers. It checks exact reverse membership, account existence,
primary labels and the existing raw-PII restriction in current and predecessor
images before encoding those same alias rows. Physical tombstones and account
rows without aliases consume the local work allowance. Exhaustion defers capture;
validation never repairs an inconsistent index or supplies finalized authority.

Scoped account capture retains the original accounts, universal-ID and opaque-ID
readers through encoding. Current and predecessor images require exact inverse
membership, unique universal IDs, and unique opaque members attached to a universal
ID; implicit accounts without either remain valid. The allocation-free check
charges physical rows, undo tombstones and inspected members before work. It
neither invents an AccountId-to-identifier hash relation nor repairs live indexes.
Native identity replacement or State publication invalidates the checked capture.

Scoped NFT and RWA captures check their exact owner/domain and owner/status/frozen
groups, respectively, on both retained native images. Empty buckets, omitted
members and foreign members fail before leaf encoding; absent undo rows still
consume bounded local work. Captures retain all original source/index readers
through encoding and reject native identity changes or overlapping State
publication. These checks do not repair derived state or establish finality.

Scoped escrow capture applies the same exact grouping checks to seller, optional
buyer and status indexes. Buyerless records require no buyer group and cannot
appear in any such group. Current and predecessor memberships are checked through
the original retained readers before canonical rows are encoded; absent undo rows
and buyerless source rows still consume the local work allowance. Snapshot index
restoration rebuilds both canonical images, including optional buyer changes,
so replacing the latest block retains its real predecessor memberships.

The actual table catalog routes escrow and repo-agreement rows through these
retained grouping checks before encoding. Repo initiator, counterparty and
optional custodian groups are exact in both images; snapshot restoration derives
both images, preserving untouched members of changed buckets through rollback.
The former unchecked readers are removed. This does not replace the separate
agreement-admission, complete-State publication or finalized-anchor requirements.

Asset lookup recovery projects all eight derived indexes from both retained
definition/domain/balance images. Owning-domain changes also move untouched
balances in the domain index; holder sets deduplicate balance partitions and
nonzero membership requires at least one nonzero partition in that image.
Both images must satisfy definition and domain references before any index is
replaced. Recovery retains complete predecessor buckets and redundant source
touches without modifying canonical source rows or undo. This is snapshot index
reconstruction, not an authenticated live capture or finalized State root.

The canonical asset-definition reader checks exact owner groups, optional domain
groups and definition-to-domain lookups in both retained native images before
encoding. Referenced domains must exist in the same image, and restricted
definitions must own a domain. All five original readers survive through the
final identity check. Scans and referenced-domain lookups consume bounded local
work without allocating or repairing live indexes. The balance reader separately
checks its five derived indexes, including exact domain projections and holder
membership over all account/definition partitions. Both zero and nonzero holder
checks use the same current or predecessor source image; source undo tombstones
and inspected range rows consume the local work allowance. All eight original
balance/reference/index readers survive through encoding. These scoped checks
do not establish a complete State commitment or finalized anchor.

Contract-alias capture checks the exact inverse lookup and the shared persisted
lease-window relation in both retained native images, retaining both original
readers through encoding. It rejects duplicate targets for one alias, missing or
foreign inverse rows and invalid leases before allocating encoded rows. Physical
rows and undo tombstones consume bounded local work. Expired and undeployed
bindings remain representable until ordinary cleanup; this capture does not
replace invocation-time deployment, authority or expiry checks. Live writes,
restore and capture use the same lease-window primitive. The catalog consumes
this checked capture directly, with no parallel unchecked binding reader.

Account-rekey capture retains the original records, accounts, alias bindings and
occurrence index through encoding. Both native images must have exact nonempty
occurrence buckets for the active and all historical account IDs. The shared
provenance primitive selects only the maximal account-ID-rekey suffix: those
predecessors must be distinct, retired and unambiguous, while alias reassignment
history may refer to live accounts or repeat historical occurrences. Every active
account exists, every existing alias matches its record, and a record may remain
after alias cleanup. Physical rows, undo tombstones, inspected history and the
shared provenance scans consume bounded local work before access; dense histories
may require a larger local capture allowance. The catalog uses this checked
reader directly. Snapshot reconstruction also validates both retained record,
account and alias images before replacing the occurrence index. It preserves the
original source rows and undo, complete predecessor buckets and redundant source
touches. Snapshot reconstruction allocations, complete State publication and
finalized custody remain open.

The complete root and its exact predecessor must travel with prepared State
journals and publish under the same State generation as World, runtime and replay
membership. Recovery must authenticate that owner before exposing State. Scoped
paired tables already bind hash-key inclusion/absence and ordered raw-Norito-key
range commitments to the same canonical value encoding. Their bounded range
verifiers authenticate boundaries and interior rows, but neither these selected
table roots nor supplied composition digests establish a complete finalized State
anchor. Complete publication, node custody, recovery and current/predecessor range
integration remain open. Preserve the distinction between execution-prefix and
finalized-State commitments to avoid a header/root/finality cycle; table membership alone grants
no permission to disclose private rows.

The Kagemusha registry freeze cannot be lifted with a direct
`CanEnactGovernance` check: that permission covers enactment of an approved
referendum. The certified Parliament set now has one canonical initial
signer-policy proposal with an exact empty predecessor, effect preimage and
head compare-and-set. Its due-certificate reducer moves a one-use, fixed-size
authorization through transaction apply, and State rechecks the complete
certificate digest and resulting registry before publication. Unrelated writes
remain rejected; governed release install, activation, retirement and runtime
reload are separate unfinished transitions.

G3 retirement now removes the old four-hash/16-column binding schema, Halo2
IVM registration and key generator, dedicated native STARK binding relation,
Torii preparation/proving jobs, and SDK/CLI callers. The generic proof registry
rejects reserved IVM identities. Focused Core/Torii test builds and regenerated
OpenAPI provenance are still pending on the changing source tree.
The V1 statement model's `IvmCompleteStateRootClaimV1` is intentionally separate
from the current selected-table leaf/subset roots. Its constructor and digest
authenticate nothing on their own; only a Core-owned finalized State/Kura/QC/DA
handle can promote a root claim to an execution anchor. The access, return,
effect and event hashes are public claims until the complete interpreter/STARK
relation constrains their exact canonical contents and Core checks every current
dependency before transactional application.
Retain reserved-name rejection at generic proof-registration and OPEN_VERIFY
boundaries so removing the special binding circuit cannot re-admit it through a
generic digest circuit. The fail-closed admission is not evidence for private execution or finalized
full-state proofs; public preparation needs a new admissible V1 response shape.

G5 production activation still requires funded fallible remaining trace/debug and
shared-host dispatcher allocations; scheduler graph/result/channel/pool allocations;
remaining generic `Memory::Clone` allocations; and fully prepaid
active and nested host execution. The protected return stack is now reserved before
child-call gas, and call-frame bitmap and vector-slot growth is prepared before
table-validation gas. Inactive runtime-template copies no longer duplicate spare
frame capacity. These bounded cuts do not fund all frame and scratch owners.
Private interval storage in State-owned VMs retains the original active pool;
growth reserves replacement capacity while the previous allocation remains live,
template copies partition the existing parent reservation, and warm reset reuses
interval capacity after a fallible preflight. Retention or a reduced pool limit
does not refund live interval storage.
Local diagnostic step/access recorders and the optional initial image now use
one checked parent-funded plan with child partitions; four new and 20 existing
recorder tests pass. Diagnostic owner coverage does not qualify remaining production scratch.
Parent-credit shortage
must not wait while holding parent funding. Configuration changes must retain the
original pool while allocations survive. Prepared-contract cache operations defer
original-pool refund notifications until their store mutex and complete preparation
claim are released, including partial-allocation refusal and unwind, and preserve
enclosing State refund batches. Register logger shells reserve their exact physical
shared owner from the VM's original pool before construction or invocation replacement.
TLS scopes and borrowed loggers retain that credit through final release; cache retention
does not charge the shell twice. Replacement refusal precedes guest-state or log changes,
and invalid detached host custody severs aliases without allocating. Syscall instructions
admit one detached shell before base gas or cycle debit; default-root input preparation
admits it before argument effects. Reserved quote and body callbacks reuse that shell,
checking the same invocation, code, trace epoch and mode before each isolation.
TLS scope retirement
releases its RefCell borrow before final-owner callbacks. Per-cycle register/memory
root rows use fixed backing admitted from the same VM pool before instruction or
padding effects, including a twelve-cycle instruction crossing the configured cycle
limit. Growth retains old and new credit until replacement publication; reset preserves
the original pool binding. Test-only snapshot copies own their independent row credit
without charging it again through the aggregate trace estimate, and defer refunds until
register-log guards are released. Register paths own eight inline digests copied from
one canonical borrowed, exact-size sibling traversal; the owned crypto proof API
collects that same traversal. Dirty and clean register path extraction use the existing
fixed tree backing, with no temporary proof or path allocation. Live register events
carry the fixed path inline, and detached diagnostic verification still rejects short,
long and out-of-range borrowed paths. Register-event rows now retain exact fixed
backing from the original VM pool. Growth admits replacement capacity while the old
allocation remains live; initialized values, tags, paths and roots are erased before
retirement. Root preparation, instructions and syscalls admit public upper bounds
before their effects. Nested scopes partition the existing event quota, host masks
preserve it without emitting, and an instruction retains its quota through delayed
native completion. Independent diagnostic snapshots fund their own copied rows;
retaining or evicting a shared logger never refunds another borrower's live storage.
Delta-trace backing and the remaining trace owners are still open. State dynamic
access prepasses now use the original prepared-cache pool
for VM construction and raw-selector preparation, preserve typed VM resource refusals,
and decline optional hints through the existing conservative access/fence boundary.
Shipping standalone/CLI VMs, state-free overlay helpers and State seed/restore dummy VMs
without an explicit execution budget remain open funding boundaries; they are not
qualified by the funded paths above. Global cache
configuration publishes limits, retention budget, shard geometry and the snapshot
before eviction callbacks; callbacks can reenter ordinary configuration writers
without the outer writer partially overwriting their changes. The configuration
guard remains reentrant and held through callbacks; enclosing non-cache locks
still require their original refund owner. Generic-program verification and cold
runtime/template allocation now use the existing State pool. Generic summary bytecode
reserves its exact fixed backing and shared control from that same pool before
allocation; summary/image clones and evicted borrowers retain the original credit
until both allocations are reclaimed. Local cache eviction
and both State generic outer-cache sites defer refunds until their locks and leases
are released. Generic admission preserves typed allocation refusal through overlay,
executor and trigger owners, including original analysis allocation refusals;
trigger policy reuses the existing prepared cache
instead of creating a separate pool. Executor restore now performs the same static
admission as the VM loader without an eager default VM. Live validation and
migration fund VM images and baselines from their State's existing pool; warmed
variants keep the first pool's identity, while foreign-State clones execute
uncached on their own pool. Executor eviction and lease return defer original-pool
wakes until the pool guard and enclosing same-pool refund scope have released.
Decoded and prepared instruction arrays now reserve their fixed backing and shared
control from the original State pool before decoding/preparation. Same-pool loads
share those owners; diagnostic or foreign-pool arrays are rebuilt under the actual
VM pool before installation. State arrays bypass process-global code-only caches.
Contract admission also funds its temporary instruction buffer, and State contract
preparation funds the shared artifact image. Retained control-flow boundaries and
successor nodes are constructed directly in funded shared arrays without temporary
vectors; every clone retains the original credit through final reclamation.
Literal admission and native preparation share one borrowed descriptor/envelope
validator without directory vectors or payload copies. Native literal values and
pointer-provenance indexes reserve their exact original-pool arrays before
construction, rebind foreign-pool storage before VM installation, and retain credit
through snapshots and final borrowers. Empty indexes have no shared shell. Typed
canonical literal refusals preserve their original local classification; malformed
payloads retain the same deterministic admission failure.
The prepared contract's shared shell and fixed entrypoint index reserve their exact
original-pool layouts before allocation. Name lookup sorts descriptor indices in
place and borrows immutable metadata names instead of cloning strings. Private-input
reachability reuses one fixed queue/visited buffer, marks instructions on enqueue,
and releases scratch before publication. Shell and index charges remain with the
final shared borrower through eviction, pool shrink and unwinding; diagnostic
preparation does not grant a funded State owner.
Stateless instruction counts and syscall classifiers scan validated borrowed
instructions without decoded arrays or aggregate reports. Aggregate syscall analysis
reserves exact occurrence scratch and immutable histogram storage from the State's
original pool before allocation, preserving full-width IDs and sorted counts. Cache
hits and report clones share that histogram; eviction, shrink and unwinding refund
only after its final owner, with notifications deferred through the actual cache
and enclosing State guards. Empty histograms need no shared shell; zero retention
keeps only live analysis owners. Optional static-state dataflow borrows original
entrypoint roots and reserves combined instruction-fact rows and a bounded FIFO
from the actual State pool before traversal. A queued flag coalesces pending
changes while preserving the existing fixed-point merge. State-key analysis borrows
original pointer envelopes and Norito payloads without eager hex tables, retaining
the first symbolic key per state-syscall site through later ambiguous merges.
Sorted, distinct output keys reserve their exact UTF-8 arena, range index and shared
shell together before allocation. Read/write views and clones retain that original
pool charge through the final owner; empty results allocate no shell, and traversal
scratch is released before publication. Local refusal at traversal or key publication
declines exact scheduler hints and keeps conservative fences without changing gas
or transaction validity. Literal text analysis uses the shared Norito nominal-frame
validator and borrows original Name/StatePath bytes. One exact index and audited
ICU/stable-sort scratch demand are admitted together from the State pool before
allocation or NFC validation; normalization retains no replacement string, scratch
ends before publication, and index credit follows its actual lifetime. The shared
model predicate preserves syntax, exact NFC and original nominal identities; owned
binary and JSON decoders charge the same audited normalization demand. Physical
allocation census and owned/view framing parity remain required validation gates.
State artifact-admission consumers preserve local refusal through their existing sticky
transaction or overlay owner. Remaining raw executor/registry byte copies,
metadata and other typed literal decoder storage, admission-policy scratch,
cache indexes and executor-pool control
allocations, and host scratch still need funding.
Core exports
measured resident, reclaimable, borrowed, evicted-but-live and peak
reservation gauges alongside retained, active and unmeasured-owner totals. Composite
retained-owner classification and funding missing active/scratch owners remain open;
these gauges do not represent complete process RSS. The new
pool and buffer APIs are not evidence that those production paths are funded.

The local physical runner is an Apple M1 Ultra with 128 GiB memory. Exact-candidate
M1 Ultra calibration remains unrun. Local required Metal kernel parity passes;
Graviton3, CUDA parity, mixed-hardware
four-validator execution and publication qualification still need their required
runners and service custody.

TODO: Freeze and record one complete candidate, regenerate all native/SDK artifacts,
and replace each open gate with its actual evidence. Physical runners, provenance
signing and deployment credentials are required inputs. Do not infer release
readiness from local components or represent skipped/missing execution as a pass.
