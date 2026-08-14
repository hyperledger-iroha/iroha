# Sumeragi lifecycle simplification

This source-coupled record freezes the baseline for replacing the accumulated
Serve, witness, latch, and producer-episode scheduling paths with one
deterministic lifecycle coordinator. It is not proof-ledger promotion evidence
and must not be refreshed merely because the replacement changes a sealed
source.

Completion of this architectural replacement is explicitly outside the current
`ML-*` Production Multilane Finish scope; see the closure ledger's
[TODO classification](sumeragi_v2_multilane_closure_ledger.md#explicitly-out-of-scope).
The production runner now gives every height to the sealed lifecycle
coordinator/ledger stack. `PendingKuraApply` uses a dedicated no-clock state
whose verified successor and lifecycle-storage authority enter the ordinary
lifecycle loop. This scope statement
advances no ledger row or gate and does not exempt the live source from ordinary
build and test coverage.

The current production launch seam consumes the leader-wire store only through
a distinct one-shot authority minted by the already-open `SafetyWal`; adapter
recovery does the same for serviced-candidate state before WAL replay. Those
stores and WAL append retain an opened post-open directory/leaf identity and
perform bounded descriptor-relative I/O. The lifecycle runner now mints the WAL
directory owner from the opened Kura-root capability before adapter recovery.
Non-Unix basic WAL I/O
retains its legacy path fallback, but adjacent-store authority minting fails
closed until an equivalent handle-relative implementation exists.

## Frozen starting tree

The baseline was captured at `2026-08-09T23:10:45+09:00`, before the new
coordinator source and module declaration were added.

- Branch: `optimizations`, two commits ahead of and four commits behind
  `origin/optimizations`.
- `HEAD`: `01dd293010e94f39055eda190d38396e6d561a3e`.
- Active merge `MERGE_HEAD`:
  `a1da64c76e8ce460bd3ea277814c2ee7fcd9940c`.
- Index: 288 changed paths, 77,257 added lines, and 78,752 deleted lines. The
  binary cached diff SHA-256 is
  `0a3a233e9c267242349089807077de294a24d10f1da225d7d0833b054b20561a`.
- Unstaged pre-refactor worktree: 16 changed paths, 1,189 added lines, and 394
  deleted lines. Its binary diff SHA-256 is
  `6ed97133261113eb4b7a6e446adadf4ba0375b0caccac6bc32b401015b3e7c53`.
  This digest is reproducible from the current tree only by excluding the new
  `v2_lifecycle_coordinator.rs`, its `mod.rs` declaration, and this baseline
  record.
- Untracked paths before the refactor: zero.
- `Cargo.lock` SHA-256:
  `e6da0ca17c73b77367806a208fefe8fc1301342a3245e29a2de4586a5dd86bf3`.
  The file must remain byte-identical.
- `formal/sumeragi_v2/proof_coverage.json` SHA-256:
  `b82d79ae26d17f4fda865d2492526543833c67ecc35eba8544501748c448f161`.
  Its frozen status is 35 `tlaps_proved`, 12 `specified_unproved`, 6
  `trusted_contract`, 1 `out_of_scope`, no promoted cross-tool rows, and
  `machine_checked_completion: false`.

The active merge, complete index, unstaged late-passive-Fetch repair, and every
other concurrent change are retained in place. No reset, checkout, stash, or
bulk replacement is authorized.

At capture time no Cargo or rustc process was active. An existing
TLAPS/Isabelle history-extension proof and a SANY process were active against
the sibling `iroha-sumeragi-v2-production-ready-20260801` checkout. Coordinator
work must not overlap those jobs with Cargo, rustc, TLC, TLAPS, Verus, or an
aggregate checker, and those processes must not be interrupted.

## Replacement-corridor baseline

The production line count ends immediately before each file's top-level test
module. The token count uses the proof ledger checker's `rust_code_tokens`
lexer over the same prefix. This deliberately broad corridor covers the state
and scheduling owners that the coordinator is intended to replace.

| Source | Production lines | Rust tokens | SHA-256 |
|---|---:|---:|---|
| `crates/iroha_core/src/sumeragi/v2.rs` | 12,130 | 64,851 | `54637183d82dcce98fa4e6831ab1d7b68bba7ca5e479a27098bec045ec927e76` |
| `crates/iroha_core/src/sumeragi/v2_effects.rs` | 11,901 | 62,675 | `04014f40e4278b54ba49ea98dbed74471b9f933fc36c58849c8879edf5f8464a` |
| `crates/iroha_core/src/sumeragi/v2_runtime.rs` | 17,726 | 87,393 | `69e83bac3128a68a95226586a9abaddf2e004f2469036e479fadd4068ce7f342` |
| `crates/iroha_core/src/sumeragi/v2_worker.rs` | 18,537 | 94,468 | `85077659c0b2ed52852b41dece874d7e448c11d7044aad75e4ba6903d5ad8422` |
| `crates/iroha_core/src/sumeragi/v2_runner.rs` | 5,580 | 26,951 | `89a8edfa92d879c5bdf3dd3a11a66e0dd621afb15abaf9d9dfd1267fc21b4022` |
| `crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs` | 1,716 | 8,170 | `686437e9a28e62e055044c7e8068f672dace83cd7686792626aa5b099b04afc2` |
| **Total** | **67,590** | **344,508** | — |

The final reviewed replacement corridor may contain at most 206,704 tokens,
which is the integral 60% ceiling of this baseline. Production additions and
deletions must also be classified from this same frozen corridor so the final
ratio demonstrates at least two deleted production lines for every production
line added. Test lines do not enter either ratio.

## Provisional source seals

The exact-Serve checker already expects the worker's actual test context:
`#[cfg(test)] pub(super) mod tests`. A focused authenticated include-closure
probe found no mismatch, so no speculative checker change was made.

Three same-round closure digests are frozen individually. They currently match
their authenticated recursive source closures but become provisional as soon
as the coordinator changes the corresponding sources:

- `v2_effects.rs`:
  `65209ef560de0410ef8ac009bfbfb3e549afacba1809469e96c53926d1535c07`.
- `v2_runner.rs`:
  `74da559229ff1ffb382a5ac0f36362aa1bc2d37606ec888ac917450338786a48`.
- `v2_worker.rs`:
  `d91a31969aa397dd1840e635a29f2579e0d9d9e3297c3f1ce07256bc33acb6b9`.

Their protected properties are Decision-over-Prepare and terminal rebind in
the effect path, reconciliation after the serialized runner transition, and
the worker's non-consuming held-offset projection. Their existing negative
mutations remain regression oracles. Each property and mutation must move into
one of the four final authoritative contracts—state schema, transition table,
selector/rank, or persistence/recovery—before its old seal is removed. These
digests must not be refreshed in place.

## Frozen replacement decisions

The pure reducer keeps one sole-writer authority module below the 1,500-line
production ceiling. Immutable schema/value types live in the sibling
`v2_lifecycle_schema.rs`; this split does not create a second transition
authority. The sealed, state-free production planner-input authentication lives
in `v2_lifecycle_scheduler_inputs.rs`; it can mint no logical state or runtime
work and hands its opaque result directly back to the coordinator.
Capacity-denied work is a frozen pre-admission fence and therefore
does not allocate an ordinal. Its complete semantic request is retained, the
named capacity generation is the only unlock, and an unlocked fence is either
admitted, replaced by a new fence, or retired on conclusive rejection.

`LifecycleKey` remains the specified six-field key. Its phase is a closed
statement-kind discriminator: Broadcast Proposal, Prepare/Commit Vote,
Prepare/Commit QC, TimeoutVote, and TC are distinct phases, as are Proposal,
Vote, and Timeout equivocation and invalid-body diagnostics. A diagnostic
subject is a domain-separated digest over its kind, offender, and canonical
authenticated conflicting pair. Routes, signatures, aggregate carriers, and
full envelopes remain physical-only evidence.

The adapter carriers themselves retain every fact needed to derive those
keys. `EnterView` carries the complete authenticated protected PrepareQC, not
only its round and subject: proposal round, phase, and execution commitment
remain available after a timeout-certificate transition. Its logical identity
normalizes interchangeable signer/aggregate carriers, while its exact physical
identity retains the full TC and QC bytes. Equivocation reports likewise carry
one closed Proposal, Vote, or TimeoutVote pair. Offender, round, and diagnostic
kind are derived from that pair; the logical diagnostic digest canonicalizes
the two unsigned statements, and physical identity retains both complete
signed artifacts in observation order. Only the cryptographically authenticated
adapter boundary can mint the sealed pair; the executor independently rechecks
its structural conflict against the frozen height authority before reporting
it, and no sibling module can manufacture a replacement pair from raw wire
values.

Every scheduler episode must carry the coordinator's exact frozen capacity
geometry, and every physical slot belongs to the admitted work class's typed
capacity lane. Duplicate physical digests retain the lowest canonical slot,
while every supplied slot is consumed once independent of input order.
Admission constructors are sealed inside the coordinator module. The exhaustive
production adapter classifier is the only authority allowed to mint the logical
phase, stage, and immutable scheduling topology. The logical target is not a
seventh caller-supplied identity field: it is the key's authenticated subject,
or the verified height-context digest for subjectless work, so ledger recovery
reconstructs it without trusting an extra value. Admission and recovery
inputs never carry an episode universe: the coordinator derives context, leader,
roster slots, view, subject, phase, and capacity from a separately sealed,
verified-height authority, and rollover replaces that authority atomically. A
separate authenticated planner snapshot supplies live rank debts.

The capacity geometry is likewise sealed. Sign, Fetch, Store, Validate, and
Apply all charge the existing `effect_work_capacity` pool; splitting Sign into
a second coordinator pool would overbook the executor. Broadcast, EnterView,
and both diagnostic classes charge the two retained reducer-effect batches,
bounded by `2 * MAX_EFFECTS_PER_STEP`. Certified Serve uses the existing checked
roster-plus-reply-source family bound, and ProducerTurn has the same one-to-one
capacity. Production factories derive these limits from the verified context,
shared `SumeragiV2Config`, and network reply-source bound; callers cannot supply
raw geometry. Certified Fetch retains its existing secondary request gate.

Capacity waits belong only to bounded pre-admission fences. A claimed record
may block only on an explicit external or recovery generation; letting it wait
on its own reserved capacity would form a local lasso. Advancing an external
or recovery generation atomically wakes every stale record sharing that
source. The closed work-shape table binds each work class and specialized phase
to its exact permitted execution-stage kind and is shared by admission and
persistence. The rank's `source` component is the numeric ingress-source
service position, not reducer provenance. Causal authority remains
authenticated by the sealed owner/effect binding across persistence, signing,
and retransmission continuations. The eight natural-number rank components are
named in their mandated lexicographic order so they cannot be permuted or
truncated by an adapter. For every `plan_turn`, the authenticated runtime
snapshot supplies only identity-bound mode/capacity/selector/lane/source/runner
debts. The coordinator derives the remaining-stage topology and exact live
members of the target's frozen predecessor cut, rejects missing or extra ready
rows, and binds the resulting rank to the returned lease. No rank snapshot is
an immutable record or ledger field; external generation advancement may
legitimately change a later snapshot.

The raw planner input has no raw production constructor, `Default`, or `Clone`.
Its test-only mint rejects duplicate ready ordinals, duplicate generation
sources, and local Capacity/ProducerTurn generation claims instead of silently
coalescing them in a map. Planning computes prospective external/recovery
wakeups first, validates the complete identity-bound ready census, and only
then publishes those generations. A malformed or stale census therefore
cannot make waiting work ready before the coordinator fails closed. Production
Ready rows for Validate additionally carry a registry-derived, exact-address
attestation of `ExecuteBody`, `ValidatedCompletion`, or
`RejectedCompletion`. Missing Validate attestations and attestations on any
other work class fail the whole census. Only the canonical rejected completion
demands one extra Consensus output slot. If that slot is full, the record stays
Ready and unclaimed and contributes the current Consensus capacity generation
to `TurnPlan::Waiting`; capacity-blocked rows are omitted from that call's
selectable predecessor set so a later row able to release the slot can run.
When feasible, the reservation is bound privately to the claimed lease, and
all concurrent admission counts it as used capacity.

The production owner now has one private planner-input factory for the
independently complete direct-registry subset. It derives the entire Ready
census from coordinator indexes, reattests each exact Validate address through
the concrete registry, and accepts closed validated or rejected completion
carriers plus the exact body-receipt-bound recovered Decision Apply carrier.
Recovered PhaseVote and standalone Proposal/Timeout Sign carriers use a
separate dispatch-only transaction. The owner rejoins its exact launched body
store and executor/output guard, reauthenticates the one current Ready row, and
for PhaseVote also rechecks the unchanged terminal Validate parent and typed
Validate-to-Sign continuation. It then reserves one class-sensitive Consensus
queue position before claiming the row. Any post-claim projection failure must
restore the unpublished claim or fail-stop; no durable or service mutation has
occurred, so restart reconstructs the Ready carrier.

The worker task retains the complete event tag/request and any future Prepare
body marker; the guarded result additionally retains the exact Proposal body
payload when required and the signature. None has a parts or payload accessor.
Generic completion draining parks this family without acknowledgement. The
owner-only extraction removes the exact completion-position accounting entry
but keeps the dedicated command index `CompletionPending`, and dropping the
opaque guard closes output for restart. Settlement is intentionally absent
until each Signed successor family has a WAL-ahead, restart-closed transaction.
The Apply attestation exposes only its typed bounded-I/O demand and an opaque
exact-position dispatch key. It is rejected before row construction or lease
mutation until the worker capacity cut is present. Validate completion debts
are derived internally from a sealed
`DirectRegistryCompletion` origin: these carriers have no I/O admission,
fair-ingress occurrence, selector episode, lane/source position, or outer
runner reach. No caller supplies the resulting zero components, and the raw
snapshot remains test-only. Execute-body Validate is explicitly rejected
because it still needs an I/O-capacity and runner observation. Every Ready
class without one of those closed registry attestations is likewise a typed,
mutation-free rejection. An empty authenticated census plans `Idle`.

The reservation is transient and never enters LifecycleLedgerV1. Dropping a
consumer or failing before the typed commit leaves it visible on the active
lease. Generic settlement rejects such a lease fail-closed: the future sole
rejected-Validate transaction must either consume the reservation into its
one invalid-body report without an intermediate generation change, or release
it on a typed non-report branch and advance Consensus generation exactly once.
The guarded production-owner certified-Fetch method consumes the prepared
fair-ingress selector into the same composite transaction that owns its exact
lane/source positions. The reified move-only Completion/Runtime/Ingress cursor
preserves the runner iteration sequence and supplies its exact reach debt; it
is not an independent planner mint. The production lifecycle runner calls this
transaction only from the live current-Ingress cursor, so an arbitrarily
retained snapshot cannot be replayed as current authority.

The executor separately mints a move-only, context-bound mode observation:
its debt is one until the typed Kura application completion is retained and
zero afterwards. It is derived from `finality_completion`, not from a looser
`ready_to_finish` or queue-empty predicate. The runner likewise mints only a
context-bound observation of its live Completion/Runtime/Ingress cursor for a
closed target turn. The certified-Fetch owner rechecks the supplied mode
observation against the borrowed live executor before reserving or planning.
Runner freshness remains tied to the pending current-turn call boundary.

The body store is an equally strict pending boundary. The unified factory
removes its production root reopen and accepts only a freshly opened,
move-only `QuarantinedV2BodyStore` beside an adapter-bound execution/storage
seal. Minting that cut rejects markers already promoted, rejected, or retired.
Authenticated height recovery first mints a move-only storage authority that
binds the exact live Kura instance, verified context, context-addressed
lifecycle/body roots, genesis-or-rotating signature policy, and the universal
genesis account derived from the same authenticated genesis key. The production
owner factory consumes that authority, checks exact State/Kura Arc and network
identity plus its private authenticated-startup marker, and accepts none of
those components as independent inputs. It constructs one `V2ApplyService`;
the quarantine's sole consuming operation filters markers by recovered-
finality subject then authenticated WAL authority, semantically revalidates,
and seals them. The owner retains that exact service for live Apply. The sealed
owner launch then transfers that exact store and service
through the serialized runtime and executor into
`ProductionV2Services::start_with_apply_service` under a parent-sealed move-only
permit. Staged-genesis verification seals the exact
signed body into a move-only recovery token; only that optional token can
install the body into the executor before the worker exists. The transition then
retains only its comparison seal. Construction and every consuming validation
are covered by one fail-stop operation, armed before the owner can be rejected.
Before that consuming handoff, the authenticated startup retains the fixed
validation-marker frontier internally and exposes in production only the
one-shot WAL-bound authority that consumes the producer ordinal high-water,
typed producer terminals, and leader-wire recovery cut into durable preflight.
It exposes neither the adapter nor the replay-effect vector.
The non-Pending runner consumes this launched stack as its sole adapter,
executor, service, ingress, and local-Proposal owner; the isolated PendingKura
height consumes the same launched ownership through a narrower no-clock state
with no Proposal or pacemaker authority. Completion-observer and
status publication are not constructor side effects. A verified live successor retains
the State-owned Kura identity used during construction and can consume itself
into its exact context, activation, and one rotating-policy lifecycle-storage
authority. Foreign Kura projection fails closed, and the runner consumes this
authority at each lifecycle successor boundary. Before activation, a private
move-only runner key permits one bounded callback over only the launched
executor and services. An armed non-permit fail-stop scope spans exact pre/post
checks that the two still share one output guard, the joint ingress remains
closed, the observer permit remains present, and live clocks remain unarmed.
The scope witnesses open output on both sides but releases its admission permit
before invoking nested executor/service code, so synchronous fail-stop cannot
self-deadlock. Callback failure therefore cannot return a retryable partially
configured stack. The production runner mints that key immediately before
lifecycle setup. Interrupted-tip
`PendingKuraApply` now crosses an opaque Decision-Fetch/WAL/runtime replay
join: the authenticated wrapper admits only the exact same-block Decision
Fetch, removes ordinary Fetch authority, and verifies the retained runtime
sidecar before local dispatch. Ordinary activation rejects both the
uninstalled replay seal and installed pending evidence. Its dedicated no-clock
lane-recovery/finalization state retains the activated lane adapter with the
lifecycle owner, publishes the recovered current-height snapshot, and hands
the authenticated successor into the ordinary lifecycle loop. A consuming activation type
state spans one fail-stop operation across live-clock arming, status projection,
the retained one-shot observer, exact joint-ingress opening, and
runner-authorized status publication. CompleteTip activation additionally
consumes the still-joined retired-H authority; ordinary/current/snapshot status
uses a distinct runner-owned seal. The resulting owner exposes only a
borrow-bound callback gated by a private runner key, never owner, executor, or
service parts. The ingress planner rejects unless the running owner and service
retain matching store and output-guard seals; a cursor snapshot alone cannot
authorize production planning. The activated owner now enters one consuming
finalization chain. It proves executor and recovered-work quiescence before
clearing readiness, closing the exact ingress, and jointly detaching the
Certified-Serve and leader-wire gates. It then consumes the executor's Kura
receipt/artifact and retires the serialized adapter WAL under fail-stop
ownership. The resulting type state keeps services, lifecycle stores, and
finality evidence joined while the existing lane/service rollover seals the
durable exact-output handoff. Only after that handoff may it authenticate the
recovered registry, rescan and revalidate the live Certified-Serve directory
against ledger-owned rows plus capacity-fenced admission waits, retire
payloads, and publish one all-row terminal LedgerV1 successor. The staged
successor and published receipt are opaque; publication consumes the exact
coordinator instance, and its final token consumes the concrete registry
before a cleanup-ready state can permit normal finalized-height worker
teardown. Operator shutdown is a separate consuming boundary for unpublished
or active lifecycle owners: it closes runner readiness and the exact queue,
releases prepared local-Proposal state, jointly detaches both ingress gates,
then permits normal worker stop without minting finality or retiring durable
rows needed by cold replay. The CompleteTip wrapper consumes retired H and H+1
together on that unpublished path. Canonical-body startup recovery borrows the
future ordinary or CompleteTip activation authority, temporarily opens its
exact normal ingress with the same admission semantics as the legacy loop, and
RAII-closes readiness and ingress before setup postflight; clocks and status
remain sealed. That wrapper also lends the ordinary closed-ingress
executor/service setup transaction while retaining the retired predecessor, so
common preactivation no longer needs a raw H+1 owner escape. The production
runner now mints and drives these states. PendingKura uses a separate one-height
lifecycle-owned no-clock path before handing its verified successor to the
ordinary lifecycle loop; no legacy height loop remains.

`ProductionV2Services::capture_lifecycle_capacity_rank` consumes the complete
selector. Under one locked transaction it either retains the physical I/O FIFO
guard, the correctly derived Consensus admission slot, and the canonical-output
fail-stop operation, or returns the selector with an opaque service
release-generation wait. Pre-plan abort completes the operation after restoring
ownership; once planning begins, only command publication completes it, so an
error closes output admission without a drop/reacquire window. No queue depth,
reservation parts, or reusable debt array is available. The owner first proves
by process-identity comparison that the service fail-stop guard is the
executor's canonical output guard. A foreign guard is rejected before capacity
capture or coordinator mutation. The owner then binds the reserved result to
the exact Waiting Fetch and concrete registry incumbent, rejects an existing
queued/active/completion-pending executor work owner, and constructs the sole
prospective Ready row from all six authenticated debts. Planning advances
request generation `n` to `n + 1`; reserved submission
settles the exact lease `Blocked` on the same source at `n + 1` before publishing
the persistence FIFO command. Phase B consumes the durable body completion and
advances the source from `n + 1` to `n + 2`, making that same Fetch row Ready.
Certified Serve stays typed unsupported at this planner join even though its
durable admission and concrete carrier binding now exist. Live execution still
cannot derive volatile reply-route authority from scheduler metadata; the
runner transaction consumes the actual route owner without reconstructing it.
For one selected Serve target, runtime reports only the direct facts that this
is the first checked observation and whether an exact older owner is runnable.
The worker opens one move-only, barrier-bound predecessor admission from that
observation. It admits only the selected older owner and its same-owner fanout,
then closes on explicit completion, error, or unwind through its RAII guard.
No persistent episode counter, cross-component witness, or
`Ready | Claimed | Complete` predecessor state remains.

The first fair-ingress tranche is an inert, move-only pre-dequeue queue cut,
not a planner or a second scheduler. Its sole mint takes `service_lock` before
the queue-state lock, freezes the physical admission cutoff and ready-source
prefix, and releases the state guard while retaining exclusive dequeue service
for later composite validation. Every pre-cut row binds its authenticated wire
key, complete process-local ownership projection, physical queue/source class,
exact stored canonical bytes and length, the production dequeue helper's exact
`Blocked | Strict | Dependency` verdict,
and its monotone obsolete-owner status. An opaque immutable carrier census is
retained beside that comparable geometry for executor-only classification. The
selected pending identity additionally binds carrier-derived height context,
exact source owner, wire identity, ownership projection, and unique physical
admission ordinal.
Lane and source positions are exact one-based non-zero values. Release-mode
capture/revalidation proves the complete ready/lane/pending-wire indexes,
counter and byte-accounting cut before comparing it. Producer appends at or
after the cutoff—including a newly ready source or newly admitted barrier
carrier—are excluded from the cut-aware projections without changing frozen
authority; malformed post-cut state still fails structural validation. Any
pre-cut reorder, removal, encoded-carrier or index mutation, identity mutation,
or same-wire ownership-history coalescence fails its comparison. The service
guard may span coordinator snapshot validation; the queue-state guard may not
span network, cryptographic, effect, or coordinator work.

While the remaining ingress-state cleanup is completed, the cut retains typed
snapshots of both existing physical barriers. The Certified-Serve projection
binds the exact gate
actor, selected request/carrier, request cutoff, and predecessor-cleared
predicate. The leader-wire projection binds its gate actor, durable Ingress
ordinal set, complete active token/carrier/predecessor map, earliest selected
barrier, body/control dependency, and monotone obsolete-token verdicts. The
existing dequeue and lifecycle cut derive these values through the same pure
helpers. Rebinding either gate or advancing any pre-cut authority therefore
fails the queue CAS; these projections disappear with the old barriers rather
than becoming new scheduler state.

The queue-local cut alone deliberately does not mint selector debt: transport
queue state cannot decide the per-occurrence fatal/restart, retained-dispatch,
timeout-recovery, capacity/owner, terminal-subject, malformed-carrier, and
Certified-Serve preparation/backpressure predicates. A separate inert executor
join now classifies the complete opaque pre-cut census under the same service
turn, validates the tracker/work/pending-fetch/retained-response request cut,
and confirms that any acquired family claim equals the retained response. It
maps every certified response to the formal aggregate untrusted resource source
even though its production hop is authenticated, and counts each drainable
request-fenced physical occurrence independently of claimed-family authority.
Every response which passes exact read-only authentication first remains in the
physical-ordinal census; candidates are grouped by exact signed-request hash,
and only the unique lowest physical ordinal in each family receives the
claimed-family verdict. Non-winning retransmissions remain census members and
may still own request-fence priority. An occurrence holding both authorities is
counted once because selector debt is the checked cardinality of their concrete
ordinal set union; an empty complete set is exactly zero. Remote-invalid
response carriers may retain request-fenced physical priority; inconsistent
local indexes fail closed.

The returned `PreparedLifecycleIngressSelector` is opaque, move-only, and
borrow-free. It retains the complete queue-minted occurrence-identity map,
verdict map, selected lane/source position and identity, plus each winning
family's immutable carrier and equality-reprobed executor candidate. Its
embedded opaque queue witness also preserves the physical cut, complete
ready/lane geometry, occurrence identities, and Serve/leader barrier
projections, so the future final CAS can reject a reorder of an unrelated
pre-cut row rather than checking only the selected target. Releasing the queue
guard and executor borrow makes this a preparation rather than live authority:
it may be stale immediately after return and exposes no general clone,
constructor, mutation, claim, reservation, or dequeue API. The only
general-purpose crate-visible mint takes the ingress queue and target physical
ordinal together; raw cut rows and candidates remain sealed. The recovered
Decision-Fetch prerequisite adds a narrower queue-owned mint with no ordinal
input. Ordinary checked dequeue and that mint call one shared candidate
selector over the same ready-source rotation and per-source lane order, scanning
every `Strict` candidate before any `Dependency` and selecting obsolete
carriers independently of the downstream predicate. Queue state is released
before that predicate runs while dequeue service remains exclusive. The
executor supplies the same pure retained-debt/terminal predicate used by the
ordinary runner, authenticates the complete cut, and returns an opaque selector
only when the exact fair winner is the recovered response-family owner. An
ordinary, obsolete, foreign-context, or non-winning retransmission is a
non-mutating pass-through; it cannot cause discovery to skip to a later
recovered ordinal. The result still cannot mint `SchedulerInputs`.

The launched lifecycle now owns a unified one-turn driver prerequisite around
the real borrow-bound Completion and Ingress cursor. Completion first services
its retained Apply-deferred, guarded Sign, and guarded Decision-Fetch owners,
then takes at most one physical worker head, and only then classifies the
complete Ready census. A selected Apply, Sign, Fetch, or full-census Broadcast
transaction consumes the turn. Unsupported or ordinary work returns the exact
unchanged cursor. The Sign preview structurally selects exactly one of
Broadcast, ProposalPrepareWal, VoteBroadcastAndSign, or
ProposalBroadcastAndSign before the driver calls one consuming settler. A mixed
Apply/Sign/Fetch Ready census now freezes the worker FIFO and exact-output
corridor together. Every attested row enters the same scheduler snapshot with
its physical availability and predecessor debt, and only the ranked row
receives a typed worker or output/executor reservation. An unavailable row
remains in the exact Ready census without a sequential class-specific probe.

The closed-ingress preactivation key also owns the modular runner's
opaque local-Proposal scheduler state. A WAL-authenticated recovered
ProposalIntent remains move-only through cold adapter advancement; its sole
initializer compares the exact reducer directive and mutates that same runner
state inside the armed setup transaction before privately minting a
context/directive-bound prepared owner. Ordinary and CompleteTip activation
consume and retain that owner, so returning or discarding the directive cannot
acknowledge recovery without suppressing the duplicate local attempt. The
activated runner-only borrow supplies that same opaque scheduler owner beside
the lifecycle owner, executor, and services. The non-Pending production runner
mints that owner before setup and retains it through activation and live proposal
scheduling. Only the isolated PendingKura no-clock recovery height continues to
use the legacy `LocalProposalState`.

Ingress first consumes and retries any retained recovered capacity wait, then
freezes exactly one winner with the same queue-owned strict-before-dependency
selector and pure drain predicate used by ordinary dequeue. The cut retains the
service episode. Obsolete, context-free, foreign-context, and ordinary winners
are physically removed by the queue's existing accounting/rotation tail and
become one opaque move-only handoff; no free-standing ordinal or second
selection crosses the boundary. A non-response head never enters response
census. A selected response authenticates only its own signed-request family,
so an unrelated later malformed family cannot poison the exact ordinary head;
an exact recovered family winner alone retains its queue witness and enters
Decision-Fetch Phase A without dequeue.

For a selected current-height Certified-Serve request, the driver arms output
fail-stop before authentication, durable negative staging, or service
preparation. Accepted, rejected, and service outcomes move beside the exact
dequeued carrier into the opaque handoff. Capacity backpressure completes the
local fail-stop scope but retains both the physical cut and the already
installed off-queue debt, so the Serve barrier cannot be leapfrogged. Every
post-preparation error closes output while the service guard is still held, and
dropping an unconsumed handoff closes output before its admission or carrier is
released. The activated ordinary lifecycle owner consumes that handoff through
one production runner tail; PendingKura admits only its restricted decided-lane
path. A runner-owned lifecycle height
driver also preserves the real Completion/Runtime/Ingress cursor order and
derives output authority from the retained services for the whole ordinary
batch. `run_inner` now transfers every ordinary, Applied, Snapshot, and
CompleteTip height into this owner; a PendingKura recovery executes once under
its lifecycle-owned no-clock state and transfers its verified successor into
the same lifecycle loop. No second batch or post-dequeue implementation
remains.

The queue-only final CAS now exists behind that sealed boundary, but it is not
exposed from `PreparedLifecycleIngressSelector` and has no production caller.
The cut binds an unforgeable process-local queue identity and its move-only
witness must agree with the enclosing expected lifecycle context and selected
physical ordinal. Commit reacquires `service_lock`, snapshots the exact
cut-aware geometry and both barrier projections, releases the state lock for
carrier re-encoding and ownership-hash validation, then reacquires state for
the final cached geometry/barrier/context comparison immediately before
mutation. The Certified-Serve reservation lifecycle id already binds its exact
request hash, so this final lock compares the reservation, carrier ordinal,
payload shape, and any dependency round/subject without hashing the request.
That final cached comparison performs no body encoding or ownership hashing;
after it succeeds, the shared production tail still performs its existing
durable ownership validation while holding the state lock.
Post-cut appends remain outside the comparison and survive source
rotation; any pre-cut reorder, removal, coalescence, ownership change, barrier
change, or queue substitution rejects without dequeuing. Exact success uses a
single factored production tail for durable leader-wire binding, Certified-Serve
physical drain, lane/global counters and indexes, source rotation, and the
runtime physical cut. Consuming the witness makes this queue transition
one-shot. It still cannot claim a response family, reserve runtime/service
capacity, or act without the future output-permitted `v2_effects` transaction.
That future wrapper must consume the complete prepared selector, finish the
last candidate equality probe while its immutable family carriers are retained,
then release every retained inbound `Arc` before invoking the queue commit; the
shared production dequeue tail rejects a selected envelope with another strong
owner rather than cloning physical ownership.

An inert late-response readiness join is now sealed and module-private beside
that queue witness; no sibling production module can invoke it before the
consuming transaction exists.
It borrows, rather than consumes, the complete
`PreparedLifecycleIngressSelector`, and only the queue-selected occurrence may
enter it. A request-fenced response which is not the lowest authenticated
physical winner of its exact signed-request family is rejected; it cannot
borrow the winner's candidate. The borrowed preparation retains the complete
selected queue-minted context, digest, and physical ordinal for the future
queue CAS; the readiness join first proves that exact identity is the family
winner, then binds the exact request hash, executor-authenticated response
candidate, its sealed
`PendingRuntimeEffectBinding`, and the active context. Certified-Fetch
admission and response wake share one fail-closed key helper for the existing
six-field Fetch projection, including the same domain-separated block subject
and execution commitment; an ordinary Fetch without explicit certificate
commitment authority cannot use this path. The binding's authenticated causal
lifecycle hash supplies the only causal root.

The exact signed-request hash also feeds one shared
`iroha:sumeragi:v2:lifecycle:certified-fetch-wait-source:v1` derivation. A
response authenticates that external source, not a caller-selected generation:
the coordinator locates exactly one existing Fetch row by semantic key and
causal owner, reads that row's current `Waiting` token, requires its source and
coordinator-observed generation to agree exactly, and passes that unchanged
token through the existing readiness reducer. The staged transition changes
the original `Waiting` row to `Ready`, advances its source generation, and
replaces its one existing physical slot at the same address with the complete
queue-independent completion digest. That digest binds the causal key, exact
Fetch effect identity, canonical replay authority, and durable body frame; the
queue occurrence and authenticated response transport identity are deliberately
excluded. It never allocates an owner, ordinal, record, slot, or capacity
charge. A missing/non-unique slot, a slot outside the frozen authenticated
capacity geometry, a slot not already consumed by the incumbent carrier, an
invalid consumed-set subset, or a no-op digest is rejected before mutation. The
prepared logical mutation retains an exclusive borrow of the exact coordinator
snapshot until its infallible assignment, so it cannot be applied to a different
or meanwhile-mutated authority. The consuming Phase-B transaction below must
pair this logical `PhysicalReplacement` with the same-address concrete-work
registry swap; neither half is exposed alone in production. An exact duplicate
against `Ready`, or a late response against the
exact terminal tombstone, stutters without advancing the generation or
replacing the installed response. Missing/foreign identities, claimed rows,
wrong sources or generations, ambiguous shared-source waits, and damaged
indexes return typed failures without latching a new coordinator fault.
Certified Fetch now crosses that boundary in two sealed phases. Phase A
consumes the initial selector after one final executor equality probe, moves
the winning candidate's non-clone `AuthenticatedCertifiedBodyResponse` into an
ordinary bounded storage-worker command, and drops every selector witness and
retained inbound `Arc`. The command uses the existing executor `EffectWorkId`
and an exact `(queue identity, response hash)` work descriptor; it allocates no
runtime lifecycle ordinal. Full, disconnected, output-closed, or conflicting
admission returns the whole move-only task unchanged. Exact retransmission
coalesces against the existing work index, so repeated probes cannot schedule
a second persistence operation.

The worker fsyncs the canonical body through `V2BodyStore`. Its ordinary
completion carries the exact queue identity, the same opaque authenticated
response, and the sealed `DurableCertifiedFetchBodyReceipt`, but no stale
selector witness. Completion extraction removes only completion-lane position
metadata. A private drop-armed work acknowledgement retains the exact command
descriptor in `CompletionPending`; it releases that duplicate fence only in
the final successful Phase-B tail. Dropping an unconsumed completion closes
consensus output for restart. The completion travels as a typed outcome of the
ordinary drain and is never parked in a service `Option`, latch, or second
scheduler branch.

Phase B is one consuming coordinator surface. It captures a fresh complete
selector for the persisted physical ordinal, equality-reprobes its exact
candidate against both the persisted authenticated response and current
executor, and preflights every still-live owner before durability: the exact
response-family claim and Fetch/body-pipeline retirement, service Fetch-owner
removal, logical Waiting-to-Ready same-slot replacement, concrete-registry
address/incumbent conversion, durable receipt, authenticated responder, full
response family, and ordinary `Admit` dequeue disposition. The registry
preflight accepts only the selector-minted opaque authority and preserves
cardinality at the same `(OwnerId, ordinal, PhysicalSlotId)` address. Every
failure before the LedgerV1 call returns the complete opaque completion for a
fresh-selector retry; no response, receipt, acknowledgement, or queue parts are
exposed.

After those checks the transaction begins one fail-stop output operation and
invokes `persist_exact_staged_successor` for the staged Ready successor. The
persistence call itself is the status cut: any error from it is restart-only,
because atomic replacement may already have crossed rename or fsync. A
successful return is followed by the fresh witness's checked queue CAS. A CAS
error retains that exact witness and completion in a restart-only result. Exact
success enters an assertion-only tail: install the closed durable completion at
the same registry address, publish the staged coordinator, commit the executor
response claim and retire its Fetch/body-pipeline owners, remove the exact
service owner under the held output permit, release the indexed command
descriptor, and disarm the fail-stop operation. There is no runtime
`BodyAvailable` reservation, compatibility ordinal, raw-parts API, or
post-dequeue retry path.

This composite is driven by the serialized lifecycle runner. A live
Fetch row whose LedgerV1 payload is `BodyFrame` now denotes the durable Ready
carrier: its canonical completion digest binds the causal root, complete Fetch
effect (including QC, manifest-presence shape, and ordered certified sources),
and exact body-frame replay envelope, while excluding request, response,
responder, and fair-ingress retransmission identity. Restart therefore needs no
`PendingFairIngressIdentity` or dequeued response occurrence. The closed
registry carrier retains the storage-authenticated `DurableBodyReceipt`, not a
dequeued response.

The recovery mint is a consuming, queue-independent storage cut. It takes
ownership of the exact opened LedgerV1 and its opened store instance, verified
height context, and non-clone `V2BodyStore`; authenticates each retained source
list and QC before minting its body or pending binding; censes every live
BodyFrame-backed Fetch; and retains all five values behind an opaque all-row
seal. The seal reloads the same retained ledger store and compares its complete
canonical frame, not a caller-selected same-context path, in addition to the
live-Fetch count and verified body-store context. It has no clone, row,
candidate, work, parts, or install API, so it cannot be reused to mint one row
at a time or paired with a foreign store.

The sole V1 startup transaction now consumes that seal together with the
matching authenticated Certified-Serve payload cut/store. It validates the
complete Fetch census and empty registry before consuming terminal Validate
body outcomes, moves every logical Fetch candidate into the recovery cut,
installs every closed concrete Fetch carrier into the fresh registry, and
prepares the coordinator from the same retained ledger-store instance. Full
coordinator/registry equality is checked before ledger publication or payload
orphan pruning. Publication reloads that store once more and replaces it only
if it still equals the authenticated predecessor frame, so prepare-to-commit
drift fails without overwriting the changed frame. Success returns one opaque
owner retaining the verified context, coordinator, registry, payload store, and
body store. Every failure is startup-fatal and exposes no authority or retry
parts. Unsupported live classes still fail closed. The serialized lifecycle
runner owns this startup transaction and the corresponding completion
settlement; the count-only compatibility drain remains fail-closed if reached.
This is first-release V1 only: there is no legacy decoder, fallback,
compatibility branch, or dual scheduler path.
Selector debt zero remains valid only when the complete typed verdict census
proves that no pre-cut occurrence owns priority.

The production cutover has an explicit ordinal-free seam. A move-only
`PendingRuntimeEffectBinding` retains only the authenticated causal lifecycle
key, exact concrete-effect identity, and inherited semantic statement under a
separate integrity projection; the runtime ordinal is absent. The
logical coordinator still stores only the resulting physical slot and digest.
Complete process-local `AdapterEffect` values live in a sibling deterministic
registry keyed by `(OwnerId, record ordinal, PhysicalSlotId)`, never by digest
alone, because two inherited body authorities may share one concrete carrier.
Registry installation consumes both the effect and pending binding, rejects
overwrite, digest drift, or disagreement between the pending causal lifecycle
key and the admitted `OwnerId`, resolves only an exact lease-advertised slot,
and is one-shot on execution. Lease removal returns the complete move-only
effect-plus-pending token rather than a bare effect, so a `Blocked` or
`Replenished` settlement can restore the incumbent without reminting causal
authority. It owns no admission, readiness, rank, retry,
wait, generation, capacity, lease, or ordinal state. The registry remains inert
until the same atomic production switch removes the old runtime allocator and
scheduler; it is not a dual-mode execution path.

The concrete admission seam consumes the complete effect and its move-only
pending binding. It first projects and stages the digest-only coordinator
transition. A first `Admitted` result may derive the staged record's sole
`(OwnerId, ordinal, PhysicalSlotId, digest)` address. An exact retry of a
`Waiting(Recovery)` record may derive the same immutable address only after the
staged reducer has rebound its authenticated geometry and made it `Ready`;
this is reported separately from an ordinary retry. In both cases that exact
work is installed without overwrite before lifecycle-ledger publication. A
registry failure discards the staged logical transition and returns the input
pair. A ledger publication failure synchronously removes the just-installed
entry, keeps the old active logical state (including the unrebound recovery
wait), returns the pair, and latches the existing durability fault. The staged
coordinator becomes active only after registry installation and ledger
publication both succeed. Capacity waits, ordinary retries, terminal
replay/stutter, and rejections return the exact pair without touching an
incumbent registry entry. The externally nameable holder exposes only empty
construction; every registry mutation stays inside the coordinator module.

Certified Fetch completion has a separate direct reducer seam and must not
re-enter the retired producer machinery. Production must not expose its
execution token from a raw response or a pre-persistence selector snapshot.
The exact response is first authenticated against its outstanding certified
request, then its canonical body is written through `V2BodyStore`; only after
the body file and directory entry are synchronised does storage return the
sealed `DurableCertifiedFetchBodyReceipt`. While that bounded I/O is
outstanding, the original Fetch row remains the sole executable authority and
no closed completion or Store row exists. A crash before the receipt therefore
recovers or retransmits the Fetch. A crash after body fsync but before the
frame-bound Ready LedgerV1 successor is published leaves an idempotent orphan
and follows the same Fetch recovery/retransmission path. Queue-independent Ready
reconstruction begins only after that LedgerV1 publication. Body-first orphan
files are safe and idempotent; ledger-first Store publication is forbidden.

After obtaining that receipt, the transaction freshly recaptures the complete
selector/queue cut, prepares the drop-inert registry preflight, consumes it with
the receipt, performs the exact checked dequeue, and only then installs the
closed same-address completion carrier. No selector or queue witness retained
across the blocking persistence call is admissible. The resulting borrow-bound
registry execution token checks the claimed Fetch lease, owner, ordinal,
consumed slot, installed response digest, retained Fetch effect, retained
pending causal binding, and nested durable body receipt. While that token keeps
the concrete row exclusively borrowed, the adapter previews `BodyAvailable`
against cloned reducer and wire-registry state. Preview never calls the
serviced-candidate store, producer-continuation reservation, deferred FIFO, or
runtime lifecycle ordinal allocator.

A `Busy` preview leaves the completion installed and settles the unchanged
Fetch lease on one context-scoped external reducer-fence source. The adapter
owns that source's monotone generation and advances it whenever pending
persistence, awaiting-signature, or replay-fence authority changes. Generation
`u64::MAX` is permanently reserved as invalid; exhaustion fails closed before
either a wait token or a reducer mutation is exposed. Idempotent, superseded,
and deferred-conflict results retain the same exclusive adapter
borrow until the future transaction gives them their typed settlement, so they
cannot become check-then-use classifications.

An applied preview is valid only when the cloned reducer emits exactly one
`StoreBody` whose tag, round, and subject all equal the input `BodyAvailable`.
The retained Fetch pending binding then projects the Store binding without a
runtime ordinal: the immutable causal key and inherited semantic statement stay
exact while only the concrete effect kind, physical identity, and integrity
projection change. This projection is deterministic data, not separately
executable authority; it remains sealed inside the parent-to-successor
transaction and cannot coexist as a usable duplicate of the Fetch binding.

The production transition must stage Fetch `Advanced` and Store admission on
one coordinator copy, release the parent's Effect charge before admitting the
successor, require the same `OwnerId` with a newly allocated record ordinal,
install the Store concrete work, and publish one lifecycle-ledger replacement.
Only after every coordinator, registry, body, service, request, response-claim,
and queue preflight is exact may the already-previewed adapter state be moved
into place. A post-dequeue mismatch is process fail-stop, never retry. This
direct path remains unreachable until restart can reconstruct the exact ready
body and replay or inject the matching `BodyAvailable` before exposing a live
Store row; process fail-stop cannot substitute for that power-loss contract.

The next direct seam advances that durable Store row to `ValidateBody` without
reviving a parallel completion scheduler. Its sealed pending binding projects
only
an exact `StoreBody -> ValidateBody` pair with identical tag, round, subject,
causal lifecycle key, and inherited candidate statement; only the concrete
effect identity and integrity projection change. The adapter rechecks the
nested `DurableBodyReceipt` against the frozen context and registered manifest,
then previews `BodyStored` on cloned reducer and wire-registry state. Applied is
valid only for one exact `ValidateBody`; Busy and every inactive result retain
the adapter borrow, and commit remains an infallible state installation inside
the future Store registry transaction.

The concrete registry mirrors that boundary with two closed, move-only body
carriers. `DurableStoreBody` and `DurableValidateBody` each retain their exact
coordinator-minted address, owned adapter effect, ordinal-free pending binding,
and durable body receipt. They also retain an independently transferred hash of
the authenticated response manifest; execution requires that hash to equal the
receipt's catalog hash instead of treating the receipt as both sides of the
comparison. Typed borrow-bound preflights re-project each carrier under the
verified height context and require the claimed lifecycle key, causal owner,
stage, reconstruction source, and complete consumed one-slot Effect geometry.
Generic registry borrow/take operations reject both closed forms, and the
current tokens expose no constructor, installation, removal, commit, or parts
extraction. The sealed Fetch-to-Store and Store-to-Validate successors project
their mandatory replay authority and body-frame payload only while retaining
the exact parent registry borrow. Coordinator staging consumes each token into
a dual-borrow, drop-inert prepared cut; it still cannot publish until the
adapter, coordinator, registry, ledger, and recovery cuts can commit together.

Because the Store input already names a synchronised body-store frame, restart
must reload those exact bytes from the durable catalog rather than depend on
the volatile `ready_bodies` cache. Publishing the Store-to-Validate ledger replacement is
safe only when recovery can replay `BodyAvailable` and then `BodyStored` for
that exact manifest before exposing the reconstructed Validate row. Durable
bytes alone do not prove that either reducer transition was replayed, and
process fail-stop does not replace this power-loss ordering contract.

Successful deterministic validation has a wider closed reducer result than the
earlier body stages. Its private, borrow-bound preview registers the
independently durable execution commitment on a cloned wire registry and then
classifies exactly five outcomes: reducer-fence `Busy`, ignored/inactive,
applied with no effect, one exact `Apply`, or one exact safety-WAL `Persist`.
Every outcome retains the staged registry. Non-Busy ignored outcomes also
retain the staged reducer because `ValidationCompleted` may advance
`Durable -> Validated` before determining that the current role, view, or
decision owns no child effect. The `Persist` carrier keeps the complete core
effect sealed and exposes neither encoded WAL bytes nor an append capability;
the `Apply` carrier requires the exact event tag, subject, proposal round,
execution commitment, and durable Decision. Preparation mutates no live
adapter, WAL, lifecycle record, or parallel completion queue.

Deterministic validation failure uses a separate private preview because it
must never mint a success commitment or enter the safety-WAL continuation.
After rebinding the exact `DurableBodyReceipt` to the frozen context and the
independently registered manifest, it admits only four outcomes: reducer-fence
`Busy`, ignored/inactive, applied with no effect, or one exact
`ReportInvalidCertifiedBody` carrying the registered Prepare QC for the same
round and subject. Every outcome retains the staged registry, and every
non-Busy outcome retains the staged reducer. The report path round-trips its
core certificate through the wire registry and rejects any phase, proposal
round, subject, or carrier mismatch. These move-only tokens expose no commit,
WAL append, deferred-completion, serviced-candidate, producer-continuation, or
lifecycle mutation surface; the future composite transaction must consume the
selected rejected-validation carrier and publish its diagnostic effect through
the same coordinator-owned execution cut.

The body-store side of that seam is scheduler-free. It accepts an exact
`DurableBodyReceipt`, a separately carried expected manifest hash, and the
deterministic validator; before invoking the validator it rechecks the frozen
context, round, subject, checksummed frame, stored manifest, and both manifest
hash authorities. Its closed non-clone result is either a post-fsync
`ValidatedBodyReceipt`, a deterministic rejection bound to the same durable
receipt, or an exact certified-merge-sidecar deferral bound to that receipt.
No work id, runtime owner, lifecycle ordinal, or scheduler decision appears in
this surface. The existing worker-task entry is not a second authority: it
delegates to the same storage core and must be deleted at production cutover.
Only the registry path supplies the independent manifest authority needed by
the direct lifecycle transaction.
The scheduler-free result has a consuming success-only extraction which
returns every rejection or sidecar deferral intact. The closed Validate
registry preflight can bind that exact `ValidatedBodyReceipt` without changing
its row: it rechecks the durable receipt, any inherited Prepare/Commit
commitment, and derives a domain-separated replacement digest from the
incumbent physical identity, independent manifest hash, durable frame, and
canonical execution commitment. The resulting borrow-bound token exposes only
preview inputs, the validation receipt, and old/new digests; it has no install,
remove, or commit operation until the coordinator Ready replacement and
registry conversion can publish atomically.

The body-store validation marker is a first-release closed outcome envelope,
not a success-only witness. It stores either the exact validated execution
commitment or the canonical deterministic-rejection code beside the context,
proposal round, subject, manifest hash, and checksummed body-frame hash. A
merge-sidecar deferral is never persisted as terminal authority. Rejection
diagnostic text remains volatile and does not enter the marker. On restart all
marker kinds are quarantined; the body is reloaded and deterministic validation
must reproduce the same success/rejection kind and, for success, the identical
commitment before the marker is promoted. A changed kind, commitment, frame,
manifest, or rejection code fails closed without partially promoting the
catalog. An exact rejection repeat is idempotent and does not rerun validation.
Promoted rejections remain private. The storage-only lifecycle assembler now
consumes them only through one body-store-instance-bound aggregate catalog cut
covering both successful and rejected outcomes. The cut selects every exact
`AdvancedNoSuccessor` Validate claim once; dropping it restores selected and
unselected outcomes, while commit consumes only selected outcomes and restores
unrelated success authority needed by recovered-WAL replay. Pending semantic
revalidation, retired sidecar deferrals, a missing/substituted frame, or an
ambiguous success/rejection key fails before recovery authority is published.

The inert asynchronous handoff now makes that storage interval explicit. The
borrow-bound Validate preflight can be consumed into a sealed, non-clone owned
request which snapshots the exact registry address and incumbent digest,
adapter coordinates, durable receipt, independent manifest hash, causal key,
inherited candidate-authority statement, and immutable lifecycle key and stage.
Consuming that request is the only new execution operation: it calls the
scheduler-free body-store core and retains the complete request beside the
closed result. The mutable registry is not borrowed while body I/O or
validation runs. Reattachment reacquires the registry and rechecks the
unchanged address, digest, closed carrier, pending effect binding, causal key,
authority statement, durable receipt, lifecycle identity, and outcome; failure
returns the owned executed token intact, while success remains drop-inert under
a new exclusive borrow. No service queue, work identifier, caller-supplied
ordinal, ordinal allocator, or second scheduler participates in the handoff.

The claimed-side dispatch cut is now explicit and remains inert. Its sole
coordinator-plus-registry entry point consumes the exact claimed Validate lease
only after rechecking the active record, reverse identity indexes, closed
carrier, verified projection, and one-slot geometry. A domain-separated
external wait source binds the coordinator-minted address, incumbent digest,
causal key, inherited authority statement, durable frame hash, and independently
transferred manifest hash. The entry point samples that source's current
observed generation, rejects the reserved maximum and any waiting-row alias,
then settles `Claimed -> Waiting` on a coordinator copy and proves that no
unrelated record, index, capacity, generation, or ordinal state changed. Only
after those checks does it detach the registry request, publish the staged
coordinator, and return one non-clone dispatch. Every precommit failure returns
the caller's exact lease; a body-store error returns the complete dispatch and
wake authority intact. Dropping a successful dispatch leaves the row explicitly
Waiting and the closed registry carrier unchanged. This tranche creates no
second body task or work identifier, enqueues no service work, publishes no
Ready event while dispatching, rewrites no lifecycle ledger, and has no
production caller. Merge-sidecar registration and wake remain prerequisites
for wiring it.

Executable validation completion is one volatile two-sided transaction.
Successful validation and deterministic rejection each install a closed
same-address carrier which owns the moved `DurableValidateBody`, its original
incumbent digest as a separate authority coordinate, and the exact body-store
outcome. The replacement digest is domain-separated by outcome and binds the
old digest, independently transferred manifest, durable frame, and either the
canonical execution commitment or the one closed reducer-level rejection
identity. Human-readable rejection text remains diagnostic-only. The
preflight rechecks the exact Waiting token, immutable lifecycle key and stage,
one authenticated independent Effect-slot episode, reverse indexes, durable
metadata, unchanged registry address and digest, pending binding, receipt, and
outcome. It then publishes `Waiting -> Ready` with an equal-slot physical
replacement on a coordinator copy, proves the wait generation advanced by
exactly one and every unrelated record, index, capacity, high-water, debt,
durable, universe, and consumed coordinate remained unchanged, and only then
stages the registry carrier under an unwind-safe guard. The live coordinator
is replaced with `mem::swap`, immediately followed by an infallible guard
disarm which returns only precomputed typed location metadata; no fallible or
panicking work follows either volatile commit. Any precommit error returns the
exact executed dispatch, and unwind before the swap restores the original
registry row byte-for-byte.

Missing certified-merge sidecar is not executable completion. It installs no
carrier, publishes no Ready event, and returns a sealed move-only deferral token
which retains the exact executed dispatch, wake authority, and missing
reference while both live sides remain byte-for-byte Waiting/original. The
subsequent sealed sidecar registration and same-row wake transaction consumes
that token. Waiting, Ready, wait generations, and physical carriers are absent
from `LifecycleLedgerV1`, so neither executable publication nor deferral writes
the lifecycle ledger. A crash before the volatile cut retains only the durable
Validate row; a crash after it likewise reconstructs that durable row and
semantically revalidates the body-store success marker before any replacement
is re-exposed. There is no production caller yet.

Ready Validate execution now has an inert, sealed preflight. One exact
coordinator-minted lease must retain the independent Validate key and stage,
owner, ordinal, single Effect slot, and the outcome-bound replacement digest.
The registry replays the retained incumbent through the verified height
projection and rechecks its causal key, candidate statement, durable receipt,
independently transferred expected manifest hash, and closed outcome authority.
Only a durable validated receipt or the one canonical reducer rejection
identity can cross the boundary; merge-sidecar deferral never publishes Ready,
and rejection diagnostics remain inaccessible. The resulting non-clone token
holds the exclusive registry borrow. Its sole fixed-output join consumes the
token, constructs one private-field authority for the already-proved outcome,
and passes that value directly into the matching sealed adapter entry point.
There is no generic callback, replayable `&self` projection, or raw receipt and
coordinate return path. The adapter preview is immediately consumed into one
opaque, non-clone publication preflight while the registry token remains
borrowed beside it. Validated Busy, inactive, no-effect, and Apply plus every
rejected branch retain their exact staged authority unchanged. Validated
Persist accepts only validation-origin `PrepareIntent` or `LockAndCommit`,
encodes the exact next WAL payload, checks its expected sequence, and simulates
the matching `Persisted` acknowledgement on cloned reducer/registry state. That
simulation must close to exactly one matching vote-signing continuation and no
nested Persist. The encoded payload, post-acknowledgement state, and Sign effect
remain private. Adapter or publication-preflight errors retain the complete
opaque registry token. The bridge exposes only the nine-way Copy discriminator:
no commit, install, extraction, WAL bytes, raw event or receipt constructor,
worker-task identifier, or scheduler authority.

The first-release surface has no raw body-stage, Apply, or Sign preparation
entry points and preserves no compatibility wrappers for them. Adjacent work
can advance only through the sealed fixed-output lifecycle joins below.

The validated-Persist branch additionally has one fixed, no-argument registry
join. The still-installed validated completion mints a move-only predecessor
authority; it exposes no effect/pending parts and can project only the exact
Prepare/Commit Sign retained by the adapter preflight. `PrepareIntent` preserves
the ordinary Validate lineage. `LockAndCommit` can refine an ordinary Validate
only through a private capability minted from the exact staged registry
PrepareQC after proving the complete reducer-QC/wire-QC conversion and matching
unsigned Commit vote; no caller supplies or extracts a certificate. A failed
branch or lineage join returns the entire dual-borrow preview before WAL I/O.
The bound token alone appends the retained encoded payload and obtains a live
WAL identity from the actual fsync receipt plus the retained complete frame; no
synthetic or scalar locator enters production. It then replaces the temporary
frame-derived pending owner with the predecessor-derived Sign binding inside
the sealed replay envelope. Any append or post-append error is fail-stop and
restart-only. The post-fsync token retains the staged adapter state and the
registry borrow, exposes no Sign/effect/pending/receipt/locator parts, and its
Drop closes the adapter until recovery because WAL durability cannot be rolled
back. Its sole consumer is the live atomic Validate-to-Sign publication. That
transaction projects the already-authorized Prepare or Commit Sign directly
from the nested post-WAL pending/replay seal; it cannot repeat ordinary-to-
Commit refinement or accept a caller-supplied candidate. The coordinator first
stages the Validate parent as `Advanced`, its typed Sign edge, and the exact
Ready child. The registry then converts the existing recovered-WAL restorable
Validate cut into the same exclusive detached-parent/child-vacancy reservation,
retains the detached parent without a restore path, and prevalidates one opaque
ordinary Sign carrier against the staged child digest, causal owner, ordinal,
and unique Effect slot.

A live publication requires an attached LedgerV1 store. The store identity and
the on-disk current frame must equal the same coordinator projection used to
derive the staged successor; an in-memory-only success is forbidden. The exact
V1 successor is atomically replaced and fsynced before any volatile state is
published. After that return, the path contains only infallible ownership
moves: swap the staged coordinator, fill the exclusively reserved child row,
swap the adapter reducer and wire registry, clear the armed persistence marker,
install the next fence, record the `ValidationCompleted -> Persist` and
`Persisted -> Sign` outcomes plus body progress, and finally disarm the
fail-stop Drop guard. Any preparation or persistence error retains opaque
post-WAL authority, never restores the detached Validate parent, and requires
restart through the durable WAL/LedgerV1 reconciliation contract.

The transaction precomputes the exact post-commit adapter status before
LifecycleLedgerV1 fsync by temporarily installing the already-staged reducer
and registry, applying the two deterministic progress observations, encoding
the snapshot, and restoring the live pre-publication state. After ledger fsync,
the precomputed value is installed last through the infallible status setter;
the fallible status encoder is never called across the restart-only boundary.
The serialized runner still cannot invoke this consumer until its production
owner co-locates the lifecycle coordinator and concrete registry with the
adapter and replaces the legacy runtime scheduler rather than shadowing it.
The restart prerequisite is now represented explicitly in LedgerV1 by a
canonical three-state durable continuation. `None` is required for live rows
and unrelated tombstones. `AdvancedNoSuccessor` is accepted only for an
`Advanced` Validate that deterministically produced no child.
`AdvancedSuccessor` carries one typed forward edge and ordinal. Fetch and Store
must respectively name `FetchToStore` and `StoreToValidate`; Validate may name
`ValidateToApply`, `ValidateToInvalidBodyReport`, `ValidateToSignPrepare`, or
`ValidateToSignCommit`. Decode rejects unknown codes and every noncanonical
code/optional-ordinal pairing. Ledger validation and direct coordinator
reconciliation reject a missing, backward, dangling, multiply named,
foreign-owner, foreign-source, or semantically foreign edge. The target must
retain the same causal owner and reconstruction source, preserve context,
round, proposal round, subject, and independent predecessor scope, and obey
the commitment-refinement lattice. Fetch and Store preserve commitment
exactly; every Validate child carries a commitment, while an existing parent
commitment must match it. The longer body debt is therefore
Fetch=5, Store=4, Validate=3, signing=2, and Apply/report=1, so every newly
admitted continuation strictly reduces the first scheduler-rank component.
The durable payload relation is equally exact. Fetch owns no body-frame
reference before completion and may publish one on Store. Store-to-Validate
and Validate-to-Apply must then retain the same context, proposal round,
subject, manifest hash, and checksummed body-frame hash byte-for-byte. A mixed
or substituted frame invalidates the whole ledger. Payload-free Store and Apply
rows are not part of the first-release format; the generic raw effect projection
therefore fails closed until the receipt-bound body-stage transition installs
the exact frame. Vote-signing and diagnostic
children own separate replay authority and therefore carry no body-frame
payload, while their terminal Validate parent retains its exact frame. Every
live, claimed, waiting, and Ready Validate row carries that same key-bound
`BodyFrame`; scheduler-demand attestation and dispatch/completion preflight
compare it with the closed registry carrier's `DurableBodyReceipt`, so a
payload-free or substituted frame can neither be claimed nor awakened.

The live Validate-to-Sign transaction persists its parent tombstone, child row,
and typed continuation in one ledger replacement before exposing its volatile
registry or adapter publication. Other adjacent-body consumers must preserve
that ordering when wired. `AdvancedNoSuccessor` requires exact coverage
by a storage-authenticated body outcome joined to the immutable Validate
parent identity, including the identical body-frame payload. The checksummed
typed ledger tombstone is the authority for
the historical no-child branch: restart must not rerun the old adapter preview,
because later WAL/reducer progress can legitimately classify the same body as
Busy, duplicate, or already decided. Absence of both the child and that exact
body-outcome coverage is never authority. The general storage-only factory
deliberately continues to reject such a tombstone when no body store is
supplied. Its body-aware sibling authenticates the complete tombstone census
against the aggregate outcome cut, and the recovered-WAL sibling joins that
same census with the opaque installed Sign projection. All logical and
Certified-Serve checks precede catalog detachment; partial multi-row selection
restores the exact catalog before returning the owned ledger and Serve
authority.
For Persist, WAL fsync precedes ledger replacement. Startup must therefore
replay the WAL first and repair an old live Validate plus its uniquely
authenticated awaiting-signature continuation into the identical typed Sign
edge before ordinary ledger/candidate coverage runs. Missing or ambiguous
repair fails closed. The final ledger edge need not retain a WAL id because the
restart-only seal binds the exact replay frame sequence, reducer persistence
identity, verified complete-frame hash, parent owner/key/source, and
authenticated Sign candidate. The pure WAL recovery result retains the
calculated hash beside every recovered payload. The filesystem adapter exposes
no sequence-only append acknowledgement: its typed fsync receipt contains the
sequence and exact frame hash, the retained in-memory record contains the same
pair, and the adapter compares both plus the payload before acknowledging the
reducer. The recovered vote carries that three-field identity opaquely through
the ledger-parent proof, runtime successor, authenticated repair, durable
repair, and every failure that retains those seals; no raw parts API exposes it
as executable authority. The adapter now keeps startup effects inside a
private non-clone wrapper and defers initial status publication. Its sole
consuming join re-decodes and reauthenticates the final `PrepareIntent` or
`LockAndCommit` frame, proves that the exact vote remains the reducer-owned
awaiting signature, and removes the unique matching Sign before any raw batch
can escape. The vote never separates from that adapter wrapper: the same fixed
join consumes it together with an opaque, move-only cut detached from the exact
Ready/validated registry completion and uses a verified height context
reconstructed from the sealed adapter. No raw effect or pending binding crosses
that boundary. The join also handles the legitimate crash cut where an ordinary
Validate was already in flight when the WAL durably registered a PrepareQC,
using the token-retained full certificate to derive one Commit binding. The
validated-body outcome, durable receipt, parent address, and installed digests
remain retained beside the logical repair and are rechecked against the WAL
vote's execution commitment and the exact frame retained by the live or
already-repaired ledger parent. Projection failure restores the exact detached
carrier. After successful authentication the registry row is absent and the
result retains the exclusive registry borrow and vacant parent-address
reservation. Before fsync the outer splice binds that address's owner, ordinal,
semantic key, stage, durable source, payload, and physical slot to the exact
opened LedgerV1 parent. The shared ledger reducer then accepts only the live
parent transition or the exact already-repaired parent/child stutter; no third
terminal or continuation shape passes. It derives the concrete Sign address
from the staged/replayed durable ordinal and proves that child address is also
vacant. The store then reloads the caller's opened snapshot, rejects staleness,
and fsyncs the complete frame. This exact stutter is the crash-reentry path when
the ledger fsync completed before volatile child installation.
Every error—including preflight and post-publication errors—retains the sealed
registry reservation and is fail-stop/restart rather than ordinary rollback.
Failure retains every authority inside an opaque diagnostic; success produces
one non-clone adapter-plus-durable-repair seal with no raw effect, binding,
receipt, or startup-batch extraction. An authenticated startup authority may
not publish initial status directly. It first authenticates the terminal WAL
frontier against durable `last_id`, then binds the reducer's current signature
to its latest exact authenticated owner frame before passing the exhaustive
no-authority, phase-vote, control-Sign, or Decision-Fetch storage branch, exact
registry/coordinator open, and status-last publication. Unsupported live rows
retain the sealed startup and fail closed. The production runner reaches this
join through the unified owner factory, which obtains the opaque Validate cut
internally from the same sealed
registry/ledger parent. LedgerV1
purely stages either the one live-Validate to typed-Sign repair or an exact
idempotent stutter, revalidating the complete ledger relation in both cases.
The ledger store's publication receipt binds the store path, context,
parent/child keys, typed edge, child ordinal, and canonical frame hash;
authority checks reload and hash the bytes currently at that path rather than
trusting a caller snapshot. The outer post-fsync authority retains that durable
logical repair, the complete detached validation, the exclusive registry
borrow, and both vacant concrete addresses.
The post-fsync authority now has one consuming concrete-work installation. It
reloads the receipt-bound store and requires the repaired-pair stutter, vacant
parent and child, an otherwise empty causal owner, the receipt's exact child
ordinal, and the sole Effect-class slot/digest before mutation. One infallible
map insertion installs a closed `DurableRecoveredWalSign` carrier which owns
the complete durable repair plus detached validated completion; it exposes only
a borrow of the Sign effect required by generic registry inspection and never a
pending binding or consuming pair. Failure retains the complete uninstalled
authority in an opaque fail-stop diagnostic. Success returns an opaque installed
startup cut which keeps the registry exclusively borrowed and revalidates one
same-owner child with the parent absent. Dropping that cut releases only the
borrow and leaves the exact closed row installed. The installed cut now feeds
one sealed coordinator-open and publication transaction: it splices the exact
authenticated recovery parent to the installed Sign child (or accepts the
already-repaired child), prepares the coordinator without publishing either
store, checks exact registry/coordinator/LedgerV1 agreement, commits the ledger
and authenticated payload pruning, rechecks the post-commit join, and only then
publishes adapter status. Every failure retains the adapter, unpublished
effects, installed registry borrow, recovery authority, and any prepared or
opened coordinator state still applicable; status failure is fail-stop after
the exact open. Only production runner wiring and exposure of the final opaque
published wrapper remain intentionally unavailable.

The rejected-report branch now receives its one Consensus reservation before
the Validate record is claimed; the consuming transaction still must convert
that lease-bound reservation into the exact report child or release it on a
typed non-report result. A claimed record cannot use record-level Capacity as
a settlement wait. A durable Validate row is live, terminal `Advanced`, or
typed `Cancelled` by explicit old-height retirement; generic Completed,
Rejected, and Failed tombstones are rejected by the shared payload/terminal
validator. Validation success versus deterministic rejection is
reauthenticated from the exact body-store outcome, not encoded as an
unauthenticated tombstone discriminator.

The corresponding ordinal-free successor projections accept only the closed
`ValidateBody -> Apply`, ordinary `ValidateBody -> SignPrepare`, inherited
Prepare `ValidateBody -> SignCommit`, and inherited Prepare
`ValidateBody -> InvalidBodyReport` shapes under the same tag, subject, causal
lifecycle key, and exact statement refinement. Apply uses the existing
commitment-refinement lattice: an ordinary Validate may acquire the Decision
commitment, a Prepare-authorized Validate may advance only to the matching
Commit commitment, and a Commit-authorized Validate may retain only that exact
commitment. Prepare signing is only the ordinary-to-Prepare refinement; Commit
signing is only the exact Prepare-to-Commit refinement. The report binds the
full registered Prepare certificate in its concrete identity and carries no
candidate statement. A legitimate ordinary Validate may concurrently observe
a newly registered PrepareQC. Only the fixed rejected-Validate adapter preview
may now mint a move-only capability proving that the emitted report names the
exact PrepareQC in its staged registry; the closed registry join uses that
capability to derive the report binding while retaining the ordinary causal
root. No caller can supply the registered statement or certificate, and none
of these projections is executable or installable authority.

A shared inert coordinator reducer checks Apply and both Sign edges: it
terminalizes the claimed Validate parent before admitting the exact one-slot
Effect child, preserves the causal owner, advances the Effect generation once,
and keeps net Effect usage constant. Apply additionally requires the
store-minted validation receipt to bind the certificate's proposal round,
subject, context, and exact execution commitment. Another inert reducer stages
validated inactive/no-effect and rejected inactive/no-effect as
`AdvancedNoSuccessor`. Both release the parent Effect once; the rejected branch
also releases its lease-bound Consensus reservation by advancing only the
Consensus generation, never decrementing durable Consensus occupancy. Its
production entry accepts the fixed registry/adapter preview token and rejects
Busy, Apply, Persist, and Report classifications. All borrow-bound tokens
expose no persistence, registry mutation, or commit method. A future atomic
transition must still join the selected Validate lease, durable validation
receipt or rejection identity, exact reducer preview, coordinator cut,
registry carrier conversion, WAL ordering when required, and restart
reconstruction before any staged state is published.

An ordinary frozen-prefix target is blocked only by a member of its own frozen
predecessor universe that is currently `Ready`; a `Waiting` member blocks
nothing. The dormant producer uses a distinct handoff-barrier policy: once
Serve makes that adjacent record ready, later ordinals cannot overtake it.
Thus the producer debt has strict precedence without turning every earlier
Serve or unrelated prefix target into a global scheduler barrier.

`LifecycleLedgerV1` uses a new framed Norito file with explicit stable numeric
codes for phase, work class, stage, predecessor scope, and terminal outcome.
It persists context/height, the ordinal high-water mark, nonterminal records,
terminal tombstones, reconstruction sources, typed durable continuations,
small Certified-Serve payload references, and adjacent producer debt. A
continuation is durable reconstruction metadata, not admission identity: an
exact retry still stutters on its terminal parent. The ledger excludes
readiness, leases, wait generations, rank snapshots, physical carriers, and
scheduler episodes. This first-release format has no predecessor-format
detector, migration path, or compatibility branch.
Certified-Serve references are canonical Norito encodings of a typed small
envelope (request-bound lifecycle subject, exact signed-request hash,
authorizing-certificate hash, and typed terminal receipt), not unchecked opaque
bytes. The request
hash—not the certificate hash—is the content-addressed resolver key because a
certificate does not recover the requester, request signature, or the exact
request hash authenticated by a response.

The six-field Serve key keeps that exact signed-request identity without a
seventh field: its subject is a domain-separated digest of the certified block
subject and signed-request hash. This prevents two requesters for the same body
from aliasing one terminal tombstone or cached response while preserving the
common lifecycle-key schema.

Exact Certified-Serve payloads live in a scheduler-free, context-bound payload
store. It retains the full signed request and, after completion, only the
manifest, responder, signature, and exact response hash; canonical body bytes
remain in `V2BodyStore`. Admission fsyncs a Pending payload before persisting
the atomic Serve/Producer ledger pair. That operation is one sealed boundary:
each bounded capacity fence owns one exact pending frame, while conclusive
rejections synchronously delete and directory-sync theirs. An ambiguous ledger
durability failure keeps the authenticated file for restart reconciliation.
The sealed post-fsync receipt stays inside that coordinator-owned fence; no
runtime-side cache can forge or lose the receipt needed for rollover cleanup.
Repeated backpressure therefore cannot grow beyond the bounded admission-wait
table or consume the payload-store lifetime bound. Completion fsyncs its terminal
payload before settling the ledger. The arbitrary-verifier request mint exists
only in tests: production authentication is fixed to the live adapter or an
immutable verified-height roster and proof-of-possession set. Pending
persistence independently reauthenticates that exact request, derives the
retaining validator from the node's actual consensus key, and proves its QC
signer membership before minting a post-fsync receipt. The completion write
also accepts the exact durable-body receipt, rechecks request/manifest/body
binding, certified local retention authority, and the frozen-roster responder
signature, then mints its receipt only after file and directory
synchronization. Newly created store and ledger directories are installed one
component at a time and each owning parent is fsynced before success can be
observed. A payload-only crash tail is an orphan;
any ledger reference without an exact authenticated payload is a startup-fatal
recovery error. Opening the payload files yields only a structural recovery
cut. Before the lifecycle ledger can join it, startup re-verifies every exact
request signature and QC against the verified height roster and proofs of
possession, proves that the local validator still owns certified retention
authority, and reconstructs any completed response from `V2BodyStore` for an
independent manifest, responder-signature, and full-response-hash check.
After the exact successor is fully reconstructed but before its sole LedgerV1
publication fsync, startup deletes and directory-syncs every fully authenticated
payload with no predecessor or successor owner. This keeps every fallible
payload-store operation ahead of logical publication; a partial prune removes
only unowned Pending files and is safe to repeat after restart. A stale or
incomplete authenticated cut cannot authorize that pruning step. Completed or
typed-negative payloads without a ledger admission are impossible under the
write ordering and therefore fail startup without deleting the evidence.

Durable metadata is bijective with coordinator records rather than a parallel
ownership table. Serve and its adjacent ProducerTurn share one reconstruction
source and all five non-phase key coordinates; only `Serve` becomes
`ProducerTurn`. Successful Serve retirement requires
`Completed(Some(response))` so
the cached response digest is persisted before the producer is exposed. The
store uses fixed internal record/frame bounds; caller-provided bounds and
`usize::MAX` validation are forbidden. A sealed 65,536-record per-height ceiling
counts terminal tombstones as well as live work, and admission fails closed
before allocating when the ceiling would be exceeded. Durable admission and
terminal settlement reduce on a private staged transaction, atomically replace
the ledger, and only then publish the new in-memory state. Generic settlement
cannot terminalize Certified Serve: only the exact completed or typed-negative
post-fsync payload receipt can authorize that ledger transition. The production
coordinator is not clonable, so the height-local ledger retains a single public
writer. A persistence failure leaves the prior owner/ordinal or active lease
visible and latches a durability fault.

Durable open never accepts a caller-provided ordinal seed. It opens and
validates the height-local ledger first, derives the coordinator high-water
from that frame, and joins every live row to exactly one sealed,
storage-authenticated recovery candidate. ProducerTurn rows join only through
their adjacent Serve. Physical geometry is rebound before the coordinator is
returned, and any remaining `Waiting(Recovery)` row rejects startup. Every
Serve tombstone or live row must also resolve through the exact request-keyed
payload store. A terminal payload written before its ledger update is treated
as the expected crash cut: startup rebinds the atomic Serve/producer pair,
settles the authenticated terminal result, persists the reconciled ledger, and
only then exposes the coordinator.

The bounded storage-only recovery assembler consumes the exact already-opened
`LifecycleLedgerV1` frame together with the move-only authenticated
Certified-Serve payload cut; it accepts no raw recovery candidates. It permits
live `CertifiedServe`/`ProducerTurn` rows only when the existing payload
resolver covers the adjacent pair exactly. Every other live class returns a
typed `MissingDurableRecoveryAuthority { ordinal, work_class, stage }` failure.
A terminal Validate with `AdvancedNoSuccessor` is classified before generic
terminal acceptance and requires its separately authenticated body outcome;
the storage-only cut therefore rejects it when that authority is absent. Both
the successful seal and every failure retain the exact ledger frame and payload
authority. Durable open compares its reread to that retained frame before
reconciliation, so a pre-repair cut cannot be reused after a WAL-ahead
Validate-to-Sign repair and no classify/reopen gap can change the accepted
ledger census.

BodyFrame-backed Ready Fetch has a separate consuming prerequisite rather than
weakening that general assembler. It retains the owned ledger store and exact
LedgerV1 frame, verified context, owned body store, and complete
queue-independent Fetch census in one opaque cut. The cut is not executable or
installable on its own. Its completed startup composite consumes the entire
phase into the logical recovery cut and initially empty concrete registry,
opens against that same retained store, joins terminal Validate outcomes and
Certified-Serve payload recovery, and returns only the co-located production
owner. No partially spliced Fetch phase survives a later assembly failure.

The post-fsync recovered-WAL cut uses a separate sealed storage assembler; the
general storage-only assembler is not weakened. Its only additional input is
the opaque projection minted from the exact installed recovered Sign registry
row. That projection retains the repaired child candidate and its durable
owner/ordinal address. The assembler admits exactly one live Sign child only
after its key, owner, ordinal, work class, stage, reconstruction source,
payload, terminal state, and continuation match the repaired LedgerV1 row.
The old live Validate parent, a foreign or additional live Sign, and every
other live ordinary row still fail closed. The projection stays borrowed with
the installed registry cut, while success and failure retain the exact ledger
frame and move-only Serve authority. Durable open rereads and compares that
complete frame before reconciliation, including on same-context crash reentry.
One consuming `AuthenticatedRecoveredAdapterStartup` factory now owns the
complete first-release branch. Its private exhaustive authority is exactly
`None`, one recovered phase vote, one recovered control Sign, or one recovered
Decision Fetch; no pair of branches can coexist and no caller supplies a
branch or recovery cut.
`ProposalIntent` admits only the current exact `SignProposal`, and
`TimeoutIntent` admits only the current exact `SignTimeoutVote`, after the
terminal frontier is authenticated independently and the latest matching frame,
reducer awaiting-signature state, tag, action, local role, and complete residual
vector all agree. Byte-identical intent records may recur, so reverse scanning
selects the later exact match; later WAL frames may own queued phase signatures.
The control token retains a non-decodable frame identity, exact effect,
canonical V1 replay evidence, and one-shot pending/candidate mint authority.
The runner-owned body store first crosses a move-only fresh-quarantine cut that
rejects any marker already promoted, rejected, or retired; that cut and the
adapter-bound execution/storage seal are the sole production factory inputs.
Queue/archive/event ownership, the local signer, and the cadence authenticated
by signed-genesis or snapshot recovery additionally require a private
runner-minted permit; the lifecycle runner now mints it from the authenticated
runner signer/cadence bundle. The factory never substitutes fresh height one's uncommitted
State placeholder cadence. The signer public key stays comparison-only in the owner,
and launch checks the peer key plus any claimed roster position before gate or
runtime construction.
After residual-effect, startup-instance, State/Kura/network, root, policy, and
WAL checks, the quarantine's one consuming transition applies the recovered-
finality and WAL-authority filters, replays every retained durable marker with
the exact `V2ApplyService` retained by the owner, and seals the store before the
factory opens Certified-Serve or LedgerV1. Only sealed test helpers may reopen
fixture roots. Startup mints authority for only that
current Sign; the adapter and WAL remain retained so the later
lifecycle-completion transition can acknowledge it and authenticate the next
queued signature without prebinding an eager token. The guarded worker result
can project only an adapter-private authority which verifies the exact local
signature and previews `Signed` on cloned reducer/registry state. The preview
classifies the closed result as the mandatory Broadcast alone, Broadcast plus
one already-authorized Sign, or Broadcast plus Proposal's Prepare-intent WAL
request. For the Broadcast-only Vote/Timeout shape, the lifecycle owner retains
the preview beside a staged registry/coordinator successor, fsyncs the Advanced
Sign and live Broadcast as one LedgerV1 frame, then commits only assertion-only
adapter and worker ownership moves. A separate typed refanout claims and
reauthenticates that still-live Broadcast, reserves exact output, changes only
volatile coordinator state to a recovery wait, and then enqueues. LedgerV1
remains Ready so restart reconstructs the output debt, while the volatile wait
prevents duplicate topology fanouts during the current process.
Cold open rejoins that exact row pair to the recovered WAL request, verifies the
signed consensus message against the recovered roster, replays `Signed` on the
cold reducer, and installs only the authenticated Broadcast carrier. Proposal's
body/chunk plus Prepare-intent-WAL cut remains a TODO at the runner boundary and
may not enter the single-Broadcast transaction. The narrower
Broadcast-plus-next-Sign path is sealed and durably publishable for the
already-WAL-ahead Proposal shape.
The reducer-derived next-Vote lookup is consumable only by the launched service
and its exact executor/body-store instance; that join requires the same
validated receipt, durable receipt, recovered manifest, execution commitment,
and Proposal manifest hash. The adapter then consumes the move-only body owner
while authenticating the latest matching WAL Vote. One opaque combined
projection retains the inherited Broadcast candidate and a distinct executable
WAL-derived Sign candidate. Staging allocates adjacent ordinals, preserves
the Broadcast parent owner, assigns the next Sign a fresh causal owner, and
checks exact Effect/Consensus capacity generations. The registry cannot split
the pair before the staged rows match. One transaction reserves Proposal
control plus chunks, fsyncs the exact two-child LedgerV1 successor, installs
both registry carriers and the reducer state, parks only the Broadcast behind
that process-local output owner, acknowledges the worker, and atomically
enqueues the batch. The WAL-backed next Sign remains Ready; the persisted
Broadcast remains the restart output-debt source. Proposal control and
canonical chunks enter one aggregate exact-output reservation under one
corridor mutex. An exact recovered Prepare vote uses the same adjacent two-child
fsync without Proposal output authority: both its Prepare Broadcast and Commit
Sign remain Ready, and the typed refanout driver must own the Broadcast first.
The
preflight charges frozen capacity once, plans both FIFO owners without mutation,
and either commits both or returns the recovered move-only authority unchanged;
ordinary live Proposal broadcast uses the same all-or-nothing planner before
publishing its first-send marker. Recovered reservations additionally bind the
exact body-store instance and output guard. For the initial
`ProposalPrepareWal` shape, the same reservation is acquired before any WAL
I/O; the transaction preflights the sole `PrepareIntent -> Sign(Prepare)`
continuation, fsyncs the encoded intent, reauthenticates that follow-on Sign
against the exact frame and validated body, then fsyncs the adjacent Broadcast
and Sign rows. Capacity failure before the append is mutation-free; every
post-WAL failure is restart-only. Restart retransmission targets the full remote
voter set without mutating live fast-path state. Cold recovery now has a
frame-bound classifier for exactly the
Proposal-to-Prepare and Prepare-to-Commit two-child shapes, independent of
unrelated later high-water, plus an affine WAL authority which replays the lost
historical signature only when the reducer reproduces both durable children.
The control-Proposal cold branch now reconstructs the canonical payload/chunks
from the same revalidated body store, admits the linked Broadcast/Prepare-Sign
pair into the complete census, and advances the adapter. The phase
Prepare-to-Commit branch rejoins its retained Validate/body authority, admits
the linked Broadcast/Commit-Sign pair, and advances the adapter to the exact
Commit-signature fence. Refanout authenticates the full Ready census and
recognizes either pair only through the Broadcast carrier's retained next
address and digest; unrelated Ready work and an unrelated adjacent Sign stay
independent. The initial Prepare-intent WAL transaction feeds those same cold
cuts: a crash after WAL fsync but before LedgerV1 reopens the WAL-ahead pair,
while a crash after LedgerV1 reopens the exact linked two-child census.

The phase-vote path carries the exact LedgerV1 store/frame through repair fsync
and Sign installation. The control and Decision-Fetch paths each project one
payload-free Ready candidate and run one sealed LedgerV1 transaction: an absent
row allocates its ordinal with checked addition and publishes the exact
successor, while an already exact row coalesces without rewriting durable
bytes. Decision Fetch additionally binds `manifest=None`, the complete Commit
QC, frozen ordered roster, and exact authenticated Decision frame. A same-key
row with different record, metadata, geometry, replay authority, or installed
carrier fails closed. All paths authenticate body-backed Ready Fetch only
against the retained frame, install one exclusive WAL carrier beside the
complete Fetch/Serve/Producer census, and consume the registry borrow into the
owner.
`ProductionLifecycleOwnerV1` retains adapter status unpublished after that
complete join, and every successful branch retains an empty residual effect
vector. The consuming owner-to-worker launch must publish the armed status only
after executor, service, and ingress construction succeeds.
Focused BLS fixtures cover Proposal and Timeout repair, exact reopen,
cross-action and tag/frame mutations, extra and dual residuals, foreign replay
authority, unchanged high-water/ordinal on coalesce, and status-last ordering.

Decision Fetch recovery is concrete and fail-closed. A marker still quarantined
for semantic replay cannot cross the body-store startup seal. A promoted exact
success is checked against the authenticated Decision, canonical same-store
body frame, manifest, validation commitment, full context, and process-instance
identity before the frame/index/marker triple moves into one opaque cut. The cut
has no body, manifest, receipt, or parts API; dropping it on any prepublication
failure restores the exact in-memory values without a storage write. A matching
deterministic rejection is an invariant conflict and stops before
Certified-Serve or LedgerV1; it authorizes neither refetch nor Apply. The
successful transaction consumes the sealed Decision-Fetch authority and body
cut, drives only the fixed `BodyAvailable` -> `BodyStored` ->
`ValidationCompleted` reducer sequence, and derives the exact body-backed
Store/Validate/Apply lineage. It accepts only an exact standalone Fetch row or
the complete four-row chain, publishes the exact LedgerV1 successor once (or
stutters without rewriting), installs the sole live Apply carrier, and
reconstructs that same carrier during cold storage open. The carrier retains
the exact validated-body receipt and exposes only a fixed bounded-I/O demand
plus an opaque exact-position dispatch key. The sealed owner launch and typed
worker dispatch now consume that authority without recreating the
runtime-effect ownership sidecar used by ordinary executor work. An Applied
worker result is rebound to the exact claimed carrier, previews the sole reducer
`ApplicationCompleted`, persists the complete terminal LedgerV1 successor, and
only then swaps coordinator/registry/adapter/executor state and acknowledges the
dedicated keyed queue owner. A missing merge sidecar remains a guarded opaque retry
owner and cannot enter generic executor deferral. The keyed completion does not
borrow the complete service stack. Its decided carrier survives generic
executor cleanup per carrier, while a terminal invalid full entry remains in a
separate lifecycle rejection class. Settlement registers the exact
round/subject/reference with the lane journal; the sealed driver then binds the
exact local peer, State, Kura, output guard, and paired service/transport owner,
and may dispatch only the next fair matching request without scanning past
unrelated effects. Its sole retry consumes that owner only after the same local
sidecar is reauthenticated. It reserves the Consensus queue while the exact key
is still `CompletionPending`, and republishes the unchanged task as `Queued`
before disarming either fail-stop guard. Unavailable sidecar or capacity
returns the complete owner unchanged; a queue-owner mismatch closes output for
restart. The serialized runner still
does not call the owner factory; it must consume the sealed launch and its final
observer-activation permit before deleting the independent runner constructors.

The live recovered Decision-Fetch path is closed through its first durable
Store successor. A Ready carrier projects only an
opaque request authority. Exact-output capacity and a vacant, disjoint
executor request owner are retained before the Completion-turn claim; the
post-claim tail installs the carrier key and request/reverse census before the
exact signed fanout becomes visible. An authenticated response is classified
as a distinct recovered selector family. Its Ingress-turn transaction
re-probes the active claimed carrier and exact physical occurrence, claims the
response, and publishes one dedicated body persistence command. Worker
completion stays guarded and keyed as `CompletionPending`; it cannot enter the
ordinary effect-work map or generic completion acknowledgement. If an ordinary
runtime-producing completion is already parked, a following recovered Fetch
completion remains in the physical channel until the older owner is serviced.
Settlement then retains the complete guarded completion while it revalidates
the claimed carrier and body receipt, reserves request/response retirement,
freshly recaptures and locks the exact fair-ingress occurrence, previews the
fixed reducer Store effect, and stages the dedicated registry/coordinator
successor. These and the output fail-stop check all precede LedgerV1 fsync.
The durable successor keeps the recovered Decision Fetch payload `None`, marks
it `Advanced(FetchToStore)`, and gives only the live Store child the exact
`BodyFrame`. The post-publication tail is assertion-only: it installs the
coordinator/registry/adapter state, retires the dedicated request indexes,
removes the prelocked ingress occurrence, removes the worker queue index,
disarms the completion guard, and completes the output operation. Any
pre-fsync error reparks the entire completion and all owners; an fsync error
latches durability failure and requires restart rather than returning a
recoverable error.

Cold storage-only open has a typed recovered-Store branch. It authenticates the
exact two-row Fetch/Store ledger chain, reconstructs the Store projection from
the same fsynced body receipt, includes that live Store in the recovery census,
and installs a dedicated carrier which retains the original WAL Fetch lineage.
If a successful validation marker already exists after the Store publication,
the recovered Apply stager accepts precisely that live Store prefix, advances
the Store to its typed Validate child, and appends the adjacent Validate/Apply
tail above the current ledger high-water mark. A second open exact-stutters;
foreign same-owner history or an exact child-key collision fails closed.

A codec-only `LifecycleReplayAuthorityV1` now defines one closed, bounded
canonical Norito envelope for every one of the 22 lifecycle stages.
Its source families retain exact scalar reducer tags and WAL
sequence/persistence/role/frame-hash identities, signed consensus broadcasts,
local-body, signed-proposal, or QC-origin body authority,
authenticated equivocation pairs, the PrepareQC/rejected-body binding, and the
Certified-Serve payload-store source. Decoding performs complete-input and
byte-for-byte canonical re-encoding checks, while record matching is only a
structural equality check over context, key, class, stage, and durable payload;
it does not mint executable work or replace consensus authentication. Its
fields stay private and decoded values expose no source, encoded, or parts
accessor. The envelope is a required, non-optional field of candidate and
ProducerTurn admission, coordinator durable metadata, recovered records, and
each canonical LedgerV1 row. Admission rejects an inexact envelope before any
mutation; ledger decode/open and WAL repair recheck the complete structural
record binding. There is one first-release V1 format, with no default,
placeholder, migration decoder, or compatibility fallback. Restart still must
reauthenticate each retained source against the verified context and owning
store before reconstructed work can execute.

Certified-Serve and its dormant ProducerTurn now also have one closed,
runtime-only replay-evidence pair. The fresh factory accepts only the exact
authenticated request and its canonical Pending post-fsync receipt; it
recomputes the Pending frame hash and checks request hash, exact QC hash,
payload hash, bounded local-retainer index, QC-signer membership, and active
context before deriving both fixed records internally. Authenticated payload
recovery retains the independently verified local-retainer index and
recomputes the canonical Pending, Completed, or typed-negative frame before a
fixed recovered factory can reproduce the pair. Both opaque values share one
storage origin, expose only exact equality/record predicates, and remain
clone/drop inert. They expose no source, request, certificate, retainer,
payload hash, key, stage, encoded, or parts accessor. Fresh post-fsync and
authenticated recovery projection derive adjacent CertifiedServe and
ProducerTurn candidates with separately encoded authorities from that common
family. Startup keeps the pair whole behind one shared `Arc` retained by both
exact concrete carriers; it never exports or reconstructs either half. Every
nonterminal Serve and every nonterminal ProducerTurn, including the ordinary
dormant `Waiting(ProducerTurn(serve), 0)` reservation, must have its exact
address/slot/digest carrier before the reconciled LedgerV1 is published. A
terminal Serve needs no executable carrier, while a live producer beside that
tombstone still does. This remains true for payload-store-ahead terminal
reconciliation, where the logical tombstone may temporarily retain its former
Pending geometry: the slot-free terminal oracle checks its exact outcome,
payload, metadata, and replay authority without treating that geometry as
executable. The entire Fetch/(optional repaired Sign)/Serve/Producer
census and every batch entry are checked before the first registry insertion;
publication failure removes the complete staged batch and returns its owned
authority.

The production owner also has a payload-first fresh
admission transaction. It accepts only the selector's opaque exact-request
target, the local signer, and the authenticated request. The payload store
first fsyncs a Pending frame and marks abort authority only when that call
created the frame. The owner then derives and stages LedgerV1 plus both exact
shared-family carriers, proves the complete current and prospective census,
and invokes the exact LedgerV1 successor publication while the carriers are
staged. A proven pre-ledger failure may remove only that freshly created
Pending frame; an existing frame, failed abort, or invoked LedgerV1 publication
retains the authenticated tail in a non-decomposable restart-required result.
Exact Completed tombstones replay and exact negative/cancelled tombstones
stutter without a Serve carrier after the same complete current-census check.
Fresh admission now uses one exhaustive coordinator-to-registry census for
every nonterminal row and concrete carrier variant. The census first validates
the complete durable ledger plus key, owner, Ready, and capacity indexes; it
then authenticates Serve/Producer debts and their shared replay allocation,
recovered Broadcast/next-Vote links, and each exact carrier projection before
the new adjacent pair can be staged. The production terminal owner transaction
accepts no payload id, receipt, candidate, ordinal, digest, route, effect,
pending binding, or replay parts. Completion reloads the exact canonical body
through the body-store instance retained by the owner and persists its receipt
through that owner's exact payload store; a typed negative outcome derives the
opaque request id inside the same store. Before persistence the owner
authenticates the exact active Serve lease and signed request. It also proves
both current shared-family carriers and the complete private registry census.
Private-state disagreement is a recovery fault, while caller-owned
lease/request/body mistakes return the unchanged lease before publication.

Once the exact terminal receipt exists, the owner constructs the existing
sealed terminal Serve/Producer replay pair, stages the terminal coordinator
reduction, and publishes the exact LedgerV1 successor while holding the carrier
transition. Ledger failure restores both incumbents and leaves the logical
coordinator unchanged, but the owner remains faulted with its terminal payload
tail and publication authority retained for startup. Ledger success has no
fallible tail: Serve is removed, Completed/Rejected/Failed replace Producer
with the terminal-family Ready carrier, and Cancelled removes both. The raw
coordinator settlement wrappers and raw-id production payload writer do not
exist. Coordinator admission and LedgerV1 debt validation require the exact
key/source/family relation; scheduler execution remains outside this inert
evidence. A future owner-to-worker launch must transfer an authenticated
completion capability bound to the retained body-store instance without
inventing a receipt or parts API. The first release has one V1 carrier and one
exact startup path.

The recovered Prepare/Commit path now performs the first sealed integration.
The verified runtime WAL identity remains non-decodable. It can project only a
separate `PersistedWalFrameLocatorV1`, whose decoder establishes structural
evidence rather than WAL authority. The current recovered vote derives its
PrepareIntent versus LockAndCommit role and Sign action from the latest exact
matching authenticated frame, canonical round-trips the complete V1 envelope,
and retains the resulting opaque evidence beside the move-only vote authority.
Prepare requires an exact view. A queued historical Commit may use the later
current view only while the full selected LockAndCommit PrepareQC remains the
exact active lock lineage. That evidence is cloned only as inert data when the
vote is consumed into `RecoveredWalVoteSuccessor`; exact locator, bounded tag
relation, role, unsigned vote, and PrepareQC lineage are rechecked when the
authenticated lifecycle repair is formed and whenever its concrete pair is
revalidated.
Neither wrapper exposes encoded bytes, locator/action parts, a generic source
constructor, ledger admission, or execution authority.

The certified body pipeline now retains one normalized replay-evidence family
from the selector-authenticated Fetch origin through its closed Store and
Validate carriers. The family owns the exact QC and response manifest once,
plus the exact fsynced body-frame binding; fixed Fetch, Store, and Validate
wrappers transiently reconstruct and canonically compare only the requested
stage envelope. Successor tokens retain the projected wrapper, and a live
Validate-to-recovered-Sign detach moves the same evidence into the sealed
repair before restoring it unchanged on failure. Cold body-store bytes alone
cannot recreate a certified transport origin. The consuming Ready-Fetch cut
instead joins the body store to the exact LedgerV1 replay family and verified
roster/QC, reconstructing only the queue-independent carrier while retaining
the exact storage-authenticated `DurableBodyReceipt`; it never reconstructs or
stores the dequeued response. The separate detached Validate variant likewise
requires byte-for-byte receipt equality rather than a truth sentinel. Digest
bytes, including all-zero frame hashes, have no reserved sentinel value. These
runtime wrappers are non-decodable and expose no
certificate, manifest, receipt, frame, encoded, or parts access; the normalized
family is not independently runnable authority. The sealed certified
Fetch-to-Store and Store-to-Validate transition methods now consume the exact
successor wrapper into a candidate carrying the mandatory persisted field and
BodyFrame, derive its child digest from the sealed one-slot geometry, and keep
the registry token alive inside inert coordinator staging. Raw production
entrypoints for those two edges do not exist; publication remains owner-internal.

Terminal Validate staging now follows the same closed boundary. Inactive and
no-effect adapter branches consume the complete Ready registry/adapter preview;
the registry derives the parent `BodyFrame` from its still-installed completion
and returns only a permit-bound opaque projection. The staged no-successor cut
retains both authority borrows and records `AdvancedNoSuccessor`, releasing the
rejected branch's transient Consensus reservation exactly once. A rejected
report first consumes the Ready preview into the canonical invalid-body replay
seal, then projects its child only while a private non-copyable transition
permit remains nested inside that adapter/registry token. No raw report effect,
pending binding, receipt, or candidate accessor crosses either production
entrypoint; all failures retain the move-only authority token, and both staged
cuts remain drop-inert and unpublished. There is no raw Validate successor
helper: Apply and Sign remain unavailable until their WAL-bound move-only
authorities enter the same sealed transaction, rather than passing through a
temporary effect/pending compatibility surface.

Direct signed replay now has the same closed first-release prerequisite for
exactly `Broadcast` and `ReportEquivocation`. Each non-decodable wrapper can be
minted only from the complete `AdapterEffect` and its exact ordinal-free
`PendingRuntimeEffectBinding`; it canonical-round-trips the existing V1
broadcast or equivocation source while retaining private causal-root and
physical-effect fingerprints. Those fingerprints preserve exact signatures,
message bytes, and equivocation observation order even where the canonical
logical conflict normalizes them. A move-only registry pre-admission token owns
the effect, binding, and fixed class-specific wrapper with no optional or
unsealed variant. The direct adapter boundary now consumes those fixed wrappers
for Broadcast and ReportEquivocation only, and coordinator admission plus
LedgerV1 retain and recheck the resulting inert envelope. There is no raw
constructor, parts or encoded accessor, and zero-valued digest bytes are not
reserved as a sentinel.

Live safety-WAL continuations now have a separate inert exact-replay cut for
`SignProposal`, `SignPrepare`, `SignCommit`, `SignTimeout`, `Apply`, and
`EnterView`. The cut accepts only the reducer's one pending `Persist`, appends
and fsyncs its exact frame, checks the retained frame against the append
receipt and WAL record, acknowledges that persistence identifier, and returns
a closed linear batch. The five payload-free effects carry a non-decodable WAL
source plus a unique pending binding derived from the exact live frame and
complete effect; callers cannot splice a same-effect foreign causal root.
`Apply` instead retains a source-only Decision seal until the fixed registry
join revalidates its installed Validate carrier and receipt, projects the child
pending binding from that exact predecessor, and binds the body frame. A
foreign receipt cannot construct the completion, and a foreign causal root is
rejected before the source seal is consumed. Errors after append latch
restart-required fail-stop; dropping the returned batch or pre-admission token
publishes no work but never rolls back the WAL. LedgerV1 now requires a replay
field, but the live sealed WAL transition has not yet supplied its fixed
envelope to candidate admission; raw WAL stage projection fails closed until
that join is threaded. Zero-valued frame hashes remain structurally valid.

Local proposal bodies now retain a separate pre-intent replay capability from
the active-view producer cut. The runtime consumes a one-shot mint permit only
while binding the exact `AssembleBody` owner to its local `StoreBody`; cloneable
worker tasks never carry that capability. The executor keeps the non-decodable,
move-only seal in bounded sidecars, joins it only to the exact body-store
receipt, projects it through the exact Store-to-Validate pending lineage, and
then retains the same normalized `LocalBody` plus body-frame evidence beside
the exact `LocalProposalReady` command. When that command emits
`ProposalIntent`, its exact effect and runtime owner are consumed into one
inseparable inert composite rather than dropping the companion body origin.
Retries reuse the installed sidecar, failed consuming projections return the
original seal, and validation rejection, superseding view/lock, and Decision
detach retire it explicitly. Detached worker tasks therefore carry no local
replay authority. The mandatory LedgerV1 field exists, but this local wrapper
is not yet joined into body-stage candidate admission; it exposes no origin,
manifest, receipt, pending, effect, encoded, or parts accessor, and zero-valued
digest bytes are valid.

Ordinary remote Proposal bodies now retain their distinct authenticated
origin from the exact signed Proposal ingress to the runtime-owned
`FetchBody`. Direct dispatch carries the signed envelope together with its
frozen receiver ingress; Busy dispatch parks the same opaque origin only in
the bounded deferred-ordinal map and releases it solely for the selected
`ProposalReceived`. Exact effect ownership then attaches a non-decodable,
cryptographically replayable Fetch wrapper only when the effect has the
Proposal manifest, no certified sources, and no certificate. A closed,
move-only work-registry prerequisite consumes that runtime owner and permits
only the fixed Fetch-to-Store causal projection, the exact
`DurableBodyReceipt`/body-frame join, and the fixed Store-to-Validate causal
projection. Every failed projection retains both the incumbent token and the
candidate owner or receipt; dropping any successful token is publication-
inert. The wrappers expose no Proposal, ingress, source, effect, pending,
receipt, encoded, or parts accessor and reserve no all-zero digest sentinel.
The mandatory LedgerV1 field exists, but this remote body wrapper is not yet
joined into body-stage admission, installation, or runner execution.

Invalid certified-body reports now have a closed pre-admission prerequisite.
The durable Validate carrier retains a closed certified-or-remote replay
origin whose Store-to-Validate projection fingerprints the exact Validate
pending binding. The Ready/rejected registry preview supplies its exact
manifest, body frame, effect, pending binding, and canonical rejection identity
only to the adapter's matching staged report. The adapter consumes an
unforgeable move-only registered-Prepare proof, derives either the
inherited-Prepare or ordinary-plus-concurrently-registered child binding from
that exact predecessor, and compares the derived child before
canonical-roundtripping the V1 `InvalidCertifiedBody` source with rejection
code zero. The persisted source retains the exact validation origin: either the
signed remote Proposal or the certified PrepareQC plus manifest, together with
its tag. Structural projection rejects local bodies and any origin, report,
body-frame, or manifest splice. A prepared preview may reproduce its private
proof for internal revalidation, so this is move-only and unforgeable rather
than a claimed linear one-shot mint. The resulting non-decodable evidence,
report effect, child binding, registry borrow, and adapter borrow remain
together in one move-only, drop-inert token. Every failure restores both
previews. No certificate, manifest, receipt, pending, source, encoded, or parts
surface is exposed. Although LedgerV1 now requires an authority field, this
wrapper is not yet joined into report admission, installation, commit, or
runner execution; all-zero frame-hash bytes remain valid rather than acting as
a sentinel.

Rollover is a two-context durable protocol, not an in-memory map clear. The
verified successor snapshot carries its own ledger root and post-fsync
Cancelled payload receipts for every live Serve. The coordinator first removes
the validated batch of unadmitted pending Serve payloads, writes the fully
retired predecessor ledger, then creates or verifies the successor's empty
ledger with the unchanged ordinal high-water, and finally switches the sole
writer. A crash after either write is idempotent: the predecessor can be
reopened as a terminal height and retry the same immediate successor, while an
already-created successor must be empty and carry either zero (an untouched
file) or the exact retained high-water. Pending admission fences are retired by
their exact semantic keys and never allocate rollover ordinals. Every pending
Serve fence contributes its internally retained receipt to that validated batch
rollback; partial filesystem failure latches restart-required durability
failure.

CompleteTip restart retains the complete authenticated Kura finality artifact
and its exact receipt beside the successor activation authority; the recovery
boundary no longer reduces that proof to hashes before lifecycle retirement.
The exact four-row recovered-Decision chain has disjoint live and terminal
oracles: only a nonterminal Apply can reconstruct a carrier, while an
`Advanced` Apply can authorize predecessor retirement only after the retained
Kura proof is joined. The current test-only live-owner rollover reducer is not
production authority. For restart, the runner consumes CompleteTip before
acquiring the ingress permit rather than publishing through a proof-free
bridge. The disk-only transaction joins that proof to the exact opened
predecessor LedgerV1 store and byte-equivalent frame after the four-row join.
CompleteTip authentication first revalidates and retains the
predecessor `VerifiedHeightContext`, then derives private H and H+1
context-addressed lifecycle targets plus the predecessor body-store root and
genesis-or-rotating signature policy from the same Kura instance; an identical
frame copied to another root cannot satisfy the store join. The consuming cut
opens the co-located CertifiedServe payload owner and rechecks all roots,
predecessor PoPs, and the body signature policy against the retained CompleteTip
capability. Its retirement-only Completed-Serve census authenticates the
canonical payload frame, exact request, manifest/body hash, a responder that is
also one of the request QC's certified retainers, its signature, and the
response hash without reopening body bytes that normal finality may already
have deleted. This body-independent proof is accepted only beside an already-
terminal exact Serve/replay family; payload-store-ahead completion can promote a
Pending ledger row only after ordinary recovery reloads and hashes the body. Its
payload-first retirement prunes only
authenticated orphan Pending frames and advances every retained Pending frame
to a receipt-bound Cancelled tombstone. The refreshed cut is rebound to the
same source-frame identity; every live Serve/Producer pair consumes its opaque
terminal replay update, every other live row is cancelled, every prior
tombstone is preserved byte-for-byte, and producer debt becomes empty. H is
published with `persist_exact_successor` and reloaded exactly before H+1 is
initialized at the retained high-water or accepted read-only as a later valid
descendant whose owner and record ordinals are strictly above that floor. The
dedicated move-only retired token is minted only after both rereads and retains
the exact opened H+1 store and frame. A consuming owner binder admits only the
same Kura-derived target joined to the exact verified H+1 context/parent QC,
body-store root and policy, co-located Serve store and its exact post-prune
authenticated cut, adapter startup,
coordinator projection, and complete concrete registry; it exposes none of
those parts. Cut validation rescans the bounded canonical directory before the
join, so a later valid payload written by another store owner is rejected even
when the original in-memory index is unchanged. The sealed launch also joins
its leader-wire and service-owned Certified-Serve gates under one
closed-ingress RAII owner, so teardown cannot detach either durable carrier
family independently. This bound owner is not itself status-publication
authority. Its consuming launch retains retired H beside launched H+1, and the
dedicated activation consumes both only while the runner-owned permit opens the
exact ingress and publishes through the CompleteTip bridge. The lifecycle
runner mints that permit only after binding retired H to its exact H+1 owner. A
restart after either fsync repeats the transaction as an exact stutter without
opening ingress. Uninterrupted live-owner rollover now uses the consuming
activated-height finalization chain described above, and the production runner
consumes that chain directly.
