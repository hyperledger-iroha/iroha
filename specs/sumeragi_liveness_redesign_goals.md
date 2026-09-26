# Sumeragi liveness redesign goals

The async proposal contract is the refactor target: at each height the leader
samples a bounded subset of locally available signed transactions against one
committed parent and signs their chosen order in a proposal. Followers validate
that proposal and execute it deterministically from the parent; they do not
reconstruct the leader's queue, arrival order, or unselected transactions.
Unselected inputs retain local custody for later attempts. Certified QueuePlan
and lane controls are authenticated proposal inputs with their own owners, not
FIFO barriers for unrelated Ordinary work. The queue-selection correction below
implements part of this contract; lane close and Commit still violate the
single-finality-owner target.

Current async-queue counterexample, 2026-09-24: a same-source four-validator
autoscale replay admitted and committed Ordinary work and expanded an elastic
lane, then committed an empty-lane drain intent at height 8. After one planned
peer shutdown, the other three validators continued committing global blocks
past height 14, but all three retained the same close intent with no drain
certificate or retirement. The test now reaches this phase without requiring
a prior lane block: an expanded lane with no work must also retire. The
production `NativeRunnerProcess` owns the active lane reducer but has no
drain-vote, drain-certificate, or certified-merge producer;
the old `V2LaneWorkAdapter` contains those operations but is not constructed by
the runner. Complete the drain protocol under the sole process-lived lane
owner, including retained signed output and exact carrier attachment, then
prove close-to-retirement under four- and seven-validator loss, reordering,
backpressure, and restart. A local asynchronous Ordinary queue must neither
block that certificate nor bind execution to its admission-time lane. Its
signed bytes remain durable while the leader derives a bounded proposal route
from committed State. These are open acceptance conditions, not a release claim.

The leader's bounded Queue scan no longer treats a certified
`QueuePlanSynced` input, its live reservation, a held selection lease, or an
exact durability transition as a FIFO wall for later local arrivals. Each
owner still excludes its own signed transaction; the candidate assembler keeps
QueuePlan execution in its autonomous corridor while sampling later Ordinary
inputs. This removes one concrete async-queue stall. Conflict checks against
certified reservations and whole-batch execution preflight remain open. The
leader may use local FIFO for fair sampling, but block validity and forward
progress must depend on the signed proposal and committed parent, not the
local position of an unselected transaction. Preserve the reservation's exact
ownership and fee capacity while qualifying this change.

The leader now rechecks the chosen Ordinary subset's aggregate fee reservations
against the same committed parent used for individual admission. A selected
input that would overbook payer, sponsor-window, or relay-lease capacity is
deferred without consuming its Queue owner; unrelated selected inputs may fill
the candidate. Unselected local arrivals hold no capacity in this proposal.
This is a conservative admission bound, not a full sequential execution proof
or a replacement for follower validation. The two-input fee-capacity
regression, all 49 candidate-assembly tests, all 387 Queue tests, the Core
library check, and the multilane source-binding gate passed on the preceding
source cut.

The leader's bounded Queue snapshot and candidate assembler now exclude a
local QueuePlanSynced copy before looking up its registry binding or routing
it. That copy belongs to the authenticated carrier and autonomous lane owner;
its local arrival, stale registry projection, and unavailable route cannot
veto a later independent Ordinary input. The bounded scan cursor advances past
an excluded copy, so a one-entry window can reach later work on the next
attempt even if a global block commits between those attempts. Committed
parent changes no longer reset the local cursor to the same excluded head.
The carrier still authenticates its own complete admissions and
rejects duplicate entrypoints. This removes a local queue-to-proposal coupling;
it does not resolve the separate lane close/Commit deadlock or prove network
liveness.

On the current `optimizations` source, all 388 Core Queue tests, all 49
candidate assembly tests, all 423 QueuePlan contract controls, the multilane
structural source-binding gate, and the optimized daemon build pass. The focused
release-inventory owner and include-manifest checks passed before the last
test-only and Queue source correction. The registered test rename is reflected
in the release inventory and its exact digests. A four-validator mixed-arrival
run passed in 66.66 seconds: all four Ordinary inputs and the QueuePlan input
were Applied on every validator; node logs show Ordinary execution before the
QueuePlan control. The corrected seven-validator fixture also passed the same
exact-status assertion on all seven peers in 126.87 seconds. Repeat runs then
exposed two unresolved conditions: a four-peer global-status polling run
stalled at height 6 after progressing through height 5, and a seven-peer run
lost a validator at height 1 when a retained Ready Validate successor received
`UnexpectedPlan` and tripped the consensus output guard. The regression now
reads peer-local state-resolved finality to avoid a global Torii fanout per
missing hash; that cut passed once on four peers in 55.97 seconds. A scheduler
change to prioritize an authenticated physical Validate completion ahead of
unrelated Ready rank, while retaining it when its own capacity is unavailable,
is under qualification. These runs do not close the
no-fault repeatability or loss/reordering/restart matrix. The broader proof-ledger gate
still reports old-adapter `cfg_attr` restrictions and extensive fixed-scaling
source-inventory drift; those failures are not liveness evidence.

A 2026-09-24 four-validator autoscale run exposed a second local-custody
counterexample at committed height 6. A newly expanded lane closed before an
uncarried QueuePlan input reached the global ledger. Nexus revalidation treated
the resulting inactive route as corrupt durable ownership and failed the
validator's whole Queue, even though the same Queue already had an exact,
fsynced terminalization path for unadmitted claims after authenticated close.
The current candidate routes this case through that terminal path, retaining
selected and reserved claims until their owners release, while preserving the
draining authority of canonically admitted inputs. Both focused close tests
pass; a four-peer close-overlap run and broader fault qualification remain
pending.

The active runner now admits `LaneRelayMessage::DrainVote` to the process-lived
Native collector. It authenticates the remote signer, committed frontier and
embedded close committee without consulting its own asynchronous Queue or
Kura. Three exact votes from a four-validator close committee aggregate into
one certificate in the focused Core test; a mismatched transport sender and a
signed frontier outside committed State are rejected. The relay and collector
tests, production Core library check, and formal source-binding gate pass on
the current `optimizations` working tree. This is receipt and aggregation,
not issuance or delivery: no validator yet signs and retransmits a Native
drain vote, and no leader carries the certificate in a global proposal. The
authenticated network ingress retains each vote in a bounded relay; the Native
owner checks the transport signer, signature, committee, and committed frontier
after dequeue. The exact-output service now returns a signed vote to its source
when its per-target and shared capacity are full, without tripping the
fail-stop guard. This is a retained-delivery primitive, not an active Native
vote producer. The Native owner must keep and retry its original signed vote
across that refusal and across ordinary asynchronous message loss.
inactive adapter still contains the retired merge-candidate route and must
not become a competing owner. The Native cut still needs retained vote
delivery and a leader-proposed retirement control validated from the
committed parent. As a separate queue correction, an
unbound Ordinary journal row no longer vetoes lane retirement when its local
claim index is absent or stale; actual reservations and globally admitted
controls continue to block it. The retirement protocol and network proof
remain open.

The first Native signer boundary is now connected: the process-owned State/Kura
pair opens one shared drain/Commit guard, and the actual `LockAndCommit` WAL
signing path fsyncs its immutable value lock before creating a Commit
signature. The guard takes the already-authenticated Native opening capability
instead of rechecking committee proofs during every signature. Same-value
retries in later voting views are allowed; conflicting
same-height values and a drain below the signed frontier are rejected across
restart. The direct restart test, all 13 guard tests, and the production Native
Commit dispatch regression, Core library check, and formal binding gate pass on
`22846b4f74`. This is a safety prerequisite, not a working drain protocol:
the active runner collects authenticated drain votes but does not issue or
carry a certificate. The remaining candidate, Apply, and geometry paths still
consult receiver-local Kura evidence, so they cannot yet
be treated as deterministic validation in an asynchronous network.

The old merge validation path is also unsuitable for this async protocol:
`State::validate_merge_lane_drain_certificate_payload_with_releases` checks
receiver-local Kura and pending-work evidence when it has no replay proposal,
and `V2LaneWorkAdapter::accept_lane_drain_vote` checks the receiver's local
Queue before accepting an authenticated vote. Two honest validators can have
different queues and recovery schedules at the same committed parent. The
Native replacement must separate a validator's conservative local condition
for **issuing its own vote** from deterministic checks of an incoming vote or
leader-proposed certificate. The latter checks must use the signed evidence,
close-committee identity, frontier, and committed parent, never receiver-local
unselected work.

The target retirement flow has one global ordering step. After the close
intent commits, the Native process stops admitting new work for that exact
incarnation and retains previously signed Native decisions until they either
apply or are proven obsolete. Each close-committee validator derives the same
frontier from committed State and durable lane evidence, durably locks its
vote, and gossips it with retry. Any peer may aggregate exactly `2f + 1`
matching votes from the exact `3f + 1` close committee. The global leader
samples the available certificate as a proposal control alongside a bounded
local transaction subset; followers validate it against the committed parent
and the block's global QC orders it. The certificate must not require an
additional view-bound merge-signature round just to retire an empty lane.
Receiving or aggregating a valid drain vote must not wait for the receiver's
local transaction queue or source-recovery schedule. Only issuance of that
validator's own vote checks its retained Native WAL and unresolved lane-owned
effects. The old adapter currently rejects received votes when its local
blocker predicate is true; that rule cannot be carried into the Native owner.
Changing the current merge-entry commitment layout and Kura replay is part of
this migration, so the existing certificate-only merge tests remain a safety
baseline until the new block path has equivalent replay and fault tests.

The close/Commit intersection is an unresolved protocol deadlock, not merely
a missing timer. In a four-validator lane, two validators can durably sign
Commit for height `h` before a QC or global application is available. If the
global close commits with replicated frontier `h-1`, their signing guards
correctly refuse a drain vote below `h`; the other two validators cannot form
the required three-vote drain certificate. A naive local vote producer would
leave both the lane decision and retirement without a reachable next action.
Before enabling issuance, the first-release design must either make that
pending value globally resolvable at close or remove provisional lane Commit
as a separate finality authority and return its work to global leader
ordering. Prove the chosen resolution under lost QC/body messages and restart;
an empty-lane happy path alone cannot qualify retirement.

The simplification target is one global finality authority. A lane may attest
to signed payload availability and retain a prepared candidate, but a
lane-local Commit signature must not make economic work irreversible before a
global block QC orders it. At close, the global leader either proposes a
recoverable pre-close payload or releases its signed transactions for later
bounded selection. The close and that disposition are committed in the global
chain; no separate quorum vote asserting the *absence* of an unseen lane QC is
needed. This is a target contract, not the current implementation. The present
lane Commit and drain certificate paths remain until the shared global owner,
recovery, deterministic Apply, and four/seven-validator fault tests replace
them as one reviewed cutover.

The first-release ownership rule is simple: a signed Ordinary transaction is
asynchronous local input, so queue arrival order and admission-time routing are
not consensus facts. At each turn the leader samples at most `max_queue_scan`
local inputs, resolves each against one committed parent State, and proposes a
bounded executable subset. A temporarily unavailable route defers that input
without blocking independent later inputs; the next turn may retry it. The
proposal and its parent commit to the selected bytes and routing, and every
validator checks those facts before voting. QueuePlan admissions, lane decisions,
and drain certificates have separate authenticated global ownership; their
exact committed/certified identities must not be inferred from a node's local
queue. Retirement waits for those lane-owned obligations, never for unselected
Ordinary input. The remaining code still stores Ordinary admission-time routing
hints in the plan journal, so removing that extra owner is an explicit refactor
milestone rather than a completed architectural claim. Journal replay now
authenticates the original signed bytes and context even when both the old hint
and current route are unavailable. It retains FIFO custody without assigning
execution authority or a fee reservation; proposal and gossip defer until a
committed single route exists, and candidate admission is checked then.
The leader's bounded snapshot now rechecks current fee, manifest, authority,
privacy, and compliance admission for each signed external Ordinary input before
signing, retaining an ineligible input so later sampled work can proceed. This
also rejects aggregate fee overbooking within the selected Ordinary subset,
but does not prove that the batch executes successfully in sequence. The leader
now binds selection and admission to one State generation, then excludes State
publication while it rechecks the parent and signs. The start-of-block work
probe finishes before that publication lease because it acquires State
writers; selected Queue ownership is narrowed before the fail-stop private-key
operation. Any unsigned
preparation error observed across a changed State generation defers the attempt,
and a locally lagging or already-superseded parent height also defers. A
same-height hash conflict, wrong network, Queue durability fault, or armed
signing failure remains fatal.
Native source preparation checks the committed parent before body/Decision I/O.
A local predecessor that is behind or already superseded, or a State publication
racing an unsuccessful source read, yields a retained retry rather than a fatal
worker error. A same-height conflicting parent and a stable source error remain
fatal. The original Decision handoff stays with the process-lived Native owner
through this pre-preparation deferral.
Aggregate execution preflight remains open;
a fresh route and individual admission checks do not establish whole-block
executability.

Validation on 2026-09-24 before the shared checkout advanced: all 386
`queue::tests` and all 49 `sumeragi::v2_candidate::tests` passed, including
State movement during preparation and an already-committed successor height.
The checkout then advanced to merge commit `22846b4f74`. On that revision,
the rebuilt Core binary passed all 49 candidate tests and three focused
closed-route Queue regressions. The multilane source-binding checker, all
101 admission-capacity contract
tests, nine focused in-flight binding tests, and three focused Native
history-owner mutations passed. The exact-revision `iroha3d` build passed,
followed by four-validator real-network passes for
`four_peer_async_ordinary_queues_commit_leader_snapshot` and
`four_peer_simultaneous_queue_plan_collectors_make_progress`. These ingress
results do not qualify autoscale retirement, adverse delivery, or restart; the
missing Native drain certificate and merge carrier remain release blockers.

Current production-adapter candidate, 2026-09-22: retained validation has bounded
candidate descriptor slots and a finite shared allocation pool covering those
slots, two concrete World journal shell sets, the retained effects `Box` and the
candidate phase `Box`. The original shell/effects reservation moves with the
actual executed carrier through capture, decision binding, physical refusal,
publication and final destruction. The descriptor and phase allocations retain
their own charges for their actual lifetimes. Decision binding and physical
preparation add no artificial `B`/`I` admission callbacks or reservations.

This is scoped memory admission, not complete process-memory accounting. Nested
maps and values, execution/event payloads, current/undo COW copies, tiered
snapshots, geometry/archive projections, canonical wire encoding and decoding
still need complete allocation accounting at their actual creation and release
boundaries. **Complete process-memory admission remains an outstanding goal.**
The finite shell pool does not prepay these allocations or establish a process
memory ceiling; the historical allocator milestones below do not imply otherwise.

The production ownership requirement is unchanged: retain the actual source,
execution, exact finality and original State/Kura/Queue owners, preserve the same
carrier on local refusal, and publish once without rerunning execution. Cache and
recovery markers cannot replace that owner. The current production runner connects the process-lived Native reducer,
Decision candidate, State-owned economic execution and original publication/Apply
settlement. This source connection does not establish network qualification.
Validate the runtime handoff, original Apply settlement and recovery integration,
preserve all protocol and deterministic correctness gates below, and qualify
the unchanged four/seven-validator campaigns. No L1–L6 outcome or release is marked complete here.

The [retired-height ingress and observer-recovery correction](../docs/history/2026-09-23/stale-height-consensus-ingress.md)
removes a fatal relay outcome for authenticated finalized-height evidence,
replaces a stalled CommitQC discovery fanout on retry without losing the signed
request, and admits a maximum-size complete input through its bounded signed
relay. The subsequent [Native source-observation refresh](../docs/history/2026-09-23/native-source-observation-refresh.md)
keeps the original validation owner across concurrent State publication. One
captured candidate passes silent-author, nine-peer observer, same-subject,
distinct-subject repeat, four/seven-validator restart and NPoS leader-rotation
networks. An earlier distinct-subject run on that same candidate timed out in a
controlled FIFO drain, and its exact wait remains under investigation. The full
unchanged fault campaign remains open; these corrections do not close a
redesign goal.

The subsequent retained-Validate candidate durably cancels an exact
pre-execution Waiting row once finalized State passes its proposal height,
including while the original first-input recovery wait is parked. The two
ledger/carrying-owner tests and guarded driver-completion test pass;
the unchanged daemon and harness pass one run each for silent author,
same- and distinct-subject quorum release, nine-peer slow-observer pressure,
four- and seven-validator restart, and NPoS stopped-leader rotation. Those
runs do not prove the superseded Validate cut occurred in a network. The
controlled-drain timeout is a separate open counterexample; the full unchanged
fault campaign remains open.

The Native lane deadline projection now excludes an overdue clock while its
original instance lacks the effect reservation needed to enter the reducer or
has an unacknowledged completion. The runner uses its existing bounded idle
wake while physical completion or retirement can free that same reservation.
The saturated-clock Core regression and the real-owner silent-initial-author
regression pass; this is scheduler-pressure coverage, not network qualification.

The process-wide Native source slot now retires a signed Validate body request
when its exact lifecycle wait (subject, execution index and source allocation)
has been superseded. A candidate body request is tied to the authenticated
current frozen lane head; lane closure retires its redundant network request
and prunes old candidate waits, while an already authenticated buffered body
is settled for any other current route sharing that first admission. A moving
State observation cannot prove closure. Retiring either request cancels only
its unadmitted network-actor ticket. The focused three-case source-closure
selection passes, but these cuts still need unchanged multi-peer loss and
restart qualification.

Current retained-execution prerequisite: [admitted Storage restoration](../docs/history/2026-09-21/admitted-storage-restoration.md) now reconstructs exact current/undo images with per-edit funding and generic borrowed history. Replacement and ordinary blocks retain one original pool and both writers inside their callback. [Publication identities](../docs/history/2026-09-21/funded-publication-identities.md) now share that original admission, retain successor storage before execution and refund old versions only after publication unlock; exact startup and writer demands include those layouts. [Scoped detached capture](../docs/history/2026-09-21/funded-detached-storage.md) now retains original prepaid journals without writers and borrows the original refund scope for reattachment; native capture/abort signals follow both writer releases. [Executing-block abandonment](../docs/history/2026-09-21/executing-block-abandonment.md) now shares the prepared pair's original writer owner through ordinary/admitted opening, replacement, execution and snapshot restore; both locks release before callbacks and actual mutex poison survives payload and wake unwind. [Native reader readiness](../docs/history/2026-09-21/native-reader-readiness.md) now binds BlockHashes refusals to the actual blocking reader mutex and retains published native notifications through retirement. [Complete component preparation](../docs/history/2026-09-21/prepared-component-retirement.md) now retains native prepared map/Cell owners and identity before transfer, with successful World/runtime/membership cleanup held through State fences and original State/Kura/Queue completion notifications retained until commit unlock. [Fully acquired carrier abort](../docs/history/2026-09-21/prepared-component-abort.md) now keeps original component cleanup through all fences on explicit abort and Drop. [Returned preparation refusals](../docs/history/2026-09-21/prepared-refusal-cleanup.md) now return original MV/runtime/TriggerSet/World cleanup through the enclosing consumer. [Prepared runtime/trigger abandonment](../docs/history/2026-09-21/prepared-aggregate-abandonment.md) now releases all sibling writers before callbacks on Drop and unwind, retaining both original capacities. [Successor hash admission](../docs/history/2026-09-21/successor-acquisition-readiness.md) now binds the actual fresh writer before callbacks and waits on the native reader mutex when reader observation is contended. [Fresh MV pair construction](../docs/history/2026-09-21/fresh-pair-acquisition.md) now owns both actual mutexes and notifications before either constructor or policy can unwind. [Cell pair custody](../docs/history/2026-09-21/cell-pair-custody.md) now retains both raw guards before cloning and keeps joint release through complete Block/CurrentReplacement abandonment, detachment and publication-lock refusal. [World acquisition and abandonment](../docs/history/2026-09-21/world-acquisition-custody.md) now retains caller-owned field slots and releases all siblings before cleanup; its candidate validation is recorded separately. The [World/TriggerSet capture candidate](../docs/history/2026-09-21/world-capture-custody.md) now retains original slots and deferred notifications through all sibling captures; its qualification is separate. The [State capture correction](../docs/history/2026-09-21/state-capture-custody.md) composes those original World slots with all runtime and membership writers through refusal and unwind. The [State acquisition candidate](../docs/history/2026-09-22/state-acquisition-custody.md) now keeps partial acquisitions and executing blocks jointly armed through refusal and unwind; its qualification remains separate. The [direct State publication candidate](../docs/history/2026-09-22/direct-state-publication.md) passes its consumer test-target check; the [retained hash correction](../docs/history/2026-09-22/retained-hash-preparation.md) passes 636 distinct Core controls, 474 native unit tests and the canonical structural gate in their captured scopes. The complete participant candidate below covers sibling preparation. Complete process-memory accounting and production qualification remain open under the scoped admission boundary above; concrete nested World payloads, native control storage, decoding and aggregate execution/restore admission are not funded by the shell pool. The original Apply service already survives startup; unfinished-height recovery must retain its original executed owner and the named allocation charges before exposing validation markers. The later State effect-lock and aggregate cleanup records describe their actual custody corrections. This does not activate the retained production validator or close L1–L6.

Current participant custody work: the [source-coupled record](../docs/history/2026-09-21/participant-publication-custody.md) describes the original-Kura readback boundary and distinguishes remaining scalar AMX consumers from the new Native Decision path. The latter owns its sealed application markers and does not require the old participant representation. Retire that representation with its remaining consumers; do not reopen rejected MergeQC execution. Scoped checkpoint133 validation does not close L1–L6.

The [retained participant checkpoint](../docs/history/2026-09-22/retained-participant-preparation.md) joins 643 Core and 355 MV controls on unchanged input. The following [complete participant candidate](../docs/history/2026-09-22/complete-participant-preparation.md) passes 649 Core, 355 MV and 10 documentation controls with matching captured Rust inputs, plus the canonical structural gate. Its 194 distinct focused formal controls pass across preserved run/follow-up scopes. Complete resource admission, remaining notification boundaries, live cutover and unchanged network qualification remain open; no L1–L6 outcome is closed.

The [State effect lock correction](../docs/history/2026-09-22/state-effect-lock-preparation.md) moves original index acquisition before direct and retained publication visibility and retains notification custody through drain validation and replacement rewind. Its 778 Core and 21 Torii controls pass with matching source; seven-package test compilation passes. Source-contract and structural qualification retain their distinct captured scopes in the linked record. Complete resource admission and production/network cutover remain open; no L1–L6 outcome is closed.

The [membership prerequisites](../docs/history/2026-09-22/membership-allocation-prerequisites.md) add one funded typed buffer and the native owned-generation allocation floor, with both MV layouts and corrected 884-control Core scope qualified. Integrate the original finite pool before membership construction, capture and restore; a late committed-state capacity error is not a progress design. The [indexed-publication and maintenance correction](../docs/history/2026-09-22/native-indexed-publication-recovery.md) extends the [partial-write repair correction](../docs/history/2026-09-22/native-repair-partial-write.md) through actual initial publication and completed latest-pointer maintenance; 1,004 selected Core, 21 Torii and 165 copied formal controls pass in their recorded scopes. The [authenticated map lookup prerequisite](../docs/history/2026-09-22/authenticated-map-node-lookup.md) now checks externally owned nodes and distinguishes proved absence from local read/corruption failures. The [bounded external update kernel](../docs/history/2026-09-22/authenticated-map-node-updates.md) now preserves original roots through failed path writes and passes 20 map controls plus Core compilation. The [membership root owner](../docs/history/2026-09-22/membership-root-publication.md) now binds both current and rollback cuts to the actual component publisher, rejects foreign/stale/recreated owners. Its [authenticated height reader](../docs/history/2026-09-22/authenticated-membership-values.md) now uses the [single located map kernel](../docs/history/2026-09-22/authenticated-map-locations.md); 26 map and 79 membership controls pass with all prior selectors retained. Explicit root/child/value locations avoid requiring a global content index while preserving exact logical commitments and old references. The [fixed Norito record codec](../docs/history/2026-09-22/fixed-membership-records.md) now passes all 88 membership controls and the full default Norito suite, with allocation-observed framing/validation. The [original funded append/range prerequisite](../docs/history/2026-09-22/retained-membership-append.md) now retains exact replay, ambiguity, sync, physical accounting and deferred cleanup, with 1,248 Core, 651 Config, 21 Torii and 18 focused formal controls joined to the final candidate. Complete State enrollment, authenticated restart and retained-generation reclamation remain required; a fresh value location cannot repair an old retained reference. Both full-State Apply checkpoints must be replaced together under consumed complete-journal publication authority; a copied prepublication hash cannot bridge the released commit-lock interval. The [post-Apply capture identity correction](../docs/history/2026-09-22/post-apply-checkpoint-binding.md) checks the actual captured network, height and block before metadata writes; its 263 Core, 651 Config and 21 Torii controls pass in the recorded scope, with the independent DA failures kept explicit. The next State/Kura membership milestone is authenticated durable lookup at the exact original State cut and incremental checkpoint construction before resident-cache cutover; the current production Apply path materializes the full membership history twice per fresh height, so a bounded lookup cache alone is insufficient. Missing or untrusted history must produce a typed local refusal, never a non-membership answer. Qualify eviction/cold restart, sealed aliases, old/replacement cuts and Kura-before-WSV interruption before that cutover. These corrections do not close L1–L6.

Current integration location: `/Users/takemiyamakoto/dev/iroha`, branch
`optimizations`. All ongoing source integration and validation use this checkout;
no other branch or worktree is authorized for this work.

Set: 2026-09-16. Overall goal: **Active**. Implementation and qualification are
open. Starting source: `b2e4c86cc2586038ea16e94de3d7aefe94ea4040`.

Eliminate recurring internal causes of lost progress in first-release Sumeragi:
circular waits, competing scheduling decisions, lost asynchronous results,
incomplete durable reconstruction, and permanent errors retried as temporary
conditions. Replace unnecessary machinery rather than adding another coordinator
or preserving obsolete APIs, wire layouts, or storage formats.

This record owns the redesign's execution order and acceptance criteria. The
previously deferred generic lifecycle simplification is now an active
prerequisite for consensus runtime qualification. Existing
[multilane goals](sumeragi_v2_multilane_completion_goals.md) and the
[closure ledger](sumeragi_v2_multilane_closure_ledger.md) retain their feature
and release obligations; this plan discharges none. The
[August simplification baseline](sumeragi_v2_lifecycle_simplification.md)
remains historical measurement evidence, including its deletion obligations.

## Design boundary

The implementation already has a protocol reducer and a lifecycle coordinator.
Remove independent decisions around them; do not add a third scheduling authority.

| Owner | Authoritative facts | Required simplification |
| --- | --- | --- |
| Pure protocol reducer, `v2_core/reducer.rs` and `v2_core/wal.rs` | Frozen context, safe proposal, lock, vote/timeout intent, decision and application | Keep one safety transition relation with explicit durable acknowledgements. Remove redundant recovery facts only when their surviving source reconstructs them. |
| Existing launched lifecycle owner/coordinator | Work identity, ready/running/waiting/terminal state, capacity, result custody and reconstruction | Sole authority for executable work. Every wait names an actual dependency and a reachable wake event. |
| Runtime ingress and worker adapters | Authenticated messages, opaque jobs, physical queues and results | Authenticate and execute; derive eligibility from authoritative state instead of maintaining competing scheduling state. |
| Kura, body store and safety WAL | Canonical durable artifacts and validated persistence receipts | Reconstruct work from those artifacts; simplify duplicate journals only after preserving all crash boundaries and bounded recovery. |

The leader samples the finite set of eligible inputs already available at a
proposal boundary, orders that sample deterministically, and proposes. Async
arrivals after the sample remain for a later proposal. A local admission
collector may wait for peer receipts, but it must never reserve the only
receiver capacity needed for those peers to serve one another; admission
backpressure must not create a network-wide resource cycle before the leader
can take its sample. Peers in an asynchronous network need not have identical
queues at that instant; they agree on the signed proposal's ordered contents
and validate and execute those contents deterministically.
Kura can store the next block before State publishes its corresponding view.
Authenticated peer publications that arrive in this overlap wait for the State
height notification, then reclassify the same bytes against the new frontier;
the fixed synchronous reconciliation probe is not a validity verdict. The
bounded publication worker retains the input while waiting, and its sender's
durable copy remains available if the worker deadline expires.

The first direct-input cut changes `AccountTransactionDraft::new` to sign an
`Ordinary` intent. Torii durably queues signed single-route application work
without preproposal QueuePlan quorum collection. The leader's bounded sampler
selects an ordered local snapshot without waiting for matching peer queues or
lane Decisions; the signed global proposal fixes the order. Peers validate the
signed inputs and route contexts from that proposal and execute them through
the common State path without lane payload ownership. A ready Native batch and
ordinary work receive alternating height opportunities, while an exact-height
lifecycle control takes its height immediately and retains its own quorum check.
Explicit multi-route and special private-settlement carriers retain
`QueuePlanSynced` custody. The failed four-validator autoscale replay before
this cut admitted 0/96 load submissions under 429/503 QueuePlan pressure.
Focused Core/Torii checks, the four-peer direct-input replay, and the same-source
daemon build are required before claiming this cut works; the full unchanged
four/seven-validator campaign and L1–L6 remain open.

The runner's former independently transitioned
`LifecycleProducerClaimDispositionV1` in
`crates/iroha_core/src/sumeragi/v2_runner/lifecycle_height_driver.rs` now comes
from a fresh permitted-action projection of the launched owner's actual state.
Its nine variants include real successor ordinals and wait tokens, and the
historical transition oracle remains test-only until owner-fixture coverage can
replace it. The projection is read-only, uncached, and cannot mint work.

Next consolidate separate deferred-owner, timer-episode and schedule decisions
in `v2_runtime.rs`. Ordinary production dispatch must use the surviving owner
after each change. Moving the same state to another file, adding bypasses, or
running two implementations behind a toggle does not satisfy this goal.

### Lane consensus must use the same safety transition relation

The retained four-validator run `network-checkpoint-05` exposes an additional
protocol defect. All three surviving validators commit the admission-only
global block at height 5, then persist timeout intents and certificates for
global height 6, views 0 through 6. The authenticated lane frontier is height 1;
its deterministic height-2 author is the stopped validator. There is no lane
payload, proposal, or pre-payload timeout owner. The existing lane NewView
mechanism requires an already published immutable payload and only changes its
retransmission cursor. Global view changes cannot replace that initial author.
This counterexample is within the four-member committee's fault bound.

Replace the lane-specific partial safety protocol with instances of
`v2_core::Reducer`. Preserve independent lane progress, exact lane committees,
Native AMX, authenticated availability and global exactly-once application.
The replacement must include these inseparable boundaries:

1. **Frozen context before payload selection.** At the end of a canonical
   global carrier's execution, open a lane instance only when its exact route
   has admitted pending work and no instance already owns the next frontier.
   Freeze the route, incarnation, predecessor and its actual application
   height, opening carrier height, committee/PoPs and policy. Resolve authority
   at that carrier's height on the completed staged state; do not assume the
   next block's key promotions or recompute its pre-frozen epoch election.
   Later global heights, views and local queue arrival order cannot reset an
   open instance. Empty genesis or lifecycle-created lanes need no instance
   until admitted work exists. After frontier advancement, remaining pending
   work opens the next instance under the same deterministic rule. Authenticate
   the complete bounded open-instance set, including absence and closure, in
   the signed execution commitment. Preserve it through snapshots/compaction;
   an application-height locator alone cannot reconstruct historical policy.
   Ordinary, autonomous and Native participant frontier projections all enter
   this rule. Instructions may persist the actual source-bound proposal hash.
   Output roots, executed-wire hashes and current finality must never feed back
   into proposal-bound inputs or the State projection they certify.
2. **One safety owner.** The shared reducer owns proposal intent, Prepare and
   Commit intent, lock, Timeout intent, installed TC and Decision. Persist its
   complete transition before signing or announcing the successor. Remove the
   synthetic NewView vote/cache/cursor safety authority and independent Commit
   slot fence when the replacement enters production; do not keep two engines.
3. **Origin and voting round are distinct.** An exact executable payload and
   its reservations remain immutable when a higher-view leader reproposes it.
   Native signature preimages and certificate checks bind the frozen context,
   current voting round and exact origin. The voted value also commits the
   canonical RS16 layout, chunk root, exact byte length and chunk count through
   a domain-separated availability hash. A Commit QC must authenticate those
   fields without fetching a separate origin proposal; payload-hash equality
   alone must not admit a substituted manifest. A TC selects its highest PrepareQC;
   an unlocked successor can build a new payload. A local timeout or changing
   global view alone never authorizes a new origin or releases reservations.
4. **Timers precede proposals.** Replicas with an admitted lane obligation can
   durably time out a silent author without any payload, payload-dependent
   clock, or extra client transaction. Clocks, quorum traffic, body recovery,
   persistence and completions retain bounded service under lane saturation.
5. **Recovery and application remain exact.** Recover the same context, lock,
   durable intents and pending effects across restart and global advancement.
   A lane Decision makes an exact source available for the global carrier;
   economic WSV changes and QueuePlan terminal settlement remain owned by that
   carrier. Prove reservation release for losing *unlocked* origins separately
   from retention of locked/decided payloads.

Input admission and economic execution have separate global carriers. The
admission carrier must retain the exact typed executable input, while the later
execution carrier commits its actual execution base and results. Electing
another author from local FIFO contents cannot substitute for that immutable
input. The shared-reducer design avoids a second consensus algorithm.

This work is part of L1–L5, and the retained `network-checkpoint-05` run remains
an L6 failure. The current shared reducer has a pre-payload timeout owner;
source controls cover silent-author progress without a global view change.
Those controls do not replace unchanged real-process qualification. Exercise
silent/equivocating authors, partial payload delivery, competing locks, restart
cuts, cross-lane progress and committee reconfiguration.

The current production runner constructs the process-lived Native owner, polls
it independently of ingress, consumes authenticated Decisions through the global
candidate assembler, executes the State-owned economic source and settles the
original Apply through actual publication. The [earlier transport evidence](../docs/history/2026-09-20/native-candidate-and-transport.md)
and [physical ingress evidence](../docs/history/2026-09-20/native-ingress-rollover.md)
record prerequisites; their then-inactive status is historical. Preserve the
original allocation, bounded ingress accounting and global-rollover custody in
this connected path. Qualify its retained Validate-to-Apply handoff, interruption
recovery and finite-input progress on real four/seven-validator networks. This
source connection closes no L1–L6 outcome by itself.

The [publication completion seam](../docs/history/2026-09-20/native-published-apply.md)
now binds that original Apply to actual State publication, allowing different
authenticated quorum signer subsets for the same immutable value. Runtime
publication also checks original current/undo allocation identity from before
detachment through abort and publication. Driver progress leaves retired effects,
packets and returned body results with the original capacity-accounted instance;
an explicit consuming handoff preserves that custody through closure. Actual
publication now authorizes terminal consumption of drained original owners and
separately transferred closed bodies. Pending and fsynced local Decisions must
authenticate the published value; held Apply still requires its real reducer
settlement. Refusal returns the same armed owner. Accepted opening now shares
one immutable context across body jobs and retirement. World preparation retains
the original wrapper/vector allocations across refusal and abort; capacity
owners outlive retained payloads during unwind, preserving explicit poisoned
writer recovery. The private publisher now completes captured nonretiring geometry
under its original Kura lease before visibility and consumes retained lifecycle
effects inside the same generation. Its State/header and Queue requirements remain
explicit; retirement/replacement cannot proceed without original service Queue
custody. These prerequisites do not supply the live retained validator's resource
policy or activate the Native runner.

The [Queue selection-release correction](../docs/history/2026-09-20/replay-terminal-queue-release.md) retains already-authenticated canonical cleanup on the original admission when local selection or a popped guard delays removal. Actual owner release resumes the exact journal tombstone and retirement notification without another Apply. Autonomous claims remain under their checked direct-release or Kura Complete authority. Pending pre-Kura batches remain in the adapter until checked release succeeds, including the failed and unvisited suffix. These corrections restore local continuations; they do not reproduce a live runner stall, make local Queue emptiness a consensus-validity rule or complete the shared drain protocol.

The [retained capture and publication owner](../docs/history/2026-09-20/retained-capture-and-target.md)
now preserves incomplete archive capture, validated, decided and checkpointed
journals inside the existing candidate slot. Capture refusal returns that same
owner before exposing its typed local dependency; no marker authority exists
until original capture completes. `SelectedValidationCarrier::try_consume` can
restore the current publication phase without reconstructing a ValidBlock, and
lends the original producer for call-local State/Queue access. Journal admission
exhaustively borrows the complete candidate, including original deferred-record
and event capacities; this input surface does not supply allocation funding.
Identity and ready commitment derive from original journals; descriptor
reservation precedes execution. The captured State/header now authenticates the
physical target before derived persistence. Incomplete capture keeps its existing
boxed allocation through local refusal; ready phases skip consuming resume.
Build107 passed the seven-package build, 192 runtime regressions and 67 focused
source-contract controls on unchanged inputs without a stack override. These
checks do not activate the live path. Next carry this owner through the worker's original Apply
service and State/Queue pair, including pre-launch recovery ownership.
Physical writers must be released before retaining a refused phase.
Cold unfinished recovery must create that executed owner once before restoring
cached markers; already-applied recovery must repair durable completion without
executing. Resource admission, retirement and participant completion remain
prerequisites; a phase enum alone does not discharge them.

### One economic admission and lane execution pipeline

**Decision: implement; migration and evidence remain open.** Every network economic
entrypoint must use canonical durable QueuePlan admission, globally committed
admission and pending obligations, one frozen lane instance of the shared reducer,
and canonical global application. Native participant controls use that same
instance's safety authority. Retire the independent ordinary economic candidate
and Native signing corridor; do not add a second slot fence. An explicit Ordinary
fixture or direct-genesis construction may remain, but cannot authorize a
production network economic bypass. Time and consensus-system inputs retain
explicit internal ownership and cannot carry arbitrary external executables.

This replaces real production paths. Public single-transaction and entrypoint
submission and the public batch handler require signed `QueuePlanSynced`
(or the exact certified key-lifecycle exception); generic Torii adapters still
need migration. M3's strict
admission, autonomous progress, grouped/mixed-role Native settlement, exact-once
application, bounded recovery, and mandatory signed RS16 remain requirements.
Admission's existing `f + 1` durable-storage certificate is distinct from the
exact `2f + 1` Prepare/Commit/Timeout quorum in an exact `3f + 1` committee.

**Retain the input in its admission carrier.** Each
`BlockExecutionContextBundle.queue_plan_admissions` control now contains one
canonical `LaneAdmittedInputV1`: the exact entrypoint and its binding/certificate.
The routing plan is reconstructed from that binding rather than duplicated.
Before registry staging, the complete-input decoder verifies entrypoint and
signed identities, plan/context and the original journal-claim digest against
those exact bytes. Certificate-only responses cannot enter this boundary.
The first-admission rank then locates the immutable input in its finalized
carrier. A later lane committee uses existing authenticated global block and
RS16 recovery; it does not require a new post-admission journal-body protocol or
continued availability of retired admission custodians. Local reservations and
pre-admission durability remain owned and must still drain exactly.

Pending Kura custody and P2P publication must carry that same complete source
before admission, and carrier retention/snapshot/bootstrap must retain its
recoverable body while obligations remain open. Enforce exact encoded-input
feasibility against control, global DA and lane DA bounds before durable
acceptance. The current one-MiB control/four-MiB aggregate limits were chosen
for certificates; simply appending bodies without reconciling producer and
receiver bounds can accept impossible work. Preserve bounded scheduling and
typed rejection before acceptance. Complete-input custody and per-control sizing
are implemented; complete global/native envelope feasibility before acceptance
and the shared lane runtime remain open.

#### Canonical admission priority and shared-slot AMX composition

`state/queue_plan_priority.rs` owns the first-release State registry record:
the exact pre-admission binding claim plus `(actual carrier height, zero-based
index in that carrier's strict registry-key-ordered admission vector)`.
`StateBlock::stage_queue_plan_admissions` assigns this rank only on the first
successful atomic registry insertion. Exact replays retain the original bytes
and rank, even at a different position in a later carrier. Existing claim readers
decode this one ranked record; a rankless signed claim is not accepted as a State
marker. Pending obligation and route-member markers refer to the registry owner
instead of copying rank. Index gaps are valid when earlier vector entries replay
already-admitted claims. Original signed proposal height and local durable FIFO
ordinal are not global priority: a certificate can be committed after its source
height, and FIFO order differs between replicas.

Global uniqueness belongs to the single pristine carrier staging boundary:
all prior ranks precede H, and one complete strict unique-key vector assigns
distinct indices at H in an atomic marker transaction. The retained control
bytes must equal the carrier's complete native controls. Production rejects a
second staging call on that overlay; a private same-H replay also rejects a
changed original index. A later-H replay may occupy another input position while
retaining its earlier rank. Route readers reject duplicate positions within the
route, relying on this carrier-owned construction for global uniqueness rather
than scanning every historical registry record.

The fresh ordered/head reader binds the exact route, immutable binding owner,
all pending members, signed aliases, original rank and an authenticated carrier
height. Its immutable eligibility cut is the frozen context's opening global
height. It accepts a completed staged carrier H whose hash journal remains H-1;
snapshot validation supplies its committed H. A structural rank or raw scalar
height alone never grants voting authority. Frozen contexts now pin the exact
head binding hash and rank. Finalization and publication retain an open instance
only while that same head remains unresolved at that frontier. Global terminal
resolution of the head closes it and may open a new instance at the same lane
height for remaining work, with a new opening height/context identity. Dropping
a candidate overlay cannot cancel the committed instance; local absence or a TC
cannot clear a protected lock. The native adapter still must authenticate the
current finalized set before enforcing this cancellation boundary.

Before any irreversible lane proposal/Prepare, an atomic group must be the
oldest unresolved admitted group on every affected route. Coordinator and
participant roles on one route share one value. Any wait for another group then
points to a strictly smaller immutable rank; a finite graph of those waits cannot
cycle. Independent disjoint routes remain concurrent. The simplest initial
batch policy is one atomic group per affected slot; batching requires an exact
common ordered prefix on shared routes. Availability failure must fetch the
eligible group, not skip it for a later conflicting group. This ordering argument
does not replace fair scheduling, available bodies, live quorums, global apply or
native/formal validation of the complete protocol.

The existing `V2LaneWorkAdapter::autonomous_native_coordinator_for_view` elects
one Native coordinator by global height/view, and remote sender validation
enforces it. It currently prevents competing autonomous batches from splitting
participant claims. Keep that gate until the shared-reducer consumer enforces
the replacement rule. A three-route design counterexample explains why routing
alone is insufficient: X on A/B/C (coordinator A) and Y on B/C (coordinator B) can
otherwise decide A:X, B:Y, C:X, then each group waits for the other's held route.
This respects minimum-dataspace coordinators; it is not a reproduced production
AMX stall. Rank codec/staging/replay/cut/adverse controls pass in the bounded
foundation checkpoints; complete runtime and AMX qualification remain open.

#### Entrypoint and producer migration inventory

Torii callsites below are in `crates/iroha_torii/src/routing.rs` unless another
file is named. The entrypoint owner is
`crates/iroha_data_model/src/transaction/signed.rs`; the execution owner is
`crates/iroha_core/src/tx.rs`. Changing an unsigned draft must happen before fee
quotation, caller/local signing, idempotency hashing and durable admission.
Never relabel or re-sign caller-signed bytes during submission.

| Capability and current owner/callsite | Required migration and preserved behavior |
| --- | --- |
| Public `External` submission: `lib.rs::submit_signed_transaction_for_ingress_queue_plan_certified`, `execute_torii_transaction_via_proxy`; `handler_post_transaction_entrypoint` | Retain exact bytes, caller authority, fees, executable variants, routing, durable acceptance and indeterminate outcome classification. Make this admission service reusable by all producers, including asynchronous transport from adapters currently using synchronous local queue helpers. |
| `SealedCommitment`: `TransactionEntrypoint::admission_intent` currently returns Ordinary; `tx::validate_sealed_commitment_stateless` and `validate_sealed_transaction_commitment` | Give the exact signed first-release commitment layout mandatory QueuePlan semantics, through its fixed protocol meaning or a required signed field. Preserve secrecy, authority, commitment identity, expiry and commit-before-reveal. A user commitment is admitted state work, not a general global-only exception. |
| `SealedReveal`: `TransactionEntrypoint::admission_intent` inherits its inner signed transaction; `tx::validate_sealed_reveal_authentication`, `validate_sealed_transaction_reveal` | Admit the exact outer entrypoint and inner signed identity; preserve authenticated routing, pre-block commitment checks, conditional execution and signed replay-alias retirement. Route any revealed Native executable through the same lane instance. |
| Asset transfer: `submit_asset_transfer_request` → `handle_transaction_with_metrics` | Bind intent in its transaction builder before fee quote/signature verification; retain transfer semantics, prepared/direct forms and exact response identity. |
| ISO 20022: `lib.rs` submission branch → `routing::handle_transaction`, after `runtime.sign_transaction_payload` and `bind_transaction_hash` | Select intent before the internal fee quote and signing. Preserve durable message context, message/transaction idempotency, reserved hash and partial-write/indeterminate admission handling. |
| SCCP bridge proof/message: `handle_post_bridge_proof_submit`, `handle_post_bridge_message_submit` → `handle_transaction_with_metrics_and_routing_plan_sync` | Preserve proof validation, direct/prepare modes, fixed routing, response preflight and submitted hash. Prepared output and direct signer must agree on intent before signatures exist. |
| Multisig: `handle_post_contract_call_multisig_propose/approve`, `handle_post_multisig_cancel/propose/approve` → generic transaction helpers | Preserve user approval quorum, proposal identity, cancellation and immediate-execution routing. Migrate shared draft/quote/sign helpers and submit owner; user multisig approval is separate from lane consensus quorum. |
| Account recovery: `execute_account_recovery_mutation`; prepared onboarding/faucet: `handle_v1_accounts_onboard_submit_prepared`, `handle_v1_accounts_faucet_submit_prepared` | Preserve authorization, exact prepared payload/hash, account registration and issuance economics. These are service/user transactions, not consensus-system artifacts. |
| SoraFS: `handle_post_sorafs_register_manifest`, `handle_post_sorafs_register_capacity_declaration`, `handle_post_sorafs_record_capacity_telemetry` | Preserve provider signatures, authority, instruction semantics and bounded responses; migrate signing/draft and admission together. |
| Public batch: `lib_pipeline_handlers.rs::handler_post_transactions_batch` → the shared `prepare_fresh_transaction_ingress` / `submit_prepared_transaction_ingress` owner | Implemented bounded complete decode/signature/route preflight before dispatch, exact per-entry durable admission, 202 only for complete acknowledgement, and ordered 207 results after partial admission. Original hashes and rejection/ambiguity codes survive in Rust and JavaScript clients. Authentication precedes canonical retry; current custody is refreshed before charging authority quota. The independent local atomic writer and synced-intent rejection were removed. Runtime and formal evidence are scoped separately; carry this contract through full qualification. |
| Alias prepared plans: `lib.rs` alias-plan `TransactionPayload` construction currently selects Ordinary | Bind mandatory intent before canonical payload sizing, fee quotation and prepared-plan hashing. Preserve exact alias leases, permissions, dependency checks and caller-signed plan identity. |
| Core Ordinary economic candidate: `sumeragi/v2_lane_work.rs::CandidateWorkProvider::prepare`, especially `prepare_native_participant_controls` and `prepare_native_receipt` | Replace its production economic path with admitted autonomous work. The existing code rejects synced candidates but still constructs Ordinary Native controls; both paths must converge on the same frozen slot before any signing. Retain every native role/group/source/settlement predicate. |
| Queue and gossip: `queue.rs::push_with_lane_with_state*`, `QueuePlanGossipAdmission`, `gossiper.rs` owned/shared receive paths | All network economic admission, peer gossip, follower validation and replay require the same immutable admission owner. An unbound local journal entry can await certification but cannot obtain a lane reservation/signature or execute through an Ordinary fallback. Keep byte/capacity bounds, FIFO ownership and crash-safe promotion/retirement. |

The generic data-model `TransactionBuilder::new_with_time` currently defaults
Ordinary. `routing.rs::quote_app_api_transaction_builder` and
`sign_app_api_transaction` are shared responsibility points, but changing them
alone misses raw payload constructors and already signed prepared requests.
Each producer must retain negative tests for changed intent, fees, authority,
network, executable and hash; submission cannot repair a mismatched signature.

`Time` has no public-admission exception: `AcceptedTransaction` rejects direct
Time ingress and transaction-admission execution. Keep scheduled time triggers
and transaction-caused data/pipeline triggers inside deterministic carrier
execution. Audit their descendants and execution ordering so moving the parent
transaction to an autonomous source neither drops nor duplicates a trigger, nor
runs its economic effect outside the globally committed application. Genesis
retains its exact `TransactionDomain::Genesis`, signed authority, staged
committee/PoP checks and bootstrap; network-domain transactions cannot claim it.
NPoS effects, beacons, admission CAS, merge references, lifecycle/drain and DA
metadata remain typed global controls with their existing predicates and RS16.

#### Client, wire and prepared-draft owners

This inventory names the required owners, not a request for duplicate codecs or
blanket fixture rewrites. Retain canonical Norito layouts and Rust-owned positive
and negative fixtures; change expectations only with the corresponding capability.

| Owner | Current responsibility to reconcile |
| --- | --- |
| Rust: `crates/iroha_data_model/src/transaction/signed.rs`, `signed/builder_construction.rs`; `crates/iroha/src/client.rs` | Data-model builders default Ordinary and genesis validates it; Client drafts default synced. Reconcile network construction, exact offline drafts, sealed layouts, required fields and signature/intent downgrade controls. Preserve `signed_model_tests.rs`, `client.rs` draft tests and shared transaction fixtures. |
| JavaScript: `javascript/iroha_js/src/transactionCodec.js`, `toriiClient.js`, `norito.js` | Canonical submission checks synced intent; prepared/fixture inspectors also contain explicit Ordinary expectations, including privacy exact12 in `norito.js`. Distinguish offline fixture validation from live economic signing, preserving transaction parity and `ordinaryDraftInspector.test.js` assertions under the new named contract. |
| Swift: `IrohaSwift/Sources/IrohaSwift/TransactionEncoder.swift`, `ToriiCanonicalTransactionDraft.swift`, `ToriiClient.swift`, `ToriiPreparedAccountTransactionsV1.swift`, `SccpClientModels.swift` | Encoders and some prepared routes already bind synced intent; other exact draft checks require Ordinary. Migrate each live producer and its expected signed bytes together. Preserve `TxBuilderTests`, canonical unsigned support, prepared-account, contract, SCCP and transaction parity tests. |
| Kotlin/JVM and Android consumers: `kotlin/core-jvm/.../core/model/TransactionPayload.kt`, `tx/TransactionBuilder.kt`, `tx/norito/TransactionPayloadAdapter.kt`, `client/HttpClientTransport.kt`, alias/SCCP draft adapters | Payload model defaults Ordinary while builder signing selects synced; both draft accept sets exist. Preserve exact decoder tags/limits, prepared payload identity, submission and fixture parity. Kotlin remains the sole implementation for Java consumers; migrate their existing assertions without creating a Java surface. Keep JDK 8 API enforcement and Android separation. |
| C#: `csharp/src/Hyperledger.Iroha.Sdk/Transactions/{TransactionAdmissionIntent,TransactionBuilder,UnsignedTransactionPayload}.cs`, `Torii/ToriiClient.cs` | Builder emits synced, while specific unsigned-draft validation still expects Ordinary. Preserve exact enum/wire checks, offline quote/signing, `TransactionTtlContractTests`, `ToriiClientTests` and Norito fixture parity. |
| Python: `python/iroha_python/iroha_python_rs/src/lib.rs`; `python/iroha_torii_client/client.py` | Rust-backed builders bind synced; pure client exact prepared-payload checks include Ordinary and synced routes. Preserve both client families' supported capabilities, canonical bytes, submission outcomes and shared fixtures; do not introduce a second consensus codec. |

Existing source tests explicitly demonstrating Ordinary production behavior,
such as `candidate_provider_admits_ordinary_work_in_multiroute_world_and_excludes_queue_plan_synced`,
need replacement assertions for the same economic capability through admitted
ownership, plus rejection of the old bypass. Preserve batch tests including
`handler_post_transactions_batch_rejects_invalid_ed25519_precheck_without_partial_push`,
sealed routing/replay/pre-block authentication tests in `tx.rs`, owned/shared
certified gossip tests, Native grouped/mixed-role application tests and all
endpoint authorization/idempotency assertions. No test count alone proves this
migration complete.

#### Post-block context commitment and merge ordering

Open a lane instance from exact **remaining** post-block pending obligations,
not from local FIFO or immediately after an idle frontier. Proposal-native
admissions stage on pristine `StateBlock` in `block.rs::state_block_for_execution`.
Current-height activation, transactions/triggers, autoscale and AXT ratchets then
execute. Both sequential and parallel transaction paths resolve terminal pending
owners before returning; autonomous preexecution resolves them in
`State::preexecute_merge_execution_sources_into_with_replay`. Exact pending route
members derive all coordinator and participant `route_incarnations`, so admitted
Native work is included. Their bounded exact member roster is authority, not a
count/XOR summary. After migration no independent Ordinary Native obligation may
acquire the same slot.

A shared finalization boundary before `StateBlock::capture_exec_witness` must
close completed contexts, retain still-open contexts byte-for-byte, and open
successors only for remaining admitted work. Include ordinary/autonomous frontier
cells and the exact validated Native participant frontier projection. Native
application metadata currently stages later in `apply_without_execution_inner`;
that timing alone is not a defect and must not be treated as proof that it belongs
in the transaction witness. Consume its authenticated manifest/projection where
needed, preserving the separate exact application receipt boundary.

Resolve a new committee and aligned valid PoPs at opening carrier height H on
the final staged state; copy the authenticated H context's epoch, mode, policy
and signed RS16 layout. `resolve_lane_committee_at_height` reads current sources,
not historical manifests/stake. It also does not execute H+1 promotions. Global
`next_epoch_snapshot` is frozen from pre-boundary state; post-H transactions
cannot retroactively alter it. Use effective staged Nexus/manifests, including
pending lifecycle updates. Empty static/genesis/new-incarnation lanes open no
context until admitted work exists. Global height/epoch changes cannot retag an
open context, discard its durable lock, or delete the signing custody required
for it. Lane/key retirement and replay-terminal closure need exact authenticated
open-instance/custody resolution, including when pending work becomes empty.

Closure ordering must distinguish the applying carrier's pre-state from the
resulting post-state. Validate a lane decision against the exact frozen instance
in its global carrier's pre-state; that carrier may legitimately consume the last
pending obligation and remove the instance in its post-state. Once that exact
complete-set commitment is globally finalized, authenticated absence or a changed
instance identity fences later signing, broadcast, certificate consumption and
economic Apply for the removed instance. A concurrently prepared candidate must
still pass the existing exact global parent/state-generation publication gate
before any economic delta commits. Do not reject a carrier's own valid Apply
merely because its resulting post-state closes the lane instance.

Global closure supplies cancellation authority, not a separately remembered
cancellation flag. Retained workers/results must finish exact cancellation and
acknowledgement before their reservations are released. Keep durable lane votes
and locks keyed by the immutable opening instance across crash recovery; a later
opening at the same lane frontier cannot retag or reuse them. Native adapter and
wire consumers must enforce these boundaries before closure is complete. Retain
private signing keys required by unresolved frozen instances through key rotation;
public PoPs alone do not preserve signing custody.

Writing a WSV cell before capture does **not** automatically authenticate it:
`QueuePlanMarkerStorage` delegates raw storage insertion, and the execution
witness records explicit reads/writes. `StateBlock::capture_exec_witness` now
adds one compact complete-set commitment at the fixed lane-context witness key:
version, network, actual carrier height, context count and the Merkle root of
canonical frozen-context hashes. Exact full contexts remain in State and its
required snapshot field. Count and canonical route ordering bind the complete
set, including its explicit empty value; per-context inclusion proofs must use
that exact count/root. The synthetic write enters
`ExecutionCommitment.ordinary_writes_root` and `post_state_root` through
`sumeragi/exec.rs::execution_commitment_from_projection`, including the defined
Kagemusha composition. The staged/finalized Kura finality sidecar now retains the
mandatory fixed-write sparse proof alongside its existing finality material;
loading checks its root, network and exact carrier height. This preserves the
opening commitment across restart without embedding a many-MiB context set in
every witness. No current carrier self-hash belongs in that preimage. A native
opening consumer must still authenticate finality, this fixed-write proof and
the requested context's inclusion before constructing voting authority; present
membership alone does not prove a different instance's absence or closure.

Autonomous merge has an additional inseparable constraint: batch construction
and `stage_certified_merge_entry_with_replay` seal the complete World economic
overlay, and `validate_staged_merge_execution_authorization` rechecks it before
voting. The selected ownership is dedicated State/MV storage for the complete
`LaneConsensusContextsV1` set, outside World transaction storage. Its explicit
synthetic witness write authenticates the global-derived consensus metadata in
the carrier execution commitment without changing or excluding any economic
merge-overlay writes. StateBlock owns reconciliation; StateTransaction does not
mutate this set. Both committed and staged snapshots must include the required
field, including the explicit empty set. Capture seals the exact value, and
output/publication rejects any subsequent mutation. A pristine preexecution
helper cannot assume later block activation/effects have already happened;
reconcile only after the exact staged frontier and pending-work transitions.
Storage, pinned-head reconciliation, compact commitment and sidecar proof
retention are implemented in the current candidate. Their combined foundation run passes 106 selected Core controls and 58
data-model consensus tests with no ignored cases on 2,185 unchanged local inputs
(checkpoint 07). This includes the authenticated current-set reader and native
BLS/QC/TC/WAL projection; no replacement driver or network pass is claimed. The reader
copies the complete set and opening hashes under a coherent State view, releases
all guards before Kura reads, verifies current-set inclusion and historical
opening authority, then rechecks the publication generation and tip. Its token
is an observation, not perpetual signing authority. Historical opening finality
absence must be an explicit storage error rather than an unowned indefinite
wait; snapshots/bootstrap must supply the retained opening artifacts and exact
header/wire associations. Lane reducer consumption, closure custody, durable
artifact migration and complete native/formal qualification remain open.

The subsequent canonical data-model ownership and physical native WAL boundary
passed 110 selected Core tests and 66 model tests on 2,189 unchanged inputs
(checkpoint 11). The native WAL control uses three of four real BLS keys, the
authenticated State/Kura opening, actual descriptor-relative fsync, restart
before acknowledgement, TC retransmission after reopen and catch-up of the
previously silent fourth reducer without a payload. It also rejects a foreign
local key and complete-frame corruption. This is an adapter boundary test, not
a production network run. The subsequent availability-hash correction passes all ten affected Core and
nine model tests, with zero ignored tests and 2,189 unchanged inputs. The native DTOs use canonical data-model schema names; earlier
unreleased checkpoint signatures/WALs are not migration inputs.

The admission ownership extraction compiles with 2,626 unchanged captured inputs
and passes 454 Core and ten model tests. Torii passes 57 of 58; the preserved
failed public missing-transport fixture signs Ordinary intent and never reaches
its intended boundary. Its signed intent is corrected in the next candidate,
which also retains the original input with the admission certificate and checks
the per-control size before dispatch/journal ownership. That candidate is not
yet tested; complete signed global/native DA feasibility remains outstanding.

Repeated native PoP verification across full-set validation, hashing, capture,
extraction, commit and snapshot restore is an open cost concern at the existing
1,024-lane bound. Prefer constructing an immutable fully validated authority
value once and then hashing/sealing its exact bytes. Any optimization must retain
strict validation at untrusted decode/native construction and must not introduce
authority caches keyed only by height.

Replay must derive identical context state and compare the resulting commitment
to verified CommitQC. Snapshot storage/checkpoints preserve WSV bytes, but a
portable frozen-context proof still needs the finality-bound snapshot projection;
current registry reconstruction is not that proof. Test missing/extra/changed
cells, changed PoPs/roster/policy/layout, foreign opening carrier, same-slot
reopening, post-boundary activation, pending-to-terminal closure, unrelated
merge-overlay mutation and crash cuts. The old `MergeLaneAuthorityCatalogV1`
contains peer rosters, not PoPs/policy/RS16, and does not cover ordinary/genesis
carriers; it cannot substitute for the complete new authority.

### Runner ownership inventory

These are observations of existing custody, not independently mutable runner
states. The completion classifier may release a lease before the worker returns;
an empty lease is insufficient evidence for `Eligible`.

| Previous runner disposition | Authoritative source for the replacement read |
| --- | --- |
| `Eligible` | No retained completion, blocking lease, unacknowledged lifecycle I/O, exact Ready live Apply or terminal Apply receipt. Ready work remains selected by the existing coordinator. |
| `AwaitingValidateSuccessor` | Launched owner's move-only `ReadyValidateSuccessorV1`, including its exact ordinal. |
| `AwaitingValidateFence` | That same token's reducer-fence wait, with source identity and nondecreasing generation enforced by the token itself. |
| `AwaitingLiveApplyQueue` | Executor's exact live Apply key plus a fresh registry attestation of its Ready carrier and Validate parent; no copied parent/child history. |
| `AwaitingCompletion` | Launched retained result, a non-Apply worker lease, or unacknowledged Validate/certified-body persistence in the worker's indexed custody. |
| `AwaitingValidateSidecar` | Launched registered sidecar dependency. It keeps its authenticated response and pacemaker paths serviceable. |
| `AwaitingApplyCompletion` | Coordinator's exact Apply lease or launched retained deferred Apply completion. |
| `ApplyTerminalSettled` | Executor's durable finality completion whose owner is the exact lifecycle Apply, rather than generic readiness. |
| `AwaitingReplayCompletion` | Worker index for terminal Serve replay, retained through queued, active and completion-pending states until exact acknowledgement. |

Validate dispatch (`begin_durable_validate_dispatch`) and ordinary certified-body
persistence (`plan_ingress_turn_with_runner_debt`) release their lease into an
external wait before worker admission. Their indexed I/O remains a Completion
obligation. In contrast, recovered network `FetchDispatched` registers an
external response dependency and permits unrelated work. Apply, Sign and
recovered-Decision body persistence retain the claimed lease. A terminal Serve
replay never creates a new lease, so its worker index must distinguish it from
claimed Serve work. These cases must be tested at actual ownership handoffs.

### Wait and wake inventory in progress

The coordinator's four `WaitSource` variants are not a complete runtime wait
inventory. Their concrete production meanings currently include:

| Wait | Retained authority and wake | Progress condition to qualify |
| --- | --- | --- |
| Logical capacity (`Consensus`, `Effect`, `Serve`, `Producer`) | Coordinator accounting; `release_capacity` advances that class's generation after exact settlement. | The releasing completion/output must remain serviceable while admission is full. |
| Certified body response | `certified_fetch_wait_source` binds the signed request hash; the registered response and persistence handoff wake that exact record. | Passive network acquisition releases its execution lease and cannot block response ingress or pacemaker work. |
| Durable Validate result | `durable_validation_wait_source` binds coordinator ordinal, physical slot, statement, exact frame and manifest; worker custody persists until publication and acknowledgement. | Generation changes cannot discard a still-relevant result or grant stale signing authority. |
| Missing Validate sidecar | `RegisteredLifecycleValidateSidecarWaitV1` retains exact reference and result acknowledgement; lane work authenticates availability from Kura before waking, or durably cancels a superseded owner. | Lane ingress, recovery and permitted pacemaker work remain enabled while ordinary work is fenced. |
| Reducer fence | Context/height-scoped `reducer_fence_wait_source`; current adapter generation wakes the retained successor. | The exact fence completion and authentic progress must run before dependent retries. |
| Recovered output/finalization | Recovery digest plus retained output admission; finalized output handoff and durable row retirement complete custody. | Actor admission alone is insufficient to retire the durable obligation. |
| Adjacent Producer turn | Coordinator's exact predecessor ordinal and handoff barrier. | A completed Serve cannot admit an unbounded new Serve sequence ahead of its Producer turn. |

Physical worker slots, output permits, P2P writer capacity, Kura persistence and
runtime deferred/clock ownership still require complete cross-owner mapping.
The runtime now borrows a fresh bounded owner census from executor pending maps
and retained batches at every affected arbitration and ingress reconciliation
entry, including timeout freezing and retransmit-root reuse. The cached owner
vector and configure/set/publish protocol have been removed. Exact identity
validation and the pending-work plus two-batch bound remain enforced, and
passive network Fetch ownership remains outside the runnable-owner census.

The September 23 local four-validator diagnostic additionally identifies an
Apply lock-handoff failure: source authentication releases Kura before witness
and archive persistence, and a queued exact advert-tip reader can repeatedly
preempt its next try-only acquisition. The current correction retains one
original joint Kura lease through source authentication, guarded witness/archive
writes, final proof authentication and Queue/State/component acquisition. Every
refusal still returns the original decided owner after joint guard cleanup.
L4 requires a coordinated reader regression that proves the same release
observation remains pending through all phases; L5/L6 require a fresh matching
source network run. All seven lease/drop regressions pass within the unchanged 356-control Core
selection, including unlock-before-admission cleanup on drop/unwind and the
complete participant inventory. The matching local four-validator run applies
and authenticates genesis on every peer. After switching the setup assertion to
the bounded account-identity producer, the test reaches the actual author
outage: all three survivors admit the sole finite input at global height 2, but
execution finality exceeds the 180-second bound. Retained real-peer logs expose
the raw P2P classifier rejecting Native control tags 11/12 before authentication.
The correction, explicit nested decode bounds and bounded descriptor retirement
pass all 303 captured Core controls. Matching daemon50/harness51 clear the raw
wire rejection, then expose a height-3 economic rejection because carrier time
ignored the Native input creation time after an idle interval. Assembly and
validation now share the exact retained Network-input clock floor, recomputed
after prefix trimming. Matching daemon54/harness55 now successfully execute the
sole input on all three survivors and authenticate its later-view Native quorum.
Restart of the stopped author then fails at genesis: the retained publisher wrote
finality without the checkpoint-bound commit manifest required by strict recovery.
The correction writes the original captured checkpoint and authenticated manifest
before finality, preserving the existing interrupted-tip recovery cuts. All 530
Core63 controls pass on an unchanged final source/artifact join. Matching
daemon64/harness65 clear strict Kura replay, then expose a nonempty signed-genesis
lifecycle frame incorrectly excluded from physical predecessor retirement. The
correction admits that exact authenticated physical frame under genesis
policy while retaining the full owner/Serve census and strict missing-frame
exception. All 574 Core66 controls pass with an unchanged final source/binary/Git
join. Matching daemon67/harness68 pass the four-validator silent-author,
sole-input and genesis-restart diagnostic in 115.45 seconds. Seven-validator
two-restart validation fails after successful Native execution because historical
replay still selects the generic Native-rejecting validator. The current repair
shares Native source-owned preflight and recorded execution with historical
replay, preserving its isolated-State finality/wire/checkpoint tail. All six
new publication/replay and snapshot controls pass within unchanged Core75's full
580-control selection. Matching daemon73/harness74 fail earlier at height rollover:
a whole-process ingress cut rejects process-lived Native custody. The separate
authenticated closed-global cut now passes its genuine retained-Native retry,
physical-owner, corruption and caller regressions. Matching daemon91/harness92
pass the seven-validator two-restart diagnostic (298.30 seconds) and the
four-validator silent-author/sole-input/restart diagnostic (127.05 seconds) on
unchanged captured sources and binaries. Core93's fieldwise retained-journal
materialization correction passes all 137 focused controls, including the four
former default-stack failures and a real Native retry on an explicit 2 MiB
thread. Core96 corrects the diagnostic source assertion and passes all 810
controls. The subsequent [Native/global queue correction](../docs/history/2026-09-23/native-global-ingress-ordering.md)
removes a reproduced terminal-drain dependency on retained Native custody and
keeps replenished Native traffic from starving a global recovery dependency.
Final Core109 passes all 878 selected controls with an unchanged source, binary
and Git join; all 100 contract/mutation controls and the canonical multilane
structural gate pass. These structural checks do not prove runtime liveness.
The final daemon/harness pair passes the real four-validator NPoS outage/restart,
seven-validator two-restart and four-validator silent-author/sole-input/restart
checks in 226.64, 320.76 and 113.16 seconds. The first two precede only a test
correction; final daemon/harness bytes are identical and each original source
capture remains explicit. Two earlier NPoS fail-stops remain causally
unattributed. Complete process-memory admission, successor activation, the
remaining fault/seed campaign and clean signed release qualification remain
required. Earlier evidence remains in the
[epoch-authority record](../docs/history/2026-09-22/epoch-authority-cutover.md).

## Required invariants

1. **One obligation, one owner.** Every accepted consensus obligation retains
   one owner until completion, authenticated supersession, or a diagnosed
   terminal failure. Transitions conserve ownership and bounded resources.
2. **Every wait has an enabled wake path.** Waiting for response, persistence,
   capacity or validation cannot exclude the event which releases that wait.
   No operation waits for a permit or capacity held by itself or its joining
   caller. External waiting releases execution leases it no longer needs.
   Physical queue order remains authenticated, but does not create a dependency
   between independent lifetimes: global finalization cannot wait for Native
   custody that survives the height. Independently eligible traffic shares fair
   service with the responses needed to release a blocked global owner.
3. **Progress service survives ordinary saturation.** Clocks, authenticated
   progress messages, required responses and completions receive bounded service
   even when ordinary work fills its queues. Untrusted traffic cannot claim
   that service without authentication or starve admissible work indefinitely.
4. **Results outlive execution attempts.** Semantic result custody is separate
   from the job generation. Stale execution authority cannot sign or apply, but
   a still-relevant authenticated result remains available for the current owner
   to adopt without forging a new origin.
5. **Recovery reconstructs the next action.** Each durable cut determines ready
   work, an explicit external dependency, completion, or a terminal error.
   Recovery cannot require another client transaction, a vanished callback,
   an empty block, or an operator restart to release an internal wait.
6. **Retries have a cause.** A retry names its changing dependency and wake
   condition. Invalid artifacts and impossible ownership transitions terminate
   with typed diagnostics instead of entering an unchanged retry loop. Fail-stop
   is not counted as successful liveness or automatic recovery.
7. **Safety survives simplification.** Preserve exact `3f + 1` committees and
   `2f + 1` validator quorums, signed RS16 availability, authenticated round and
   context identity, durable-before-signing, deterministic validation and
   canonical exactly-once application. Observers cannot supply votes.
8. **The asynchronous queue is a source, not a vote.** A leader may choose a
   bounded subset of locally available transactions and complete admission
   inputs. Its signed proposal fixes their order and exact bytes. Replicas
   validate that proposal against authenticated protocol and committed State,
   without requiring the same local queue contents or arrival order. A missing
   local certificate or held Queue owner must not indefinitely block unrelated
   eligible work. Proposal sizing must leave every accepted item a reachable
   carrier opportunity. The current global FIFO predecessor cuts still need
   review against this rule; removing them requires tests for route closure,
   same-account economics, reservation release and restart.

The bounded leader sampler now advances across local queue windows when an
unadmitted QueuePlan prefix exceeds `max_queue_scan`. It resets on a new
committed parent or physical queue reorder, and stops at canonical QueuePlan,
selected, transitioning or durably reserved FIFO owners. A scan with unvisited entries wakes the
leader for another bounded turn. The Core regression selects ready work behind
three unadmitted claims with a one-item scan limit, then checks that a new
committed parent reconsiders the earlier claim. This is scoped queue-selection
evidence; the full network fault campaign and a committed-close transition for
the skipped claim remain required.

Protocol and storage changes may break pre-release compatibility. Keep one
explicit canonical Norito layout and reject obsolete inputs. Intentional
protocol changes update the specification, executable transition relation,
fixtures and safety/liveness arguments together. Production configuration
belongs in `iroha_config`; hardware choices cannot alter consensus results.

## Ordered goals

| Goal | State / dependency | Owner | Completion criteria |
| --- | --- | --- | --- |
| L1 — One contract and complete progress inventory | **Active** | Core/Sumeragi and formal owners | Reconcile voting/replay contradictions against executable code; map every wait, resource hold, wake source, terminal result and durable reconstruction path. Map all nine runner states to their authoritative facts. Identify each independent scheduling decision and the exact code to remove. |
| L2 — Remove competing scheduling authority | **Active** for inventoried runner slice; full L1 inventory remains prerequisite to closure | Lifecycle, runner and runtime owners | Production uses a fresh owner-derived action projection; remove the runner's shadow state and transition history, then consolidate duplicate runtime decisions. Wake events and deadlines stay serviceable under saturation. Meet the existing measured simplification/deletion obligations across the complete source closure, including moved code. |
| L3 — Conserve work through completion and recovery | **Active** for Store publication/completion handoff; integrate with L2 | Reducer, lifecycle, WAL, body store and Kura owners | One stable obligation survives view/generation changes and every durable cut. Adopt results only under current authority; no lost work, double-signing, duplicate application or lifecycle resurrection. Replace redundant journals/repair paths with the minimum sufficient recovery representation. |
| L4 — Close resource cycles and permanent retry loops | **Active** for certified-persistence retries and Native/global dependency closure; L2–L3 | Worker, P2P, validation and application owners | No asynchronous wait or failure/destructor path retains a resource needed by its own completion. Exercise typed retry dependencies at exact capacity. Terminal failures are observable and never silently retried. Ordinary work and authenticated recovery receive bounded fair service. |
| L5 — Exercise the assembled runtime | Open; test construction starts with L1 | Core tests, simulation and formal owners | Deterministically explore event orderings, saturation, restart cuts and generation changes through production scheduling/adapter paths. Replay minimized counterexamples. Pass current formal/trace checks and adversarial mutations; source-text matching and ghost fairness assumptions do not establish runtime progress. |
| L6 — Qualify one immutable candidate | Open; L1–L5 | Integration, release and component owners | Pass the real four/seven-validator matrix and applicable existing formal, chaos, multilane, workspace and SDK gates on the same source/artifacts. Documentation describes the resulting implementation. Missing, failing, skipped or stale gates remain open. |

## Acceptance matrix

These are executable acceptance properties, not new runtime monitoring layers.
Add them to existing Core, integration and formal owners. Each regression must
fail on its targeted counterexample; production must use the exercised path.

| Case | Required observation |
| --- | --- |
| Dependency closure | For every wait kind, fill unrelated queues, deliver its wake event, and complete within a declared scheduler-turn bound under terminating services. Cover sidecar/body responses, WAL acknowledgements, capacity release and application. |
| Reordering and duplicates | Permute Proposal, PrepareQC, TC, CommitQC, body and completion arrival. A future PrepareQC cannot obstruct its view-installing TC. Duplicate storms stay bounded without suppressing necessary replay. |
| Generation changes | Same-view/new-generation completion and later-view recovery retain usable results while rejecting stale signing/application authority. Legitimate supersession removes exactly its own work. |
| Crash boundaries | Crash immediately before and after each persistence acknowledgement, result publication and application handoff. Reopen real retained artifacts and reproduce the remaining obligation without loss or duplication. |
| Capacity and failure | Saturate ordinary ingress, execution and output; exercise slow/nonreading peers within the fault model. Required control/completions still run. Failure/drop callbacks finish while outer permits remain held. |
| Final transaction | Submit a finite valid workload, then stop submissions. Every accepted non-expired transaction in the test completes exactly once at all responsive validators, with authentic finality and applied-state evidence; afterwards empty queues do not require empty blocks. |
| Lane composition | Repeat relevant cases with ordinary lane work, Native AMX and a lifecycle transition. A blocked lane cannot starve unrelated executable work; required cross-lane dependencies remain explicit. |

Real networks use four and seven validators with mandatory signed RS16 and
fault counts within the declared BFT bound. Include leader isolation, loss,
duplication/reordering, parity reconstruction, restart from retained data,
ordinary-capacity saturation and the final transaction. Prove hold/drop
acknowledgement before healing through the authenticated message controller.
After healing, every responsive validator must converge and apply within a
bound declared before the run from its configuration and service assumptions.

Run at least 32 deterministic seeds for each network scenario and retain the
existing 100,000-height permissioned/NPoS chaos and applicable two-hour soak
requirements. Measure finality and application separately. Record maximum queue
occupancy, retained memory/work, throughput and tail latency against the same
baseline workload. Hiding a stall with a larger timeout cannot close a goal.
Formal fairness and I/O assumptions remain explicit and are exercised where
controllable, rather than silently assumed by a mock.

## Execution and evidence

Implement vertical slices that remove an old authority when its replacement
enters production. Preserve assertions in existing regression tests and update
construction when APIs disappear. Each slice includes focused behavioral tests
and relevant model mutations before broad qualification.

Keep one evidence record per candidate with source/lock identity, commands,
toolchain, configuration, seeds, hardware, actual outcomes and counterexamples.
Use existing build/test/formal runners and isolated generated artifacts. Inspect
active build processes before Cargo; never interrupt another job. Run focused
checks first, then workspace and release gates. Do not refresh proof seals solely
to silence drift or infer readiness from test counts.

### Implementation checkpoint

- The task goal is active. L1 and the first L2 slice are underway; no goal is closed.
- Source/history review identified sidecar self-blocking, future-QC head-of-line
  blocking, caller-held-permit deadlocks, and generation-sensitive result loss.
- The initial documentation correction aligns the round protocol and safety-WAL
  description with current same-round Prepare/Commit behavior. It changes no
  executable protocol and establishes no runtime pass.
- The runner implementation now reads retained ownership at service boundaries,
  including after Runtime and before Producer admission. Its old event-transition
  implementation remains only as a temporary test oracle. Unleased Validate and
  certified-body persistence use existing worker custody through acknowledgement;
  terminal Serve replay carries its admitted authority kind in that same index.
  Broader Core behavioral observations are recorded below; qualification remains open.
- Nondecreasing reducer-fence generations are enforced by the retained Validate
  successor itself. New tests exercise real published successors and live/recovered
  Apply settlement; launched-owner tests no longer inject remembered runner state.
- The standalone reducer baseline passed **220 tests, zero ignored** after fixing
  the harness's stale exact-count expectation of 197. Its 48 captured inputs stayed
  unchanged across the run. Six focused harness guards passed. This evidence covers
  the pure reducer and its harness, not the assembled runtime change.
- The Core unit binary compiled with 353 captured inputs unchanged. The first
  focused run passed 43 of 44 tests; five additional actual-owner/cold-reopen
  tests passed. One startup fixture incorrectly reset the former runner state
  after terminal Apply. It now asserts that the ordinary batch preserves its
  exact ingress occurrence and the production decided-lane recovery suffix
  drains it in one bounded turn. The corrected fixture passed with the captured
  source unchanged. A broader ownership run then exposed the separate Store
  transfer defect below and stopped: four passed, one failed, 1,246 unexecuted.
- The current combined owner/Serve source suite passes all 50 checks after
  correcting two stale clauses: historical completion is settled at both ordinary
  and final-drain boundaries, and PendingKura authenticates retained finalization
  custody before rollover. A new adverse control rejects removing that gate.
  These checks establish neither runtime liveness nor source inventory/hash closure.
- Durable Store publication removes the old pipeline token while an older physical
  Store can remain in flight. Its completion previously required that retired
  token before checking the exact published successor, causing fail-stop after a
  legitimate transfer. Completion now authenticates the published Store/Validate
  marker first and permits the absent predecessor only for exact coalescence.
  Missing authority without a marker and extant foreign tokens remain errors.
  The corrected binary passes all 11 selected Store completion regressions,
  including the new negative controls, with 353 captured inputs unchanged. A
  broader 1,252-test ownership run passed 1,249 tests, failed two stale source-shape
  assertions and left one incident-inspection helper intentionally ignored. Both
  assertions pass after correction and rebuilding. Separately, all 416 runtime
  and protocol tests pass with their captured inputs unchanged. These are scoped
  runs, not one complete release pass. The Store tests reproduce the executor
  handoff contract; full launched-runtime overlap still needs qualification.
- The restart integration source now covers four/one-outage and seven/two-outage
  cases, plus an explicit 32-seed matrix across four/one, seven/one and seven/two
  (96 sequential networks). It retains finite workloads and adds cryptographic
  finality against the complete original committee. Its test binary compiled in
  12m25s and the production daemon in 11m43s. Three four-validator discovery runs
  reached genesis but stopped in harness account/status reads, before exercising
  the restart: Tokio blocking-context rejection, obsolete typed-not-found
  matching, and the unimplemented iterable-account adapter. The harness now uses
  a native SDK worker and direct account lookup. The SDK preserves canonical
  HTTP status/code/message as `QueryError::Http`; all 33 query tests pass.
  A fourth unchanged-candidate attempt passed all three SDK harness controls and
  reached applied height four, but its pre-restart block check hit the same
  unsupported iterable adapter through `FindBlocks`. All four nodes exited
  normally; this is not restart evidence. That check is being migrated to the
  canonical committed block endpoint with exact height, bytes and body checks.
  Seven-validator and seed-matrix runs remain unexecuted. Fixed binary hashes,
  logs and input captures are retained under ignored
  `dist/sumeragi-liveness-redesign-20260916/network-checkpoint-01` through `03`.
  Run 03 overlapped subsequent source edits and is discovery evidence only.
- The certified-persistence audit found that Phase A retains authenticated body
  data but discards its admission candidate. Phase B reselects using the live
  request, treating even lost identity as retryable solely because publication
  has not begun. Reconstruction, view/Decision cleanup and signature-capacity
  preemption can retire Fetch requests. A regression using the actual serialized
  runtime, signed four-validator TC, worker queue and Phase B reproduces the lost
  authority as `InvalidSelectedOccurrence`. After binding the fixture to its actual
  WAL, the unchanged pre-repair source failed; the ownership repair passes both
  protected and unprotected timeout cases. It reads existing physical persistence
  custody through acknowledgement, preserves the exact request/pipeline during
  view/lock cleanup and excludes that work from signing preemption. Reconstruction
  selection now permits the disk completion that owns settlement. All three
  focused regressions pass, including capacity pressure through queued, active,
  completion-pending and unacknowledged result states, plus direct reconstruction
  racing that owner. The reconstruction test covers the executor/service boundary,
  not a full chunk-arrival/physical-local-FIFO schedule.
  The retained pre-Decision binary also passes all 331 selected worker/persistence
  regressions. Intermediate receipts are under ignored
  `dist/sumeragi-liveness-redesign-20260916/checkpoint-02`; they cover distinct
  source candidates and cannot be combined into a release qualification.
- The candidate stages authenticated Decision exclusion as terminal cancellation
  in the same Phase-B ledger transaction, preserving the exact admitted request until
  physical acknowledgement. Generic/cached Apply wait in their existing owner before
  strict cleanup. The serialized runner gates ordinary ingress and fresh Ready work
  while disk persistence or a Validate successor owns completion; the actual launched
  Validate regression checks those gates before and after physical publication.
  This source argument is not yet a machine-checked exclusion proof. Decision
  cancellation needs its own formal binding because its terminal owner is absent,
  unlike the existing Ready-result theorem.
- Phase-B retry now requires an already-changed queue cut. Permanent identity,
  coordinator, executor, registry, service and refinement failures close output before
  ledger publication. Queue-capture errors retain their typed cause. The old test
  which repaired a foreign completion in place has become separate terminal-failure
  fixtures; unchanged successful coalescence/dequeue remains covered. The combined
  Core binary passes all 11 focused persistence, Decision, classification, coalescence
  and launched-Validate controls, with all 1,609 captured local inputs unchanged
  through compilation and execution. All 17 scoped adverse source mutations are
  rejected. The broader source checker still reports 52 other obligations; scoped
  mutation checks do not close that gate or prove runtime liveness. The production
  daemon and integration harness built for the failed network attempt below.
- The combined owner/runtime/worker run selected 1,679 exact tests on that same
  Core binary: 1,675 passed, three failed on two stale source clauses, and one
  retained-incident inspector was intentionally ignored. All 1,609 captured
  inputs stayed unchanged. Corrected clauses include the fresh timer census
  signature and exclusion of admitted persistence from high-Prepare and terminal
  cleanup. An intermediate recheck passed 12 of 14 and exposed the second stale
  cleanup predicate; after correction all 14 behavioral/source checks pass on
  a rebuilt binary with the same 1,609 captured local inputs unchanged. The
  aggregate source test executes all 55 source-contract cases. Complete original
  results remain in ignored `checkpoint-03`; corrected evidence is `checkpoint-04`.
- The fifth unchanged four-validator network attempt passed all four harness
  controls and failed after 690.21 seconds during the outage. Its transaction
  `420e354ffafde9f28722473ecacfe8d057b7c5541f34a9d1e73630ffc90d109b`
  was not Applied within 600 seconds (300 status reads, no observation). All
  three online peers committed the same height-five QueuePlan admission block,
  with zero external entries and no merge, then persisted seven TimeoutIntent /
  InstallTimeout pairs at height six without any ProposalIntent. All four peers
  exited with status zero. The 3,532 captured build inputs and both binaries
  stayed unchanged. This exposes an admission-to-execution progress failure;
  restart, seven-validator and seed-matrix qualification remain unexecuted.
  Canonical block bytes, read-only WAL/ledger diagnostics and peer logs are
  sealed in ignored `network-checkpoint-05`; runtime signing inputs are excluded.
- The runtime audit found independently selected adapter-deferred work and a
  cached external-owner census published by the executor. The candidate removes
  publication and supplies one bounded, validated borrow from the six actual
  executor owner sources to all runtime arbitration/ingress entries, including
  timeout freezing. The combined Core lib-test build and four focused census
  regressions pass: exact identity/deduplication, exact capacity, all six sources
  with immediate retirement, and independent executor pending capacity. Broader
  regression and source-proof gates remain open. No additional reachable
  cache-induced stall is claimed. Physical ingress cuts, exact dependency
  authority and bounded fairness remain required.
- The retained census binary passes all 418 runtime/protocol tests; its four
  focused census/retirement controls also pass. These observations predate the
  persistence repair and do not qualify that later source.
- TODO: complete owner-derived behavioral coverage, delete the temporary oracle,
  finish the remaining wait/resource inventory and runtime consolidation, and
  execute the acceptance matrix on one unchanged final candidate.

- The native initial-author-loss fixture and live-author control both pass on
  one Core binary with 1,638 unchanged captured inputs. The negative fixture
  deliberately records absent timeout/proposal progress; this is reproduced
  failure evidence, not a network liveness pass.
- The shared reducer now represents genesis, local parent CommitQC, audited
  snapshot and externally finalized state as mutually exclusive anchors.
  `new_from_finalized_state` permits an authenticated ordinary/Native global
  predecessor without fabricating an autonomous lane QC. All **224 reducer
  unit tests pass, zero ignored**, with 47 captured harness/source inputs
  unchanged; four new cases cover malformed anchors, unchanged ordinary parent
  rules, four/seven-member pre-payload durable timeouts, and lock-preserving
  replay. Native cryptographic/state-proof binding is still an adapter
  obligation; this constructor alone does not repair lane production.
- The replicated applied-frontier candidate records the actual global carrier
  height on ordinary, autonomous, Native participant and snapshot-settlement
  paths, and rejects missing/zero or regressing anchors. After correcting a
  new fixture's `CommittedBlock` accessor, all **22 focused Core tests pass**
  with all 1,640 captured local inputs unchanged. The tested binary SHA-256 is
  `f0d3b86c5124bd1a38018571046bd7fd5a50885d839487a6a9daa548713ef3f9`.
  This is a locator foundation, not frozen committee authority. The shared
  reducer constructor and this State change do not yet connect pre-payload
  lane timers to production.

Complete-input carrier selection measures the actual unsigned proposal through
the canonical SignedBlockWire projection, with all actual policy and control
metadata. It retains a fitting admission prefix before private-key signing and
leaves deferred inputs in durable custody. Certified transaction gossip owns one
tagged canonical complete input; its authenticated entrypoint is derived from
that input and is not serialized a second time. The default-size regressions
are three approximately 800 KiB inputs under a 2 MiB carrier and a 160 KiB
transaction under a 256 KiB gossip frame. These checks do not replace full
preacceptance envelope feasibility or live fault qualification.

Complete-input structural decoding has one model-owned size/resource policy for
both authenticated admission and certified gossip. Nested transaction allocation
uses Norito’s existing frame-derived budget while retaining the 1 MiB wire cap,
field/element bounds, depth limit and stricter surrounding budgets. Structural
decoding still grants no quorum, custody, State membership or signing authority.
The exact-cap model and real 800 KiB/2 MiB carrier regressions pass, as do the
160 KiB/256 KiB signed gossip delivery, catch-up, replay and retry controls. The
combined Core/model/Torii test build passes. The wider source inventory remains open; completed-secondary cold archival now passes its scoped regression;
none of L1–L6 is closed.

Completed Native publication repair is a different durable operation from an
append or tip replacement. Its explicit first-release locator origin retains
the actual selected canonical marker and can never authorize an uncommitted
rollback. Admission and restart must reauthenticate the exact executed wire,
global finality, finalized WSV checkpoint/manifest join and retained receipt.
A partial repair keeps one complete-carrier owner: every non-target route must
have exact completed evidence or a fully authenticated later frontier with no
unfinished cleanup. Repairing A1 after B2 must never republish or roll back B1.
The combined Core/model/Torii test build and all 133 affected Core plus 61 Torii
tests pass. All eight new interruption/adverse controls pass; the original real
Apply regression now completes both economic cycles and replay checks. This
closes that reproduced repair defect, not the overall lane-runtime goals. Live
certified attempts must remain owned; successful archival must be tested only
after real economic Apply and exact Queue terminal settlement.

The first-carrier reader, all-route immutable input preparation, native RS16
materialization and exact native WAL witness replay now pass their scoped
boundary controls. They do not start a production timer or grant body Ready.
The immediate cutover sequence is thin durable input custody, checked native
effect projection, a process-lived shared-reducer owner with pre-payload clocks,
instance-scoped ingress/output across global rollover, and native Decision
consumption by global execution. Activate only with atomic removal of the old
lane/NewView/Native signing authorities; no dual engine or compatibility toggle.
The four-validator silent-author counterexample remains an open acceptance test.

Thin descriptor-bound native body custody and reverse native effect projection
pass their scoped controls. Publication or readback failure fences that store
until verified reopen. An original signed proposal retains its exact native
timeout evidence; rebuilding equivalent abstract evidence cannot reproduce its
signing bytes. Neither boundary acknowledges reducer body readiness by itself.

Before activating immutable per-instance body storage, prove that all affected
route slots remain fixed while their admitted group is open. The current State
finalizer independently reopens a route on frontier advancement, so one member
can change another still-open member's input descriptor. The sole economic
pipeline must advance affected frontiers and settle every obligation for that
exact group atomically, with lifecycle drain preserving pending ownership.
Retire ordinary economic bypasses and old Native writers in the same cutover;
add a cross-route adverse test. Overwriting a locked body or treating unrelated
global progress as cancellation is prohibited.

The inactive process-lived instance owner now passes real State/Kura/four-key
pre-payload timeout, fsync/restart, saturated-output, high-Prepare and unchanged
global-rollover controls. Its checked deadlines retain custody on overflow;
native projection contradictions fence the owner. The read-only Decision-group
consumer verifies every distinct current route against the exact first input
and its own frozen layout, permits independent voting views, and retains named
missing/earlier dependencies. All nine new controls and 46 prior selected Core
checks pass. Neither component activates a second production signer.

Move-owned body, WAL and opening jobs now have actual threaded State/Kura
controls. A process-lived bounded table reserves one exact instance/key before
opening, owns queued/transferred/completed jobs, preserves clocks through global
advancement and drains physical handles after authenticated closure without
acknowledging held Decision/Apply or transport obligations. Production does not
yet construct this table.

Candidate 48 passes 408/413 selected Core controls, all 332 block-model tests
and 122 scoped formal checks, including all 21 repair/capacity controls and
previous admission/gossip, physical-owner and economic regressions. Five new
ordered-stage controls pass; two stop at an invalid policy fixture, and three
older fragment-count fixtures fail identically on captured candidate 47.
Candidate 49 changes only those two fixture files; all five affected tests pass.
The model binary is byte-identical to 48. The fresh full structural gate on 48
still reports 148 binding errors. Each checkpoint retains its unchanged inputs
and binaries; results are not pooled into an unexecuted full 49 suite.

The integrated constructor authenticates native groups against exact applying
pre-State, runs mandatory start hooks once, then executes native economics under
height-H policy on the same owned overlay. Its private native seal binds actual
carrier identity, complete source membership and the start+native prefix roots;
those roots are not final block write roots. Actual execution outputs remain
privately owned. Later metadata cannot erase the stage; arbitrary native commit,
including through the old empty-merge authorization, is explicitly rejected.
The staged main merge and its independently corrected fragment-count fixtures
are preserved; main composition remains uncompiled.

The retained-source/shared-tail candidate builds and passes 594/594 selected Core
controls on 20,548 unchanged local inputs, including 16 new custody/suffix/scratch
controls and all 21 publication/repair/capacity controls. Its model executable is
byte-identical to the prior 345/345 passing artifact; model tests are not rerun.
The include inventory correction passes 34 source-loader controls in a complete
source mirror; full structural output equals the prior 148 errors. The 19-path
change is integrated into main, preserving its staged merge; main composition,
full workspace and network acceptance remain unqualified. Earlier failures and
exact artifact scopes remain recorded, not pooled into an invented suite pass.

LaneDecisionBatchV1 carries exact applying pre-State and ordered authenticated
input Decisions. Actual aliases and prefix roots remain private. Native FASTPQ
rows and real captures remain in the original overlay and rejoin the privately
sealed prefix before common inventory. The ordinary sequential/DAG tail retains
full results and uses actual Time call identities for receipts. Do not restore
proposal output/write claims or assign callback receipts to display identities.

Next: replace process-global witness authority with StateBlock/StateTransaction
capture ownership, including constructor hooks and applied-fragment rollback.
Distinguish canonical rejected-result observations from abandoned trial reads.
Factor deterministic candidate metadata from finality-only publication and seal
one complete bounded State projection. Sparse instrumented keys are insufficient.
Resolve pipeline callback results through one typed canonical output sequence for
network, pipeline and Time invocations, with full receipts once and explicit
input/output proof mapping; migrate model, Core, queries and SDKs together.
Two actual-execution counterexamples now reproduce the open pipeline owner gap:
direct callbacks lack a root call; nested callback results are discarded and
finalization refuses repeatedly after rollback. These test-only changes are
integrated into main. The separate 124 pending-membership formal controls pass
on the unchanged source mirror; the 148 full structural diagnostics remain.

The isolated model removes the generic header result root and has one complete
typed Network/Pipeline/Time output collection, with independent network-input/output
proofs and finite pre-reserved terminal arithmetic. Candidate 70 builds the model
with `http,ids_projection,fault_injection` on 20,556 unchanged local inputs:
3881 tests pass, one existing privacy-activation capture test fails, and
9 are ignored. All 391 block-model controls pass. The privacy failure also
reproduces on the retained pre-migration candidate-63 binary; the full suite is
not green. Both proof anchor constructors require an independently trusted target
height context; actual alternate-roster BLS controls reject circular self-asserted
roster authority. Existing CLI/JS callers already pin their chain and now pass the
verified target context; their current composition is uncompiled. Rejected Network
outputs forbid callback completions, while rejected internal outputs retain only
one matching root Failure for the whole invocation. The three new controls pass.
Generated format fixtures and all preceding assertions remain in force. The prior
eight actual-Set controls remain separate candidate-63 evidence. No current Core,
main composition, SDK, formal or network qualification follows.

The isolated applying policy now has one genesis-installed finite output envelope,
separate active Time invocation count, and total Pipeline/Time registration caps.
Post-genesis envelope replacement is rejected; active Time may change within it.
Constructors capture once after their actual start lifecycle, including an invalid
restored-policy refusal. Actual disabled/depleted registrations occupy capacity;
new/replaced actions remain deferred by their existing incarnation-height guard.
The ordinary candidate count uses the worst permitted future registry/Time growth.
State reserves one terminal plan from the exact proposal/input projection before
execution; a native multi-route group contributes one Network row. An unfinished
plan refuses publication. Its private continuation now transfers the same budget
while retaining Reserved/Running/Retained/Poisoned ownership on State. Completed
rows still cannot authorize publication. This is not complete source, framing,
trace, capture or resident-memory admission.
The nine State controls and candidate/native extensions remain uncompiled/unrun.
TriggerUse retains bounded ID, registration height and action hash, with actual
authority authenticated inside that hash; the actual-Set rekey control is unrun.
Model candidate 90 passes all nineteen new policy/parameter/terminal controls and
all 400 block controls. Full model candidate 135 passes 3917 tests with 10 opt-in generators ignored.
Nine additional internal-rejection controls cover both phases, exact row/shared
boundaries, preowned terminal strings, descriptor extremes, binary/JSON roundtrip
and rejection of malformed roots, origins, completions and business receipts. The earlier single failure was a stale privacy fixture
with a retired activation-height field; actual codec recapture changes only its
four frames and preserves strict refusal of the removed layout. Both manual-frame feature
configurations pass 21 regular tests each, with three ignored generators, after
actual feature-matched capture (93/99). Schema 94 passes all nine tests.
The private successful-Network kernel preallocates row storage, preserves input
positions under reordered execution, joins exact-call transaction-owned receipts
and fits the full row before State apply. Oversized healthy work rolls back;
local refusal or pre-apply unwind poisons the carrier, restores ZK deduplication
and drops its witness overlay. Actual callback capture now lives on each State
transaction: a non-copyable pre-body ordinal binds the original execution call,
including nested by-call work whose old returned trace was discarded. The actual
wrapper captures early errors and successful empty NoOp steps; a DFS failure
before another dispatch also latches refusal. Successful trace/completions move
once into the complete Network row before fitting and apply. Completion events
are emitted once only after fit, under the same call and actual ordinal. Missing,
foreign, failed or undrained journals block both ordinary and consensus-effects
application and poison parent publication. Child-payload lower bounds derive
from the frozen row ceiling; exact complete-row sizing remains authoritative.
Host allocation refusal is distinct from actual output overflow. Failed callback
errors remain typed and cannot be converted into a healthy OutputLimit rollback.
Thirteen exact-source journal tests pass in the fresh harness against retained
125 model/codec/crypto artifacts. Actual rejection can consume and discard completed
capture even after healthy overflow; pending/foreign/refused journals remain fatal.
Discarded business capture never authorizes apply. This is not State qualification.
Five actual-State callback controls and the earlier producer controls remain
unexecuted because Core has no test executable. The private success kernel now
retains completed actual gas/confidential work after either applying a healthy
row or dropping its oversized business overlay. A focused budget API control
checks that the next overlay still sees the completed block work; this is not
cryptographic-verifier qualification or the full rejection/fee corridor.
The same borrowing producer now runs actual Pipeline and Time invocations under
one internal rollback/retention owner. Pipeline takes signed-input events from
retained Network dispositions and frozen routes, then BlockApproved. It preserves
original Network and matched-ID positions, skips stale/depleted/disabled/current-
block actions and exhausted gas, and rebinds the persistent use-time action before
executing. Event route data never grants callback write-routing authority. Time
uses the constructor-frozen count and actual event/schedule with fresh eligibility.
Both seed the descriptor-derived call before work, retain actual nested callback
capture and exact-call receipts, and fit the full row before applying State.
Pipeline debits its root repeat before DFS; Time clears retry/debits repeats after
complete success. Same-generation guards preserve replacement actions. Healthy
output overflow drops business writes, repeats/retry clearing, witness and capture,
retains completed work and emits only the reserved root failure completion.
Real errors discard the failed business journal before separate failure policy.
Pipeline disables the exact reauthenticated action; Time schedules its next retry
or removes the exhausted action with its associated permissions. Earlier successful
siblings remain applied. No-policy Time failures retain the action and repeat.
A root failure retains its declared projection; root success followed by DFS
failure retains ReturnedBeforeRollback. Excessive diagnostics use the distinct
OmittedAfterRejection root and fixed short reason, never healthy OutputLimit.
Core checks borrowed program payload sizes before diagnostic copies and bounds
UTF-8 formatting; local journal/allocation/codec refusals still poison the carrier.
Actual controls cover event order/calls, stale candidate gaps, phase/gas skips,
full-row exact fit/one-byte-below, sibling preservation, root-success/DFS-failure,
oversized real rejection/quarantine and real retry advancement/exhaustion. These
State tests remain unrun. One exact-source formatter test passes independently;
it does not execute State. Legacy direct internal callers, complete source/capture
admission and canonical Block integration remain unfinished. The State-only
consuming seal below does not grant final publication authority.
Replay parity now validates both complete output caches and compares headers,
signatures, ordered Network sources, full typed output rows/root and all retained
side metadata. Diagnostics use explicit signed-source joins and separate internal
failures. The existing exact-wire QC/manifest, recomputed execution commitment
and WSV checkpoint gates remain independent in the replay caller. Six exact-source
structural comparison tests pass in an isolated harness against retained 108
dependencies; these do not execute State replay or qualify its authority/resources.
All old replay-validation entrypoints/assertions remain represented; strict
actual producer/Apply fixtures were not replaced with fabricated valid rows.
The private Network owner now freezes original source positions, routes,
authenticated QueuePlan validation instants and reveal execution order before any
source runs. Borrowed admission uses the same explicit-time envelope/signature
validator; stateful admission and the actual executor run in a disposable overlay.
The real callback journal, exact-call receipts and full row fit precede successful
apply. Healthy overflow rolls back business, repeat/events, witness and ZK state
while accounting completed work. Actual error discards business capture, retains
confidential work, applies prevalidated ballot penalties before eligible fees,
and preserves an applied penalty if fee settlement subsequently fails. Final
block-gas rejection retains its distinct no-fee/no-transaction-gas disposition.
Actual gas/fee eligibility comes from the typed rejection before row projection.
Oversized real error text uses a distinct preowned diagnostic string and cannot
become healthy OutputLimit or manufacture misconduct. No business receipts or
callback completions survive the rejected Network row. Actual fragment counts
come from the separate applied overlays. Signed execution, exact-fit/one-byte-below
callback rollback, real error after overflow, stateless rejection, block-gas admission,
Nexus-fee and actual two-ballot penalty controls are authored but unrun. The gas
control rejects before body execution; post-success final gas rollback remains
unexercised. A real
fee-failure-after-penalty runtime control is still required; ordinary insufficient
funds would reject at admission, so it is not fabricated as ordering evidence.
Genesis and legacy merge/native sources are excluded from this private ordinary
execution owner. Complete proposal/finality, duplicate/replay, QueuePlan controls,
route/DA/AMX authority, source/host-memory and common-wire admission remain
obligations of the canonical driver. The
producer now retains a privately constructed capsule in actual output order,
including rejected and zero-transcript calls, exact frozen Network routes and
originating network/height. FASTPQ compares proposal and frozen context, admits
only actual execution calls and typed applied ProtocolPurpose extras, and
preserves transcript-content sealing, digest validation and the invalidation
latch. Independently applied fees/penalties remain legitimate transcript owners
for rejected Network calls. One State-only driver reserves, executes all three
phases and consumes its actual rows through a seal. Complete proposal commitments
are checked before the finalizer. The caller cannot supply replacement rows,
sources or applying policy. Finalizer effects precede fragment reconciliation,
transcript inventory and one checked output attachment; exact wire hash/length
remain bound. Typed finalizer errors survive. Partial/mock/foreign sources,
unowned receipts, repeated takes, errors and unwind retain Poisoned; successful
attachment retains Sealed. Ordinary and consensus-only transaction application
after sealing is refused. The commit gate stays closed. Five source and ten seal
controls are authored and unrun; all fourteen existing inventory child files and
91 tests remain byte-exact.

The private Network owner now consumes an execution-owned rejection fee record for
every signed source class. Admission freezes exact
source/proposal/network/route/index, Gas/Nexus policy and payload length. The
actual body supplies authored or verified-replay instruction count and direct
VM/ISI gas; mixed Batch contract work accumulates in the same record. The root
closes before Data DFS, and triggered contract work cannot become another direct
root charge. Business rollback discards its effects, while independently committed
penalties precede a fresh authenticated fee overlay. Fee pricing uses the admitted
snapshot and actual retained work, never a reconstructed claimed overlay or a
Batch-only eligibility predicate. Settlement consumes the record once and does not
add gas or test completed work against the block ceiling again. Zero charges add
no synthetic applied fee fragment. Typed internal/resource and block-gas
exclusions remain; healthy output overflow drops staged business and fees.
Successful and rejected roots use the same direct-body pricing basis; callback
work still counts against block execution resources.

Both generic and self-describing raw-IVM branches now retain consumed gas before
runtime, artifact-validation or artifact-application errors can return. The old
post-application root assignment is removed so nested work is not overwritten.
Deployed contract calls also record their actual direct VM work in the fee owner.
Seven actual State fee controls and five actual raw-runtime controls are authored,
including exact block-gas boundaries, admission failure, Data callback rollback,
healthy output overflow, a consumed and context-bound fee record, admitted price
retention, non-genesis raw rejection fees, generic/bound runtime exhaustion,
artifact validation, artifact apply rollback and successful artifact application.
None has executed. Two existing executor tests now use fresh overlays for all
signed attempts, preserve every original assertion and warm cache, apply
successful setup/work, and drop failed attempts.

One private signed-root instruction budget now freezes the exact source, proposal,
network, route/index and agreed overlay instruction/byte policy. Authored
Instructions and Batch sets, verified-replay queues and consumed actual
HostExecutionArtifacts all admit a whole group before its first effect. Mixed
Batch calls debit that same cumulative owner; the late returned-instruction
recheck is removed. Individual fixed V1 bare InstructionBox encodings are measured
with a bounded counting writer instead of an allocated encoding vector, and the
byte diagnostic reports only that the measured ceiling was exceeded. Zero retains
the existing no-additional-limit meaning. This is an instruction count/encoding
budget, not a complete host/decode/durable-state memory bound.

The actual Executor wrapper closes effect and fee roots on every normal result
before Data callbacks. The actual synchronous trigger depth excludes both generic
and bound callback bodies from the direct-root instruction budget. Healthy opaque-
deferred NoOp is decided before artifact admission. Completed VM/replay work
survives later cap refusal, while refused authored work and never-started queued
confidential instructions are not recorded as executed work. Private Network fee
exclusion now uses the retained typed cap failure instead of parsing diagnostic
strings. Missing, repeated, foreign, unclosed and failed owners cannot publish
staged effects; ordinary and consensus-effects apply guards poison incomplete
State publication.

Sixteen new controls are authored: exact/unlimited/one-below authored and actual
generic/bound/mixed VM groups; bare V1 encoding under ambient flags, atomic group
counting and arithmetic failures; missing-root actual artifact refusal; actual
Network fees and plain/generic-IVM callbacks; owner reuse/context substitution
through both apply paths and a post-stage unwind; supplied post-verification
replay count/byte refusal with retained gas. None has executed. Three existing
Host artifact tests and three existing supplied-replay tests retain all original
assertions with explicit signed-root ownership; one mixed byte diagnostic
assertion now reflects bounded measurement. The supplied replay fixtures do not
claim actual proof verification.

Private Network admission now freezes exact signed Boolean quarantine
classification once. Only stateless-admitted signed sources consume quota, ranked
deterministically by outer entrypoint hash and original index; selection never
changes the original effect order or separately frozen reveal order. Zero quota
rejects all classified sources before business execution, gas, fees or penalties.
A later selected-source business or output failure does not refill its slot. Batch
and ballot shapes have no exception. Policy drift after freezing is a local
ownership error. This policy remains in the private replacement owner; the old
canonical Block/DAG is unchanged.

A finite non-cloneable completed-cycle owner belongs to the actual signed root and
spans direct generic or bound VMs, mixed Batch segments, actual proved replay and
recursive CoreHost calls. Each dispatch reserves its architectural cost before
effects, holds parent syscall reservations through children and retains completed
work through traps, reuse and unwind. HALT commits before trace flushing. Exact
fit succeeds; a refused reservation is sticky even when a host swallows a child
error. Local foreign/closed owners remain distinct from deterministic cycle
exhaustion. Zero cycle limit means no additional bound. Actual trigger callbacks
have their own scope. This is completed architectural-cycle accounting, not total
host, proof, decode or allocation work.

Actual replay work now transfers to State and the root fee meter immediately after
verification returns, including rejection after real execution. Successful replay
records its actual instruction basis once before later SCCP, AXT, payer, block-cap
or effect admission can fail; metered replay requires the retained gas and does
not add it again. Supplied post-verification fixtures now state their supplied
work handoff explicitly; they do not claim measured execution or cryptographic
proof. Typed instruction/byte preflight exemptions remain separate from chargeable
cycle exhaustion.

Twenty-four new regressions are authored: nine actual VM cycle tests, two real
nested CoreHost tests, nine actual Network quota/fee/reveal tests, generic signed
and mixed-Batch root cycle limits, authorized actual replay work and an actual
generic callback under a finite quarantined root. Existing proof fixtures
additionally cover cryptographic replay rejection and a successful actual
proof/replay followed by block-gas refusal, preserving all prior assertions.
Existing supplied-replay fixtures preserve their assertions with explicit work
custody. Only the nine new VM cases have executed; the fifteen new Core cases and
expanded Core fixtures have no runtime result.

A private persistent World baseline now complements the actual World net delta.
The shared visitor uses the same 278-field constructor registry, expanding
TriggerSet into ten semantic stores for 287 field sections. Cold capture includes
untouched values, snapshot-skipped authoritative stores and derived indexes.
Incremental versions encode only actual touched before/after pairs and check every
touched preimage, including no-ops. Cold predecessor capture reverses the actual
first-preimage journal after MV replacement undo. Field names/order/kinds are
bound; this does not introduce independent Norito type-schema identifiers.

The lower-level MerkleMap uses immutable shared nodes and a canonical compressed
binary radix tree of domain-separated key/value hashes. Its shape/root depend on
current values, not insertion, deletion or undo history. Branches bind their split
bit, raw shared prefix and ordered children; absence, empty values and count
remain distinct. Each update checks its preimage/count before replacing the root
and copies only its bounded key path. Existing Hash supplies the hashing
implementation. The map has no wire format, external node/proof input or disk
durability claim. Initial capture, retained versions, final snapshot destruction
and aggregate allocation still require resource admission.

The existing private output seal continues to bind the actual World net delta. The
new complete World value baseline is a private component exercised by tests, not
installed in the production State lifecycle, a full State root, or publication
permission. It requires the same actual predecessor: touched-key comparison cannot
authenticate unrelated untouched-state drift. Executor and trigger semantic
encoders are shared by both projections; merge, snapshot and consensus commitment
formats are unchanged.

The source-bound lifecycle audit identifies late World writes after output
sealing: AXT incarnations/ratchets, replay expiry, DA quota/pin changes and lane
cleanup, including live World pruning after World commit. Transaction replay
membership and agreed runtime/context publication are separate owners. Replacement
reverts MV owners but currently clones some live
Nexus/incarnation/lineage/manifest metadata. The complete baseline therefore needs
one State-owned commit-preparation capsule, exact custom replay-membership
projection, authenticated runtime predecessor restoration and publication in the
same State generation. Root/attestation dependency order must avoid self-
reference; existing checkpoint hashes and merge hint roots retain their narrower
meanings.

Twelve new regressions are authored. All six persistent-map tests pass: 120
insertion/removal orders, all 255 variable hash-bit splits and byte boundaries,
700 mixed updates against an independent sorted rebuild, stale-preimage/count
failure, immutable subtree sharing and independently calculated hash vectors. Six
actual World baseline tests cover untouched/skipped values, cold/incremental
equivalence, commit/replacement/rollback, stale-preimage refusal, touched-only
encoding, schema/presence and actual trigger stores. These six Core tests pass in the thirteen-control World projection selection254. Independent source reviews found no concrete defect; they are not runtime
qualification.

The complete-publication design was refined on 2026-09-18: retain the existing
execution-prefix commitment and canonical recovery snapshot hash. A new full-State
Merkle field is not required to close publication ownership. The private persistent
World baseline remains a proof/read component; it cannot authorize publication or
replace the actual retained MV predecessor. This supersedes the earlier requirement
to integrate that baseline as a prerequisite for every State publication.

Canonical Block/DAG cutover requires one consuming preparation capsule containing
the actual World, membership, runtime/context/topology/hash journals, source/witness
and frozen execution policy, together with admitted events, geometry, archive and
resource plans. Complete deterministic mutations and semantic admission before
voting. After exact QC and Kura/Native evidence authorization, consume those same
components once; do not rerun the World tail, re-admit membership or consult a local
Queue veto. A node learning a decision through catch-up must reconstruct this same
prepared transition; reconstruction failure is a local recovery condition and must
not become a second canonical rejection. Retain actual writer ownership instead
of introducing a diagnostic-generation counter as predecessor authority.

Current isolated archive preparation owns exact projections, admitted byte/count limits and original logical capture reservations while physical index writers are released. Its publish step authenticates the exact Kura receipt, verified finality and signed result-bearing body through one guarded oracle, then retries retained bytes without recomputing State. Under a held Kura publication lease, index acquisition is nonblocking: refusal retains the original capture and returns the actual physical index release observation. Every archive reader/writer notifies after unlocking, and poison remains a storage error. Both archive unit suites and the joint capture, checkpoint, physical publication and release controls pass in the 196-test DPN development build14 selection; the complete Validate-to-Apply consumer remains unfinished. Earlier archive unit evidence passed in471. Carrier journal decomposition and exact captured geometry pass their scoped484/485 controls: capture read-only projections first, then retain the original World/runtime/hash/topology/context journals and consuming membership reservation. The Kura phase owner retains one encoding, bounded verified phase differences and exact operations. Neither exposes a complete publication operation yet. The positive archive-composition and five startup-anchor fixtures pass in493/500; all remaining configured-startup fixtures pass in510. Explicit retirement maintenance and pure Native evidence observation with consuming durability attestation pass their505/506 and509/510 checks; the combined657-case regression passes on unchanged508 in511. The Native/historical geometry formal contracts pass31/32 checks and audited source inventory526 passes20; its full structural gate still reports168 diagnostics. Historical observation/attestation compiles in524 and passes all six controls in525 after correcting the fixture's canonical root;173 existing controls pass in521. The selected527/529 design replaces alias-based moves with stable canonical storage, immutable incarnation directories and authenticated reference publication, with physical retirement deferred to GC. Do not add a consensus-spanning evidence freeze to preserve mutable paths. Exact historical completion authority, cross-route pending-work closure, capacity admission, predecessor/successor recovery pins and an instance-local GC deletion fence remain required. The storage cutover and consuming publisher are not implemented; retain current guards until the complete path exists. Archive failures remain local admission/recovery conditions, not evidence that a consensus-valid block is invalid. Formal531/532 subsequently passes66 membership/delegated-State controls; the structural gate remains failing with140 diagnostics. Prototype530 was withdrawn after review found route-dependent retained descriptors acquired only after finality. The actual validator currently drops PreparedCarrier and returns only an execution hash; cached/reproposal markers can bypass preparation. Complete the exact worker-local resource handoff, typed local deferral, cached/recovered authority and consuming Apply together. Do not treat a reservation field in a discarded object, a lower local route cap, or a reopened inode comparison as that fix. Canonical terminal/compaction fixtures now preserve certified source bytes once and finalize outputs after attachments. Core/MV546 compiles545; all176 targeted Native/receipt/capacity/compaction/restart controls pass in547 without source/executable drift. Design549 selects one immutable authenticated Native proof bundle per canonical carrier plus bounded complete route/incarnation history references. Preserve exact compact-merge source, finality and WSV joins before source release; charge entire shared bundles until their last route/snapshot/recovery pin releases. Replace derived per-route application repair vetoes only through the complete representation cutover. See the dated530–549 evidence; full publication and network qualification remain open.

State-owned proof/read capture, whole-source admission/finality and allocation
bounds, canonical metadata and genesis/native/merge integration, and the SCCP
applied outbox remain required. Native and unfinished ordinary State publication
remain rejected. All L1–L6, the silent-initial-author counterexample and
four/seven-validator qualification remain open.
Local resource refusal must not become a canonical rejection. Future-invariant complete-source and allocation admission,
the parallel apply corridor and the 32-MiB proof/256-MiB consensus ceiling remain
open. A positive input count or terminal-only byte calculation is insufficient.
Compose the canonical producer and every Core,
query, proof and SDK consumer atomically. The isolated Rust SDK/shared/SCCP
consumer suite passes 850/301/197 tests again after the descriptor change
(candidate 87, no ignored tests or build warnings). All 20,563 captured inputs stay
unchanged. Earlier standalone shared/SCCP results (78/79) remain separate evidence. Details
have one full Network output owner; real BLS tests bind exact executed wire and
reject changed source/output/cache/context claims. Earlier failed candidates,
including the SCCP fixture's missing proposal input root, remain retained.
The actual ordinary Block driver now consumes the private complete-callback
producer, including the actual Network/Pipeline/Time policies and full replay
comparison. Native execution still needs the same complete source owner. Isolated Kura now
indexes only complete validated Network sources/outputs; Kaigi cursors use the
Network input index. Queries and status readers bind exact finalized wire and
charge metadata-authenticated body bytes before reading, then project one row
from an immutable carrier after complete validation. Old merge/bodyless index
promotion and the unbounded FindTransactions iterator owner are removed.
Committed Network proofs now use the same exact finalized-body authority and
explicit input-index/output joins with independently sized trees. State captures
and rechecks only the requested journal hash; no World view spans body I/O.
The raw endpoint returns original authenticated storage bytes, and complete
proof response sizing precedes large output/transcript clones. Wire and work
ceilings remain finite; complete decoded-memory reservations are still required.
Genesis validation and ValidBlock event projection now consume validated full
typed output caches and explicit original Network indices. Internal rows do not
invent Network transaction events or route authority; reveals use their actual
inner signed identity while routing by the committed outer source. Seven old
consumer controls are retained and four new controls are authored, not run.
Core library249 and full unit-test target264 now compile;264 reports zero errors
and 168 warnings on 20,613 unchanged tracked/unignored local inputs. Its unchanged
executable passes 95 output/Time/constructor controls in265 and125 query controls
in266. Six of nine additional trigger scenarios in266 stop at old event-order
snapshots. Eight explicit runtime JSON corrections then pass all nine scenarios
and their economic assertions in272 with the unchanged264 executable; the
compiled source and fixture epochs are recorded separately. Torii library275 passes after migrating 68 stale output/Time API callers. Core's
full unit-test target282 passes with zero errors/168 warnings; runtime278 passes
all 137 selected actual query, finalized-reader and trigger regressions on the
same 20,615 unchanged inputs and exact private executable. Three new reader tests
cover exact wire/work admission, budget refusal before body I/O and corrupt durable
wire despite a warm cache. State captures/rechecks its canonical hash without a
World guard across I/O; request-local visibility propagates failures on finish.
Torii's full test target277 initially fails 165 stale fixture diagnostics. Complete
typed-output/proof migrations and genuine parent-linked finality fixtures produce
a passing full unit-test target297 (zero errors/424 warnings). Selected runtime298
and299 pass all 46 history, status/details, proof, native custody and push recovery
controls on its exact executable and unchanged 20,616 standard captured inputs.
Two stale refusal-message assertions and two dropped broadcast-sender fixture
lifetimes found in292's selected runtime are corrected without weakening assertions.
The signer fixture is test-only and included in the normal source capture.
Cold whole-chain caches need anchored request pagination or a separately owned,
finite durable index: chunking alone cannot bound retained full-history rows.
Complete decoder/retained allocation budgets remain unestablished. No full Torii
runtime, production publication, current MAIN or network pass is claimed.
The actual ordinary driver authenticates genesis explicitly, retains strict signed
transaction time, and reads ABI/gas policy from the acquired World overlay. The
replacement test checks actual reverted gas policy and error-path guard release.
Earlier253 passes 44 World-preparation/DA/membership/genesis/fraud/time controls;
254 passes 13 World projection controls on its earlier source. These are separate
scopes, not a full Core or release pass. The exact images, manifests, failures and
runtime receipts are retained under ignored
`dist/sumeragi-liveness-redesign-20260916/canonical-owner-checkpoint-267/`.
Full State/runtime publication, witness capture, resource admission and SCCP
applied outbox remain unfinished. The native common execution owner is implemented
and has selected runtime evidence below; publication still requires its complete
authenticated State owner. Production
publication guards remain closed. MAIN source integration, full formal/workspace/
SDK and real four/seven-validator qualification remain open.
Typed telemetry validates complete output/cache shape and counts only explicit
Network joins; it does not yet authenticate exact finalized body bytes or admit
complete body/decode/work cost. Apply diagnostics count rejected typed outputs
without changing validation or finality. Their five new tests remain unrun.
Bridge now validates full output/cache structure and joins Network sources by
input index. Its 73 migrated controls and four new corruption/finality controls
are unrun. Successful internal/chained outbound records explicitly refuse until
the actual SCCP applied-outbox owner exists; replay ordering/custody remains open.
Failed intermediate candidates 104–106 remain retained. Mechanical migration removes 1039 unique obsolete None header
arguments; the direct-identity test now attaches a checked full Network output.
Stripped-header guards retain real context fields; output authority cannot live
in the proposal-only Header. Ordinary AMX projection validates the complete typed
output cache and follows explicit Network joins; its certified-merge branch is
preserved and native-group receipt integration remains open. New cache-tamper and
original-commitment controls are authored, not executed. The State capacity,
authority-rekey, admission, history/proof and Torii controls also remain unrun.
Finish actual State output production and remaining consumers/fixtures before
claiming their qualification. Bound positive index selection/continuation without cloning
historical height sets; integrate decode-graph and resident-memory reservations
before claiming end-to-end query/proof bounds or enabling canonical fanout. Internal invocations require their own output-history/proof owner. Source audit
finds that the production runner leaves the SCCP header root unset; execution-then-
fill is test-only. Replace this outcome-dependent proposal field with one bounded,
rollback-safe applied outbox manifest and QC execution root/count, migrating State,
replay, archives, proofs/circuits, SDKs and formal bindings atomically. No speculative
header rewrite or native SCCP activation is qualified. Three identical OpenAPI source copies
now describe the changed header/details and finalized Network proof contracts,
including byte/work refusals and storage failures. Static document validation and
two mirror/parser checks pass; Torii runtime checks and clean-source release
metadata regeneration remain unqualified.

Native accepting/commit paths remain closed until the sole consumer, durable
Apply and atomic old-signer/Ordinary economic bypass retirement are qualified.
Native query/proof/fee observability and fair runner/transport/refresh remain
required. All L1–L6 and the real four-validator counterexample remain OPEN.
Four-/seven-validator acceptance must use one unchanged completed candidate.

#### Native execution and snapshot predecessor checkpoint360

Core/MV full test compilation337 passes on20,625 unchanged inputs; all56MV tests338
and192 selected native/runtime tests341 pass. These cover actual native/common
Network/Pipeline/Time execution, retained runtime/context undo, exact special-store
snapshots and prior native fixture regressions. Snapshot module341 aborts after
nine passes at can_read_multiple_blocks; the other85 are unqualified. LLDB345
identifies a defaultWorld construction inside schema discovery on the restore
stack. Static derive-provided JSON field order347 removes that construction.

The frozen successor includes finite coherent capture321, derived UAID bindings331,
native observation fence335, fourteen mandatory service-state envelopes342/346,
recipient/confidential indexes343, trigger indexes344 and unified account identity
indexes352. Compilation359 passes on20,634 unchanged inputs after the355 missing
import is corrected in358. Runtime362 passes301/319 selected tests, including all
new predecessor controls; fourteen snapshot cases abort and four fail SCCP fixture
commitments. LLDB363 locates large by-value State frames across nested restore.
Heap-owned restoration365 and corrected SCCP proposal fixtures364 compile in370.
Runtime372 passes345/346 selected tests, including all94 snapshots on normal stacks.
Its one remaining SCCP fixture predecessor defect is corrected subsequently in374.
Daemon373 reports35 retired output/header API diagnostics; migrations375/376 and
account-scope predecessor371 compile in381. Runtime383 passes355/356 selected
checks on20,636 unchanged inputs, including all94 snapshots, all192 native/runtime
controls and all five new account-scope cases. Its remaining stale configuration
fixture is migrated in388. Daemon382 builds only the zero-test launcher; actual
library compilation392 exposes one corrected structural Time-output fixture error.
Core395 reports the anonymous-lifetime edit387;398 restores a stable named input
lifetime. Frozen399 builds Core/MV400 and actual daemon library401 on20,638 unchanged
inputs. Core403 passes all389 selected tests in194 processes, including ownership386,
retained physical dataspace guard391, sole World fee markers389, replacement
membership397 and all94 snapshots. Daemon407 passes23/32: nine Musubi cases fail
initial-genesis admission before their finality assertions. Additional daemon412
passes11/11 startup and authoritative-execution controls. Authenticated genesis
setup408 reaches the unfinished complete-State publication refusal; do not bypass
it or claim the stateful Musubi cases pass. Core/MV410 compiles20,642 unchanged
inputs, and recovery414 passes143/143: all ten new fee completeness405 and alias
predecessor406 controls, all94 snapshots, five Kura consumers and existing affected
alias/fee cases. Read-only exact execution-commitment test support408 and the
whole-batch fee helper417 remain subsequent unqualified work. Complete State
publication is the next required dependency. No full suite or runtime cutover is
inferred. Norito361
passes191 grouped codec and60 derive unit tests after357
registers the missing test harness. The original membership-ledger agreement case
and codec guard356 pass on354.

Audit340 additionally requires account scope, asset/domain/NFT/RWA/custody/game/VPN,
contract/asset alias and remaining reverse indexes to retain their actual prior
projections. Current-only reconstruction cannot satisfy replacement semantics.
Trigger consumer clarification limits the observed active-index defect to query
visibility and projection roots; Time/Pipeline matchers read action stores directly.
The source-bound receipts and failures are preserved in ignored
`dist/sumeragi-liveness-redesign-20260916/canonical-snapshot-checkpoint-360/`.
Complete-State publication, bounded capture allocation, State-owned witness, cold
history pagination, SCCP outbox, live runner integration and one-candidate
formal/SDK/workspace/four/seven-validator qualification remain required.

Frozen800 passes six-crate lib/bin test compilation801 on 20,686 unchanged inputs. Runtime809 passes 524 of 528 exact tests: all 73 MV tests and 451 of 455 Core controls, with unchanged source and retained executables. Three State publication/native ownership failures persist. One new failure identifies a receipt writer that rejects its own authenticated interrupted append before reaching recovery; source815 is in progress. Membership bindings795 and all 49 controls796 pass; exact800+803+805 source gate806 passes. World capture807 and actual carrier integration808 retain all 278 original World journals and four runtime cells after one admission; review correction816 makes the reservation outlive originals on early errors. Frozen810 contains these changes and is compiling in811; source gate813 is running. Archive writers, aggregate pre-vote resource ownership, the consuming publisher and real four/seven-validator qualification remain open. Direct receipt window exhaustion also lacks pre-vote admission and can reach fail-stop after commitment. No liveness goal is complete.

The September 19 MAIN implementation adds nonblocking preparation for the complete
World, replay membership, block hashes and four runtime journals. Each refusal
returns the original journals after releasing every acquired writer. A `Busy`
result carries an opaque observation from the actual refusing lock, captured
before its acquisition attempt; commit, abort, detachment and guard drop signal
only after physical release. MV current, undo and metadata locks have separate
sources so unwinding a partial acquisition cannot wake its own refused lock.
Hash readers must signal too. A hash read-guard panic is not writer poisoning; an actual
poisoned owner is a local reconstruction failure, never a consensus rejection or
an indefinite Busy retry. Pending async registrations are cancelable and require
caller admission; the primitive itself is not a complete candidate-memory policy.

The production completion criterion remains one move-only prepared candidate in
worker/body-store custody, joined by exact cached/recovered receipts, then consumed
under actual QC/Kura/Native authority. An execution-hash cache hit cannot replace
that owner. The immutable-store capacity goal still requires complete
geometry/receipt/archive/publication demand before canonical append; the current
shell pool does not supply that disk or aggregate memory guarantee. Partial
durable work retains the same recovery owner.
Acquire all State/component locks without waiting, return the full owner on local
deferral, and await its exact release outside the synchronous worker. Only the
complete publisher may expose State and return its post-WSV continuation. Existing
raw publication guards remain enforced until that path replaces its callers.


### Current exact-decision and geometry handoff, September 19, 2026

The DPN development continuation now retains the original raw and tiered geometry
attempts in State across local refusal, including replay State/receipt custody.
Build17 passes all 170 geometry controls. Build19 compiles and passes all 76
selected local-refusal, Queue-release, autoscale, carrier and recovery-fixture
controls on unchanged source and executable. Typed drain/storage failures remain
local through candidate validation and emit no rejection event; deterministic
errors still reject. Autoscale evaluation preserves its original sample/count
across refusal and marks lifecycle completion only after success. Eight strict
replay controls failed at fixture publication in those earlier selections. All
eight now pass on the merged Core artifact in DPN development run42; this scoped
replay result does not qualify the complete live Validate-to-Apply owner or the
four-validator recovery corridor. See the [dated DPN record](../docs/history/2026-09-19/dpn-recovery-geometry.md)
for the preceding failed builds and exact validation scope. The carrier now
resumes its original geometry under the retained Kura lease; local backend
contention releases all physical ownership before returning its actual release
observation. Complete pre-vote resource admission and source/Native authority
remain required before exposing the production consumer. Build28 separately passes
43 focused controls for original CatalogPublished completion, terminal journal
identity, retained DA/lifecycle projections, cursor persistence after generation
close, corrected DA fixtures and lock ordering. Actual index allocation and
complete resource/custody handoff remain unfinished. Build24 passes all 289
selected Core controls; all 105 MV tests and strict MV library/test Clippy also
pass. Original archive captures and their detached carrier survive insertion
refusal. The canonical runtime writer is acquired before State fences, preserving
undo and avoiding the prebuilt-block lifecycle deadlock. A private complete
decision-plus-lease owner now joins sealed wire, original witness commitment and
exact original Native admission evidence before State acquisition. Its pure
retained-evidence reads preserve body/hash/query caches. This immutable source
join grants no source release or State publication authority.

The private retained-validation service now reserves its bounded descriptor tables
before candidate preparation, installs the original journals before success-marker
fsync, and retains pending occurrences separately from confirmed receipts. Cached
and reproposed validation requires the same original owner. Selection abort restores
that owner; successful consumption keeps a subject tombstone until height-store
retirement so a delayed earlier round cannot execute again. Store-instance and
full original context/proposal checks fail closed. This adapter does not yet replace
the live scalar validator or supply aggregate resource admission.

The exact locked Concread source now has an EBR allocation-custody hook: admission
precedes cloning, the writer owns its allocated generation, commit moves that
allocation, and actual reclamation destroys/deallocates it before returning its
charge. Unrelated epoch pins delay both; clone/destructor unwind conservatively
retains the charge. Build29 passes 105 MV tests, four allocator/epoch controls and
two negative API checks. It passes 75/76 selected Core controls; build30 closes the
original store before the reopened-incarnation fixture and passes all six final
custody controls on unchanged production source. Strict MV Clippy passes. Build31 adds original current/undo MV Cell ownership through detach, retry, abort
and publication, a finite requested-layout credit pool, and raw-acquisition panic
wakes for Cell and Storage. All 120 MV library tests, five allocator/epoch tests,
75 focused Core controls, strict MV Clippy and the charged-API compile-fail control
pass on unchanged source. Existing State fields remain untracked. Original B+tree successors subsequently retain their exact cursor, undo, root and
reader identity through retry and publication. DPN runs32–35 pass 136 MV controls,
strict Clippy and the charged-API check on the frozen parent source; those results
do not qualify the subsequently merged tree. Complete actual node/cursor/nested
payload custody and an explicit configured aggregate policy remain required
for the outstanding complete process-memory goal. They are not supplied by the
scoped shell admission used by the retained handoff. The
[B+tree lifetime correction](../docs/history/2026-09-20/bptree-allocation-lifetimes.md)
repairs actual partial-clone and separator reclamation before attaching those
credits. The subsequent
[linear allocation custody](../docs/history/2026-09-20/linear-allocation-custody.md)
attaches prepaid opaque charges to exact original cursor/reader control blocks,
retains them through publication and reader-chain reclamation, and removes heap
allocation from final-tree teardown and first refund notification. Actual maps
still use explicit untracked shells pending complete node/vector/nested payload
funding. Neither these hooks nor a post-execution callback supplies the configured
aggregate policy or activates the live retained validator.

The subsequent [refund notification custody](../docs/history/2026-09-20/refund-notification-custody.md)
closes the reproduced loss of remaining waiters after a callback panic and adds
an allocation-free lexical deferral for the original credit pool. Actual freed
credits return immediately; same-thread callbacks wait for physical unlock,
while other-thread refunds remain independent. The next map boundary remains a
closed, completely prepaid edit with typed nested payload ownership and original
retirement buffers; unrestricted mutation cannot satisfy that admission contract.
The [prepaid writer/node boundary](../docs/history/2026-09-20/prepaid-writer-and-node-custody.md)
now consumes original move-only writer input under locked admission and attaches
typed charges to each exact padded node allocation through clone, split, unwind
and deallocation. The [charged cursor/retirement boundary](../docs/history/2026-09-20/charged-cursor-retirement.md)
now threads the provider and concrete charge through that same engine, retains
original fixed buffers through real reader release, and refuses deficient
bookkeeping before mutation. Shared traversal and clone probes use immutable
references. Production MV maps remain explicitly untracked until concrete model
payload policies and complete initial/undo ownership are connected. These remain
requirements of complete process-memory admission, beyond the current shell pool.

The [explicit payload-cloning boundary](../docs/history/2026-09-20/prepaid-payload-cloning.md)
requires each node payload copy to use the original provider's policy. Node
credits alone cannot authorize ordinary Clone; complete operation demand and
concrete MV payload owners remain required before production funded edits.
The [closed insertion boundary](../docs/history/2026-09-20/closed-admitted-insertion.md)
now threads a sealed prepaid mode through the existing public map owners and
plans complete insertion storage/copy demand under the original lock before one
reservation. It returns only a completed detached successor and releases unused
admission before handoff. The [retained edit extension](../docs/history/2026-09-20/retained-admitted-edits.md)
admits subsequent edits against that same original cursor and funds its exact
initial root allocation. Refusal preserves private work; publication remains
atomic. Native mutex/runtime storage, real model payloads, MV undo/transaction
storage and configured aggregate policy remain required for complete process-memory
admission; the current production-adapter candidate makes the narrower claim above.
The [borrowed checkpoint](../docs/history/2026-09-20/borrowed-map-checkpoints.md)
now preserves an already funded parent root and its original tracking buffers
through nested child edits. Abort needs no rollback allocation or admission;
caught mutation/cleanup panic makes the original cursor unpublishable. The
[native Storage undo owner](../docs/history/2026-09-20/native-storage-undo.md)
replaces the block-undo standard map with that same tree engine, preserving both
original generations through snapshots and publication retry. The live
[Storage transaction checkpoints](../docs/history/2026-09-20/storage-transaction-checkpoints.md)
now retain both original parent roots and borrow transaction preimages directly.
Abort restores both trees without allocation, cloning or inverse edits; caught
preimage-clone and owned query-key destruction panic cannot apply partial work.
The [joint Storage admission](../docs/history/2026-09-20/joint-storage-admission.md)
now reserves current edits, first block preimages and charged ordered touch storage
before any mutation. It retains both checkpoint retirements through private apply,
and both physical publication owners through joint root/identity installation.
The same stages cover EBR Cell pairs and current-only replacement, retaining
unscheduled old allocations until unlock before entering the epoch collector.
Successful physical release disarms poisoning during later retirement cleanup.
Arbitrary cleanup and wake callbacks run after every participant unlocks. Carry
these owners through concrete model payload policies, closed mutation,
State generation refusal and configured aggregate memory/work admission; native
runtime and identity allocation remain outside this boundary. The
[allocation-free State scan](../docs/history/2026-09-21/allocation-free-state-scans.md)
removes both MV read-iterator boxes and dynamic B+tree traversal stacks from the
live engine. Actual allocator controls cover committed and private State views,
history, retained readers and an exhausted prepaid pool; this funds no mutation
or candidate payload by implication. Closed admitted map removal now uses the same
engine for both modes, funding path/sibling/separator copies and bounded tracking
before mutation. Missing-key removal needs no admission; nested rollback restores
actual original nodes at full capacity. The [joint Storage removal](../docs/history/2026-09-21/joint-storage-removal.md)
now shares insertion's complete admission and original rollback owners. Missing
keys retain explicit first absence and touch without dirtying a clean block;
returned private values keep their original charges. Default and skinny MV suites
pass; the linked record tracks final validation and remaining activation boundaries.
The actual BlockHashes block, StateView and QueryView paths now retain original
shared height-indexed tree generations. Append and replacement edit private paths;
opening releases the hash writer before World acquisition. Final publication
reacquires the original physical root and exact predecessor; restored equal bytes
and same-height ABA cannot authorize it. Canonical readers use indexed/range access
and snapshot JSON streams the same ordered array. Native State identity retains
the actual tree family, while emergency Fast mapping remains read-only and cannot
open Native execution. See the [integration record](../docs/history/2026-09-21/shared-block-history.md).
Checkpoint131 uses one configured original Kura pool for
prepaid hash construction/restoration and successor creation before World/start
effects. Its hidden private tip requires no further credit to finalize; exact
current footprint distinguishes an impossible bound from release-driven pressure.
Validate, global proposals and autonomous prefix selection retain typed local
refusals. Shipping checks, the nine-package test build and scoped regression
controls pass with exact final executable joins; the integration record retains
failed captures and distinct formal/vendor scopes. Native lock/runtime provisioning
and complete resident capacity remain open. Each World field migration must propagate
its concrete mode through capture/publication and return local capacity refusals
through execution before that field can be claimed as completely funded. The
retained Validate-to-Apply candidate preserves ownership without claiming those
nested allocations are prepaid.
L1–L6 and four/seven-validator qualification stay open.


The retained-journal owner now consumes its actual `ValidBlock` through the existing
verified-finality transition. It retains the original World/runtime/membership/hash
journals, witness, output/source seals, archive plans, deferred events and reservations.
Decision binding checks the execution-prefix commitment, exact proposal bytes/header
and semantic frozen context identity. Equivalent valid parent CommitQC witnesses do
not create different context identities. Refusal returns the original journals and
verified artifact. Binding does not publish State or deliver its committed event;
source/economic, retirement, Kura/Native and original physical publication authority
are still required. Binding adds no separate memory-admission reservation.

Certified merge execution now retains the actual application header through pristine
geometry and writes-root checks, restoring the outer carrier header on success and
error before carrier staging. Late geometry capture checks the complete application
header, including time. Final journal admission precedes allocating geometry capture,
and its reservation outlives originals on every failure path. Build14 passes all
96 focused composition controls; the full historical recovery test still reaches the
competing-Native output refusal. Keep that guard until the complete consumer exists.

The next production change must connect the complete consuming publisher to retained
Validate ownership, cached/recovered authority and Apply together. A cloneable execution
hash or a discarded preparation object is insufficient. Test harness migrations retain
all assertions and use only existing nonshipping fixture authority; they do not establish
production source/finality or release readiness. All six goals remain open.

The current default governance integration target now compiles. Its 18 migrated
instruction controls and seven of eight relocated structural library controls pass;
the retained signed-ballot test still fails at its synthetic genesis's missing
authenticated Network source. A correct genesis alone is insufficient: its sealed
outputs require the aggregate publisher before the H2 commitment can become the H3
reveal prerequisite. Preserve rejection-side slash effects, both exact replay keys
and economic assertions when completing that path. Build14 also passes all 17
original-review controls and 47 current formal/source-ledger checks on unchanged
inputs. No ignored test, compatibility method or shipping fixture authority was added.


### Joint physical acquisition, September 19, 2026

The complete decided carrier now owns one nonblocking acquisition attempt across
its original Kura and State. The original shell/effects reservation already travels
with it; physical preparation adds no installation admission callback. Exact Kura
instance identity is joined before physical probes. Acquire Kura prune, canonical, geometry
and sidecar fences, then State commit, lifecycle and write fences, then the original
hash, membership, runtime and complete World writers. Hash acquisition precedes
World acquisition to avoid a cycle with an existing validation overlay. Refusal
returns the same decided block, verified artifact, journal allocations and source
seals; no StateBlock is reconstructed and no execution is repeated.

One shared physical mutex owner serves State and Kura. Each Busy result observes
only its actual failed lock before probing it. Component cleanup, unrelated locks
and other Kura/State instances cannot wake that retry. Normal release, unwind and
fair unlock notify after the physical guard releases. The fence group releases
all component writers before State fences and State before Kura. The one original
shell/effects reservation outlives the carrier payloads and all physical ownership;
there are no additional decision-binding or installation reservations.

Canonical poisoning is permanent and refuses before any probe. The prune recovery
flag is different: healthy active pruning sets it while holding its prune fence.
Check it only after acquiring that fence; a live prune remains a reachable Busy,
while an abandoned intent requires storage recovery. Recheck both conditions under
the complete storage lease. Neither Busy nor storage repair invalidates finality.

Build18 passes combined compilation, all 158 selected runtime controls and 131
formal/source-contract checks on 7,215 unchanged inputs and unchanged executables.
The runtime selection contains 111 composition controls (including nine joint
Kura/State and six Kura-fence controls), all 17 original-review regressions and 30
existing storage/restart controls. All 37 changed Rust files pass formatting.
These stages grant no source/economic, checkpoint, retirement or publication
authority. All six liveness goals remain open.

The checkpoint receipt draft is unregistered. Its descriptor-relative promotion
would make the shared writer fail outside Unix; the current dependencies lack a
qualified Windows namespace-durability owner. The final storage reauthentication
must also run through methods owned by the retained Kura lease without reacquiring
its fences, and before State component acquisition. Exact writer/readback evidence
and physical exclusion do not replace source, geometry or retirement authority.


### Retained execution and durable checkpoint custody, September 19, 2026

Ordinary execution now keeps its original invocation owner inside the sealed
output, rather than dropping it after inventory derivation. A private consuming
prefix owner verifies the exact attachment, original inventory and witness before
it alone stages metadata and the World tail. Raw State receives only a closed
transfer marker; both transaction-apply paths reject it. The complete source,
proving context and casting bindings move into detached journals under the same
whole-candidate admission. This does not mint Native or old-merge authority.

The shared checkpoint writer synchronizes the exact file and every held parent
through Kura root, then authenticates canonical stable readback. An identical
retry synchronizes the original file instead of replacing it. Malformed temporary
entries are rejected inside physical resource accounting; failure keeps that
inventory unavailable until actual re-audit. Its move-only receipt retains the
actual file and ancestor handles, exact finality and State hash, and original
Kura. Only a receipt-bearing decision can attempt physical acquisition. It must
reauthenticate this storage join under the original Kura lease before acquiring
any State fence. Refusal and abort return the same complete decision and receipt.
No State publication authority is supplied by attachment or physical exclusion.

Combined build21 passes with 7,218 unchanged recorded inputs. Its unchanged
executables pass 287 distinct runtime controls: 240 composition/output, all 17
original-review and 30 existing storage/restart controls. All 201 selected
formal/source controls and 50 changed Rust formatting checks pass. Four failed
build20 controls were rerun after restoring temporary-path validation and moving
large test fixture States to the heap; no assertions or default-stack limits were
weakened. Build19's test-path/type migration failures remain recorded separately.

Complete consuming publication remains open: Native/source/economic ownership,
geometry and retirement, bounded resources, archive/cache/events and the actual
Validate-to-Apply worker handoff still must join. Native Windows namespace
persistence is unqualified; there is one strict writer and no fallback protocol.
Production native/beacon cutover and unchanged four/seven-validator qualification
are still required. All L1–L6 goals remain open.


### Native source custody and retirement branch bindings, September 19, 2026

The actual Native source consumer now moves its original verified group vector
through execution into the private prepared stage. First-carrier bytes/finality,
RS16 body buffers, all affected-route contexts and Decisions remain owned beside
the actual overlay/results; no wire reconstruction replaces their provenance.
Construction checks the ordered payload/Decision join, and dropping the stage
releases overlay writers before the retained evidence. The live Native gate and
raw commit refusal remain closed pending complete publication authority.

The retirement proof-ledger gate now binds the real seven-pair maintenance
owner and its returned directory, including module presence, inherited-lock
order and failure propagation. Separate branch-specific relations bind committed
history-rewrite authentication and subsequent compaction authentication. The
ledger and expected tokens change together; mutation tests cover both branches,
retention bounds, original frontier input and recovery order. The current runtime
maintenance/scan remains route-local under its original fences. Moving it to a
pre-vote phase still requires bounded retained observation and actual competing
artifact-writer exclusion; reopening routes or retaining unbounded handles does
not supply that authority.

Build23 passes compilation and 331 distinct runtime controls on 7,218 unchanged
recorded inputs, with 218 selected formal/source checks and 45 mutation subtests.
All 44 Native source/economic/replay controls pass on the default stack. Build22
had passed compilation and 289 of these 331 controls: 42 failed during the shared
fixture's genesis metadata setup, as confirmed by LLDB. The fixture now returns
from World/State construction before acquiring nested block writers and retains
the original heap State. The frame active during genesis falls from 919 KiB to
527 KiB; all assertions and protocol setup remain. Build22's accepted malformed
rewrite mutation is also retained as a failure, and now rejects in build23.
The prior full 70-case Native-preparation source suite remains build22 evidence;
build23 reruns its current-owner and release-connection checks on the new ledger.

The next complete production boundary must acquire the witness recorder after
actual State writers, retain that one scope across shared start effects, Native
and internal output phases, mandatory controls and metadata, and capture it once.
Scratch recording suppression and ordinary witness reset cannot be reused as a
Native validation path. Distinct native-output and later metadata roots must keep
their original meanings. Source/economic, geometry, resource and checkpoint owners
must then join the consuming publisher and the original Validate-to-Apply handoff.
Four/seven-validator qualification and all L1–L6 completion criteria remain open.

### Native recorded execution and metadata, September 19, 2026

The private prepared source can now move its original verified groups through
actual output sealing and checked witness capture. Thread-local recorder
eligibility is checked before State access, then one recorder is acquired after
the original State writers and source preflight. It remains owned across shared
start hooks, Native Network economics, prefix/application markers, Pipeline,
Time, real metadata, lane-context finalization and capture. Failure drops the
unpublished overlay and its recorder. Scratch retains its existing isolated
semantics. Native pin expiry now uses the same after-hook, before-Network order.

The stage seals actual per-group settlement hashes. Metadata rechecks exact
sources, aliases, coordinates and those commitments before any finalizer writes.
The ordinary common prelude and remaining tail are byte-equivalent to their
previous bodies. Receipt-free Native groups emit no old AMX relay statement;
actual receipt effects still require the unchanged complete relay integrity and
real policy-root contract. No receipt or root is fabricated. The completed
output cut and later metadata net-delta cut retain their distinct meanings.

Build25 passes combined compilation and 340 distinct runtime controls on 7,220
unchanged recorded inputs: 240 composition/output/publication, 53 Native and
recorder, 17 original-review and 30 storage. Nine new controls cover actual
source custody, transfer/rejection/reveal and aliases, due-hook capture,
Pipeline/Time once, settlement substitution, late rollback, recorder nesting,
and refusal while another thread holds State. Formatting covers 68 changed Rust
files; the codec guard passes. The preliminary compiler check caught one new
settlement-type module typo. Build24 then compiled and passed 52/53 Native
controls; its only failure was a three-case fixture exceeding the two-case
builder bound. Build25 uses independent single/atomic fixtures, preserving the
assertions and default stack. The build24-to25 input comparison proves only
that new test file changed; production and formal source stayed identical.

Scoped source/inventory/mutation qualification passes 118 unique controls. The
three live-index pytest cases remain deliberately deselected: the unchanged
release rule rejects an untracked Native metadata provider. Equivalent exact
Block/host/Kura closure checks pass using an isolated reviewed index, with the
main index unchanged. Final25 repeats the actual-owner and release-connection
joins. These are scoped binding checks, not a full formal release result.

The recorded owner is disposable and cannot mint ValidBlock or publication
permission. The live gate remains closed. Mandatory control composition,
authenticated opening of a pending suffix context, genuine receipt-bearing
relay runtime coverage, aggregate resources, the sole consuming publisher and
original Validate-to-Apply custody remain required. Existing scratch permits a
recorder-owning caller to acquire State; that concurrent inversion must be
resolved before activation rather than inferred away from the new recorded
entry check. The concrete PipelineGas/SpaceDirectory economic fixture plan is
retained in the build evidence for the next qualification step. Four/seven-peer
fault/restart/final-transaction qualification and every L1–L6 goal remain open.

### Native recorder order and genuine economic relays, September 19, 2026

Every Native source-preparation, source-stage, batch snapshot/replay and shared
execution entry now checks thread-local recorder ownership before accessing
State. Suppression does not waive this check: it prevents witness writes but
cannot break the State/recorder wait cycle. Recorder-owning calls return a local
`ExecutionRecorderConflict`, not an invalid-input decision. Recorded execution
still takes its recorder after the original State writers and source preflight.
Source-only preparation retains its existing string error API and refuses before
State observation. This establishes the scoped Native boundary; it does not
claim an audit of every State API or open the production admission gate.

Scratch regressions retain actual due-hook/transfer effects, constructor refusal,
late-marker failure and rollback outside capture. Calls attempted during an
unrelated real capture now prove early refusal and unchanged witness bytes and
generation, including a pending real-transfer overlay. Their recording fixture
also acquires State before capture. A bounded contention regression exercises
nine Native source/replay entries, with suppression active, while another thread
retains the physical State writers. It proves refusal before writer release.

Four genuine PipelineGas controls cover single and atomic Native inputs. Signed
Nexus/PipelineGas maxima and gas policy exist before first admission; execution
uses the original funded State, four-validator authenticated source and signed
RS16 Decisions. Success binds the actual receipt, source identity, exact fees and
supply, canonical settlement, coordinator route, descriptor, manifest root and
signed payload byte count to the final statement and complete witness. The root
comes from an actual Space Directory record. One atomic input produces one
coordinator economic effect. Missing-root controls first demonstrate genuine
scratch receipt production, then require recorded metadata refusal with complete
rollback and recorder release. No receipt, root, or economic output is injected.

Combined build27 passes on 7,221 unchanged recorded inputs. Exact emitted
executables pass all 346 selected runtime controls (240 composition/output,
59 Native/recorder, 17 original-review, 30 storage). The current source passes
118 scoped preparation/inventory/mutation controls, both original-review
ledger/production joins and three separate reviewed-source closure checks using
the isolated index. The three main-index closure cases remain deselected for
untracked providers; no release rule or main index was changed. Formatting of
69 changed Rust files and the codec guard pass, without stack overrides.
Build26 first compiled and passed 344/346 controls. The only failures were the
new missing-manifest fixtures assuming a retained derived UAID binding despite
startup rebuilding it from actual manifests. Build27 removes that redundant
seed, compares the actual optional root and requires `MissingManifestRoot`;
production and formal sources are byte-identical to build26.

The next code boundary is the actual pristine canonical control owner from
`ValidBlock::state_block_for_execution`: consume it after exact Native source
preflight and recorder acquisition, before shared hooks, and retain the original
authenticated applying context through suffix finalization. Common AXT/DA/SCCP
postchecks must precede the one capture. Do not run another validator against an
already retained overlay or open the live gate with these components alone.

Mandatory control composition, suffix-opening authority, sponsor/lease relay
coverage, aggregate resource admission, complete consuming publication,
Validate-to-Apply custody and production activation remain required. Real
unchanged four/seven-validator fault/restart/final-transaction qualification,
full workspace/release checks and all L1–L6 outcomes remain open.

### Authenticated Native controls and complete source custody, September 19, 2026

Ordinary and recorded Native execution now share the actual pristine NPoS
preparation and application owner. Native construction requires the original
verified height context and rejoins it to the current State generation, network,
height, parent, execution policy, Nexus context and mandatory DA proof policy.
Beacon validation runs against actual committed State. Complete QueuePlan inputs
and prepared NPoS effects apply on the same pristine State overlay, after writer
and recorder acquisition and before shared hooks. A foreign State or changed
pristine owner is refused before writes. Admission and applied-NPoS hashes join
the staged Native seal, preventing control replacement after execution.

The original verified context authorizes pending suffix opening. Actual staged
control, AXT, DA and SCCP postchecks precede context finalization and the single
witness capture. No-bundle DA cursor validation returns before live cache
hydration: there is no cursor work, and reacquiring the hydrator while retaining
State writers can deadlock with cache rewind waiting for those writers. Actual
DA bundles retain the existing checks; Native DA/pin/SCCP payload execution still
requires its own complete owners and remains explicitly unsupported.

Both proposed and authenticated finalized/recovered Native sources retain the
whole resultless carrier, including its controls. The recording consumer compares
the complete carrier before releasing its duplicate and moving the original
source groups into execution. The source and finalized owners no longer retain
a second cloned batch. Scratch explicitly refuses controls it cannot execute,
so it cannot erase an admission or NPoS attachment during projection or replay.

New runtime controls use real signed single/atomic inputs, four-validator Kura
finality and actual RS16 contexts. They cover a genuine pending suffix and exact
successor opening, a third same-carrier admission retained for later execution,
real requested threshold beacons, missing/corrupt/foreign-parent refusals,
constructor and consumer context mismatches, active DA policy binding, foreign
or stale State owners, seal replacement and cold-cache validation under retained
writers. Failure paths require unchanged State and recorder release. Economic
fixtures sign their actual execution policy at genesis instead of patching a
verified successor. Existing genuine PipelineGas relay regressions still pass.

Combined build29 passes Core, Torii, test-network, Kagami and daemon compilation
on 7,037 unchanged compiler inputs. All 359 distinct exact-binary runtime controls
pass: 244 composition/output/publication, 68 Native custody/control/economic,
17 original-review and 30 storage. Build28 also passed all 359; the only two
Rust files changed for build29 remove redundant batch/header storage, and the
complete runtime selection passes again. No stack override was used. All 109
Native preparation checks, 22 source-inventory checks and two original-review
binding joins pass. The final formal source set contains 8,113 unchanged inputs;
all live-index cases run, with the main index and HEAD unchanged during that run.
An earlier serial Python harness was stopped and superseded by four disjoint
Native shards plus concurrent inventory; its partial output is not qualification.
The externally advanced `optimizations` HEAD is recorded in both final manifests.
All 15 changed Rust files pass formatting and the codec guard passes; unrelated
existing workspace formatting differences remain. Local terminal evidence is
`dist/sumeragi-main-work/validation29.json`.

This is scoped component progress. The actual global validator still needs to
retain complete Native preparation and authenticated context through the
validated prefix, cache/reproposal/recovery, voting and original Apply. Aggregate
resource admission, the complete consuming publisher, genuine remaining control
families and atomic production cutover remain required. The production Native
gate stays closed. Real unchanged four/seven-validator loss, reordering,
backpressure, leader-failure, restart and final-transaction qualification, full
workspace/release checks and all L1–L6 outcomes remain open.

### Source-owned Native carrier preparation, September 19, 2026

The original prepared Native source now enters the existing private carrier
preparation owner through common global validation. Exact origin-signature
verification is shared with V2 body storage, retaining the frozen original-view
leader, exactly one signature and its authenticated index. Canonical time,
parent, height, network, confidentiality/DA policies, payload limits, proposal
commitments and snapshot checks precede actual recorded execution. Only this
private preparation profile admits the supported Native shape; the live header
gate and raw State publication refusal remain closed.

The original verified applying context now survives recording. Capture retains
the immutable original stage allocation, all source groups and executions through
an exhaustive ordinary/Native prefix owner. The actual sealed outputs, source
inventory, witness and proving context move once into the existing metadata tail
and lifetime-free journals. No empty legacy AMX manifest grants Native authority,
and no source, witness or overlay is reconstructed for thread transfer.

Preflight returns refresh before consulting an obsolete source. Both its initial
current check and its post-preflight fence use the retained source generation;
sampling a fresh generation after the current check would reopen a publication
race and could persist a transient height/policy mismatch as invalid. The new
regression publishes an actual later QueuePlan admission with verified 3-of-4
finality before consuming the stale source, then checks no further State, Kura,
generation or economic mutation. Equal-byte generation advancement remains a
separate control.

Build30 passed combined compilation but only 124/127 Native/signature controls.
LLDB located all three new success-path stack overflows in genesis fixture setup,
before Native preparation. Build31 separates fixture construction from assertion
frames without changing production stack limits or weakening assertions. Build30
also passed 144/145 source/inventory checks: its short repeated Native predicate
failed to detect a branch mutation. The binding now covers the complete Native
match arm; two added mutations cover source-current and retained-generation loss.
These failed runs remain in `dist/sumeragi-main-work` alongside terminal receipts.

The checkout advanced through a concurrent commit and merge before build31's
source freeze. That merge also changed Pasta arithmetic profile optimization and
ancillary diagnostic/test-network code. Build31 therefore uses freshly compiled
artifacts for the complete merged input; build30 binaries are not qualification
of those changes. The source-difference receipt retains that distinction.

Combined build31 passes Core, Torii, test-network, Kagami and daemon compilation
on 7,038 unchanged Rust/configuration inputs. Its exact emitted executables pass
419 distinct controls: 244 composition/output/publication, 128 Native/signature,
17 original-review and 30 storage controls. All eight new Native preparation
controls pass on the default stack, including real single/atomic economic effects,
pending suffix controls, original-allocation custody, thread transfer, malformed
global proposals, actual publication staleness and raw commit refusal. All 149
scoped formal/source checks pass on 8,114 unchanged inputs with no index alteration
or source-inventory deselection. Formatting of the 24 Rust files in this work span
and the codec guard pass. Evidence: `dist/sumeragi-main-work/validation31.json`.

This establishes private Native preparation, not production activation or release
readiness. Remaining DA/pin/SCCP, broader NPoS and sponsor/lease owners, aggregate
capture/resources and the complete consuming publisher must join original
Validate-to-Apply custody across fresh, cached and recovered bodies. The production
cutover and unchanged real four/seven-validator fault/restart/final-transaction
campaign remain required. No L1–L6 goal is complete.

The next connected implementation is the terminal consumer of the existing
`PhysicallyPreparedCarrier`, using the retained source prefix and all actual
deferred effects. Archive capture currently authenticates through public Kura
readers which reacquire its fences: consume that retry-aware durable work before
joint acquisition, or supply a real lease-scoped authentication path. Geometry
and Queue retirement must retain their real authority and outer lock order;
capture alone is not permission to change storage. Publish all retained component
journals within one State visibility interval, and classify any later durable
failure as post-publication recovery. Only then connect the production worker's
fresh, cached and recovered validation owners to this complete consumer.


## 2026-09-19: terminal carrier publication, still before production cutover

The private consuming publisher now moves the exact ordinary or recorded Native
source owner, verified decision, original Kura checkpoint and detached journals
through joint acquisition and one State visibility interval. It publishes original
membership, all four runtime cells, World and its retained DA pin effects, DA
commitment caches, SCCP cache, block hashes and the latest header. Physical fences
release before cursor persistence, relay hydration, tiered persistence, storage
budget and query completion; Apply serialization survives those completion steps.
There is no retryable error after the first component write and no second execution.
The result retains the original source, events and original shell/effects
reservation. The former independent decision-binding and installation admission
parameters are removed; actual archive and physical publication owners remain.

Provider and reputation captures publish before the final joint lease; a successful
provider survives reputation refusal with its exact file identity and original
reservations. Final checkpoint authentication is repeated under the final lease.
Review found that body/QC/checkpoint alone did not establish the actual execution
witness required by the next height's context reader. The aggregate now stages
and promotes the original retained witness and casting bindings with the actual
finality receipt, then requires its final artifact/root-bound proof under the
lease before acquiring State writers. Missing and staged-only proofs do not grant
permission. Corrupt/foreign proofs and Kura Busy preserve original custody and
leave State unchanged. Existing idempotent stage/promote preserves the final
object but may recreate identical staging on later physical retries; complete
production resource admission/continuation must account for or eliminate it.

All 15 new archive/witness/terminal tests pass, including actual single and atomic
Native Transfer(25) publication, mandatory beacon and complete admission effects,
exact checkpoint, authenticated suffix contexts, source allocation retention,
abort/retry, single visibility interval and release of original reservations.
The geometry identity test and four prior capture checks also pass. Build33
passes the combined five-crate/test-harness boundary with 7,045 unchanged
Rust/configuration inputs and 439 distinct scoped runtime tests (264 + 128 + 17 +
30), using default stacks. All 179 scoped source/mutation/inventory/binding tests
pass on 8,121 unchanged recorded inputs. The main index and HEAD are
unchanged during qualification. The codec guard and 21 changed Rust files pass;
full workspace formatting retains five unrelated differences. Build32's single
missing HeightContext fixture path and its unchanged input receipt remain recorded.

This is not an L1–L6 completion or release claim. Pending geometry/Queue retirement
and nonempty old participant durability explicitly return the original carrier
before visibility. Complete process-memory and immutable-store disk accounting
remain outstanding goals. Original worker-owned Validate-to-Apply custody,
cached/recovered round aliases, async owner-bound local
deferral, complete geometry/participant publication, atomic live cutover and the
unchanged four/seven-validator loss/reordering/backpressure/leader/restart/final-
transaction campaigns remain required. Native live admission and raw State gates
remain closed. The next geometry owner must preserve Queue's outer retirement
fence, perform cold pending-capacity reads before geometry/sidecar locks, and
extract actual guarded geometry/GC/frontier-history operations; wrapping public
relocking APIs inside the current lease would introduce circular waits.

Local evidence: `dist/sumeragi-main-work/validation33.json`, exact build/source
receipts, four runtime summaries, formal shards and the read-only geometry plan.

## 2026-09-19: Queue retirement and original Kura geometry guards

Queue retirement observation now acquires the actual reservation-transition mutex
without blocking and returns that mutex's release observation on contention. Every
existing blocking caller and the exact pending coordinator/participant, reservation,
barrier and incarnation checks retain that physical owner. Kura transition and
catalog publication consume their original four guards through one implementation;
canonical recovery and capacity reads remain before geometry/sidecar acquisition,
and pending GC remains before sidecar acquisition. These primitives have focused
ownership and storage regressions and retain all existing Queue/geometry tests.

The connected aggregate still must retain the original Queue/State service pair,
admit the prelude and resources, preserve authenticated drain-history and directory
custody, acquire component writers before durable geometry publication, and expose
one State generation. `V2ApplyService` already owns the actual State/Queue pair;
the next connection must retain that pair through candidate detachment and retry,
and retain the inner Queue cut while keeping every later State/component probe
nonblocking. Nonidentity geometry and old participant durability remain
explicit refusals in the terminal consumer. A released mutex alone does not prove
that queued work disappeared. Live Validate-to-Apply custody, runner cutover and
unchanged four/seven-validator qualification remain open; no L1–L6 goal is complete.

Scoped validation and the retained failed fixture attempts are recorded in
[Queue and geometry publication](../docs/history/2026-09-19/queue-geometry-publication.md).

The pending-hash query now acquires State before borrowing the live Queue entry.
Its deterministic writer/removal test closes the identified indirect lock cycle;
it does not make the retirement predicate nonblocking or retain a negative scan.
Build43 retains the actual push/remove and reservation owners with exact release
observations. The aggregate must still keep later State writers try-only and
preserve complete original custody. The earlier build41 passed 23 controls; the
latest broader results are below. Production cutover and unchanged network
qualification remain open, as detailed in the dated record.

Build43 extends the observer into a retained cut over the actual enqueue/removal
and reservation owners. Both probes are try-only, refusal releases all attempted
guards, and exact failed-mutex release drives retry. All 359 Queue, 150 geometry,
eight publication-lock, 17 original review, two HTTP and one capacity controls
pass. Five additional Kura/State ownership controls also pass, for 542 distinct
runtime results. The cut is not yet retained by the production aggregate; original
service custody, admitted scan/allocation work and try-only later writers remain
required.

The admission-capacity outcome must bind ingress promises to a carrier that fits.
Build43's actual-configuration/assembler regression accepted a 512 KiB block limit
and 64 KiB headroom, then proved that an approximately 800 KiB genuine complete
input passing the protocol bound cannot fit. Two refusals retain exact durable
custody and never reach signing. This is unit evidence, not Torii ingress or daemon
qualification. Bind complete mandatory framing and signed RS16/native/topic geometry
to admission before durable receipts, preserving exact retry/recovery authority.
Acceptance still requires rejection before any durable claim, a fitting positive
case, foreign/stale owner refusal, complete mandatory-control accounting, and the
unchanged two-MiB batch/deferred-custody regressions. Torii/Core admission and
consensus-context owners share this open outcome. Reuse the existing
`AuthenticatedAdmissionCapacityV1` from authenticated runner recovery, which the
actual Sumeragi handle already carries into Torii. Reconcile configured carrier
limits and resource/transport reservations with that signed geometry before
promising capacity; enforce outstanding promises across restart. Full goal
closure is unchanged.


### Authenticated input capacity, build46

Startup now requires local body resources to cover the signed RS16 envelope;
active/terminal capacity publication, pending-Kura recovery and candidate selection
share that requirement. The reproduced smaller-local-carrier case is refused before
capacity publication; the exact durable input assembles when resources cover the
unchanged signed layout. Torii's real process handle checks active recovered
capacity, exact network/binding, worst-quorum native envelope and actual publication/
republication topic and encrypted queue bounds before new journal promises. Sizing
holds request memory; loss after quorum remains an indeterminate outcome.

Build46 passes the combined five-crate no-run build and 115 exact runtime controls
on unchanged 7,058 inputs and binaries. Formal46 passes 77 focused controls and the
full source-binding gate on 8,136 unchanged inputs. The [dated record](../docs/history/2026-09-19/admission-capacity.md)
preserves two corrected test-compilation epochs and exact evidence scopes.

The remaining admission outcome is a guaranteed complete GLOBAL opportunity:
bound mandatory metadata across State policy changes, retain mandatory pulse and
penalties, and select/defer optional evidence and autonomous anchors without losing
original custody. A current-round count or the autonomous headroom alone cannot
promise future fit. Native runtime cutover, original publication custody, full
workspace checks and unchanged four/seven-validator fault/restart/final-transaction
qualification remain open. No L1–L6 outcome is closed.


### Exact optional-evidence selection and cold custody, build51

The candidate owner now fits canonical optional-evidence prefixes against exact
proposal bytes/chunks, gives economic/evidence classes alternating height-based
opportunity independent of view, and preserves mandatory effects and original
pending custody. Unfit evidence-only work defers without a pulse-only carrier;
later ordinary work can proceed. Cold startup and active-context construction
restore completed original lifecycle proofs through authenticated finality/context
and unchanged evidence admission filters before productive I/O.

Build51 and all 286 selected runtime controls pass on unchanged 7,058 inputs and
binaries. The full source-binding gate and three final affected controls pass on
8,136 unchanged inputs. The broader 138-control result belongs to frozen source49,
before test-only fixture corrections; the [dated record](../docs/history/2026-09-19/carrier-evidence-custody.md)
preserves exact scopes and failed attempts.

Mandatory-metadata and individual evidence envelopes, pending-pool refill and
startup scan cost remain open. Autonomous anchors cannot move across global
heights by trimming. The next publication step must preserve one prepared entry
from Validate through cache/reproposal and Apply, bound to the service's original
State/Queue pair with complete capture admission and typed local deferral. Cold
unfinished recovery rebuilds once; already-applied recovery does not execute.
The production shared-lane and complete publication cutovers, full workspace tests
and unchanged four/seven-validator campaigns remain required. L1–L6 remain open.

The complete capture input surface now borrows the original StateBlock, execution
prefix, ValidBlock, context, manifest, effects, events and retained source owners.
Visibility of those inputs is not funding for their nested allocations. The
production-adapter candidate charges the named descriptor, World shell, effects
box and phase box layouts only. Events may already be allocated during execution;
the full cold tiered snapshot and geometry/archive projections can allocate later.

The outstanding complete process-memory goal must reserve execution/event
allocations at their actual boundaries, fund later projections before allocation,
and retain their actual charges through publication and delayed EBR reclamation.
Current/undo COW admission belongs before acquisition and first mutation;
detachment and reattachment move original allocations without cloning. Preserve
one ownership chain through capture, decision binding and physical publication;
extra binding/installation admission tokens are not accounting. Existing BodyStore
limits and `MeasuredBytes` are not a complete journal reservation. An allocating
encoded-size fallback, omitted container nodes or saturating arithmetic cannot
establish the required bound. This remains an open resource goal, not a guarantee
of the scoped shell pool or a claim that runtime activation is complete.


### Explicit storage failure versus validation verdict, build54

Production BodyValidationError now requires an explicit deterministic rejection
identity. Local Kura/service errors return before live rejection persistence or
cold marker promotion; planner storage tags and committed DA hydration provenance
survive their Apply conversions. Direct candidate cursor rejection and exact
missing-sidecar deferral retain their existing semantics. The existing storage
fail-stop/restart boundary remains the reachable recovery action.

Build54 and 470 selected runtime controls pass on unchanged 7,059 captured inputs
and binaries. The canonical full gate and 14 focused formal controls pass on
8,137 unchanged inputs. Captures now explicitly include the compiled source-
contract text asset. The [dated record](../docs/history/2026-09-19/local-validation-verdicts.md)
preserves detailed scopes and remaining limits.

Queue-local temporary veto, resource/physical waits and complete capture admission
still require correct shared retirement/drain semantics and actual wake owners;
a local Queue wait can depend on the same blocked height. Original prepared
execution must survive Validate/cache/reproposal into Apply; this verdict correction does not implement
that cutover. L1–L6 and all unchanged-network qualification requirements remain open.


### Physical Validate custody and local capacity, build58

Actual Queue/lifecycle mutex contention now retains the original durable Validate
dispatch, external waiting row, keyed I/O index and output guard. Mutex and worker
capacity releases notify the original service Queue; the existing ordinary-head
drain breaks the identified bounded-channel cycle. No logical wake or new ordinal
is introduced. Fixed Native evidence limits and pending local retirement promises
are typed non-verdict failures; hard canonical format limits remain rejection.
Apply retains its blocking check and final veto until original prepared publication
custody can consume a physical continuation.

Build58, 871 runtime controls, the canonical multilane gate and 49 Python controls
pass on their unchanged input captures. The [dated record](../docs/history/2026-09-19/physical-validation-waits.md)
retains detailed selection and failed attempts. The canonical close intent already
fixes admission closure at C; the next slice
must distinguish retained, WSV-ranked pre-close inputs from fresh admission and
drain them through the existing Native Decision owner. Reconcile those ranked
inputs' Queue custody only from exact Kura terminal receipts before drain voting. An
`f+1` QueuePlan certificate has no guaranteed honest intersection with the
`2f+1` drain quorum, especially across distinct pinned route committees. The
first-release correction therefore makes the certificate internal availability
evidence and gives public `202` only after canonical registry inclusion. An
uncarried or partial attempt makes no public admission promise. Its exact
unreserved QueuePlan claim can be durably terminalized once a committed close
makes future inclusion impossible; canonical pending owners remain blockers.
Never wait for the same blocked height to discharge its own claims or treat
local Queue emptiness as agreed validity. The [September 24 close checkpoint](../docs/history/2026-09-24/queue-close-canonical-admission.md)
implements this local boundary, including restart replay and Queue pop. The
decisive four-validator test must delay pre-close receipt publication through
closure and restart, retain every canonically admitted input, and finish final
work without an empty-block dependency. Also qualify distinct pinned route
committees and a pause between durable admission and attestation.
Full workspace, real unchanged four/seven-validator campaigns and L1–L6 remain open.


### Shared retained-admission authority, build60

One State projection now authenticates canonical pending route custody for Queue
refresh, exact retry, body handoff and cold replay, and the existing Native
closed-lane opening consumer. Closed work retains its original ranked binding,
predecessor, all atomic route members, incarnation and immutable committee.
Fresh ingress and ordinary proposal/reservation execution remain closed. This
removes the identified canonical-close-to-permanent-Queue-fault path without
converting an uncarried off-chain receipt into canonical pending authority.

The combined build, 1,256 selected runtime controls and canonical multilane gate
pass. All 117 selected Python controls pass. Captured inputs and binaries stayed
unchanged within each run. The [dated record](../docs/history/2026-09-19/retained-admission-authority.md)
preserves precise scope, the four repaired fixture-stack failures and the earlier
formal token-normalization failure. No assertion or production stack was weakened.

The following canonical-retry checkpoint connects Torii empty-Queue submission
to the exact State pending custody before fresh routing/capacity. Fresh-policy
independence remains open. Connect exact terminal reconciliation before drain
through the original Queue/State/Kura owner. A Queue release can restore FIFO before Kura writes its
Complete outcome; absent Complete authorization never proves the input had no
old autonomous owner. Preserve that crash cut, exact group/alias evidence and
all reservation/selection barriers. Qualify canonical carry and terminal
disposition for late and partial receipts under the no-precanonical-promise
public response. The actual
shared-lane execution/publication cutover, prepared Validate-to-Apply ownership,
full workspace and unchanged four/seven-validator campaigns remain open. L1–L6
remain open.


### Canonical ingress and recovery qualification, builds64–71

Public canonical QueuePlan retries now return a signed/minimal receipt before
fresh route or Queue-capacity checks. Peer retries verify the exact request and
journal binding, read the original authenticated first carrier under the original
proxy memory reservation and deadline, then rejoin the same State custody. A
missing, corrupt, changed or evicted canonical input cannot fall back to new
admission. All historical read/decoder working sets belong to that reservation;
State locks do not span physical Kura I/O. Build71 still applied current TTL/NTS/crypto/size policy before this lookup;
the build72 checkpoint below replaces that ordering.

Build64 passes with 1,707/1,709 selected runtime controls on unchanged inputs and
binaries. The two failures exposed an obsolete test oracle and production
recovery stack overlap. Actual retained-Validate projection assertions and
heap-owned startup state with separated recovery phases address those causes.
Build71 passes all 2,094 selected runtime tests on unchanged source and binaries:
1,615 Core, 415 data-model and 64 Torii. This includes all 563 recovery controls,
16 critical regressions and seven observed default-stack overflow cases, plus
the 1,531 residual ingress/storage/model controls on the same artifacts.
No failing control is removed and no default stack is increased. The canonical
formal gate passes with 132/134 focused controls on unchanged inputs; two broader
successor-source positives still expose 100 nested owner-binding diagnostics.
The completed Validate, construction and inventory scopes retain their negative
controls. Detailed failed
attempts and exact scopes live in the [canonical retry record](../docs/history/2026-09-19/canonical-admission-retries.md).

Authentication-only canonical retry without creating fresh
AcceptedTransaction authority is implemented and qualified separately by build72:
67 selected Torii runtime controls pass, including expiry/current-policy bypass
only for authenticated canonical custody, forged-signature and absent-input
refusal, and exact sealed-reveal identity. The full canonical multilane structural gate and all 135 canonical-retry,
ledger and capacity controls pass on 8,358 unchanged captured inputs. Broader
successor-source proof qualification remains open.
Reconcile the 17 older Torii fixtures with the first-release QueuePlan contract,
and resolve generic batch admission consistency.
Exact terminal Queue reconciliation, committee-bound receipt carry/closure,
prepared Validate-to-Apply custody and production shared-lane cutover remain open.
Full workspace and unchanged real four/seven-validator campaigns remain required;
L1–L6 are not complete.
