# Sumeragi liveness redesign goals

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

The first concrete target is the runner's independently transitioned
`LifecycleProducerClaimDispositionV1` in
`crates/iroha_core/src/sumeragi/v2_runner/lifecycle_height_driver.rs`.
Replace remembered completion history with a fresh permitted-action projection
of the launched owner's actual state. Audit all nine variants first: some hold
real successor ordinals or wait tokens that cannot simply be dropped. Facts not
already represented must acquire one authoritative owner before removing the
runner copy. A projection is read-only, uncached, and cannot mint work.

Then consolidate separate deferred-owner, timer-episode and schedule decisions
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
   this rule. Do not put a block's own hash into its WSV or witness commitment.
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

This work is part of L1–L5, and `network-checkpoint-05` remains an L6 failure.
Native counterexample/control tests record the current missing transition;
passing those tests must not be described as a liveness fix. Replace the
counterexample expectation with bounded certified progress after integration,
then exercise silent/equivocating authors, partial payload delivery, competing
locks, restart cuts, cross-lane progress and committee reconfiguration.

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
submission already require signed `QueuePlanSynced`, whereas generic Torii
adapters and the public batch handler still admit Ordinary work. M3's strict
admission, autonomous progress, grouped/mixed-role Native settlement, exact-once
application, bounded recovery, and mandatory signed RS16 remain requirements.
Admission's existing `f + 1` durable-storage certificate is distinct from the
exact `2f + 1` Prepare/Commit/Timeout quorum in an exact `3f + 1` committee.

**Retain the input in its admission carrier.** The current
`BlockExecutionContextBundle.queue_plan_admissions` carries only opaque
certificate bytes, and the native handoff retains those certificates while the
canonical marker lacks the body. Replace each control with one canonical typed
source containing the entrypoint, routing plan and exact binding/certificate
evidence. Before registry staging, verify entrypoint and signed identities,
plan/context and the original journal-claim digest against those exact bytes.
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
typed rejection before acceptance. This migration is not implemented yet.

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
| Public batch: `lib_pipeline_handlers.rs::handler_post_transactions_batch` → `push_accepted_transactions_for_ingress_with_routing_plans`; `ensure_generic_transaction_batch_entrypoint_allowed` currently rejects synced intent | Preserve bounded full-batch decode/signature/routing preflight before any mutation, rate-limit accounting and invalid-later-element behavior. Obtain exact durable admission for every entry. Local atomic queue push does not establish distributed atomic acceptance: either preserve all-or-none admission with an authenticated group protocol, or define exact per-entry durable outcomes and idempotent retry in the first-release API. Never report a blanket rejection after durable partial acceptance. This API decision remains open. |
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

## Required invariants

1. **One obligation, one owner.** Every accepted consensus obligation retains
   one owner until completion, authenticated supersession, or a diagnosed
   terminal failure. Transitions conserve ownership and bounded resources.
2. **Every wait has an enabled wake path.** Waiting for response, persistence,
   capacity or validation cannot exclude the event which releases that wait.
   No operation waits for a permit or capacity held by itself or its joining
   caller. External waiting releases execution leases it no longer needs.
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
| L4 — Close resource cycles and permanent retry loops | **Active** for certified-persistence retry audit; L2–L3 | Worker, P2P, validation and application owners | No asynchronous wait or failure/destructor path retains a resource needed by its own completion. Exercise typed retry dependencies at exact capacity. Terminal failures are observable and never silently retried. Ordinary work and authenticated recovery receive bounded fair service. |
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
