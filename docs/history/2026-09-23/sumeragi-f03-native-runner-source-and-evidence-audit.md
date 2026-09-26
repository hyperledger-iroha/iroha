# F03 Native runner source and evidence audit — 2026-09-23

Scope: `/Users/takemiyamakoto/devstuff/iroha` on the existing `optimizations`
branch. This is a read-only production-call-graph review and a qualification
boundary, not a fresh network run, release-candidate seal, or closed F03 gate.

## Current production owner

The current `v2_runner.rs` constructs one `NativeRunnerProcess` before entering
the height lifecycle. Both ordinary and pending-Kura height loops retain that
process and call `native.poll` independently of a new transaction arrival.
`native_process.rs::poll` rechecks committed State/Kura-backed
`VerifiedLaneContexts`, then calls `NativeLaneDriver::poll`. The driver opens a
frozen instance for a committee member, services its shared reducer clock,
persists intent before signing, and retains Native control/output pressure.
The clock can start before a lane payload exists. The same process captures
authenticated Decisions for `native_candidate.rs::assemble_candidate` and
retains actual publication/Apply settlement before final output handoff.

The older `V2LaneWorkAdapter` producer and `schedule_autonomous_new_view_timeouts`
remain in source and tests, but the current runner does not call either method.
The old NewView clock is keyed by an already published autonomous payload and
cannot replace an unavailable initial author before that payload. The current
ordinary ingress consumer retires old `LaneBlockNewView*`, lane-vote, and
lane-certificate envelopes; Native controls enter the separate authenticated
process ingress. Thus the earlier target-only September 23 cutover review that
says production still constructs the old autonomous signer and rejects Native
ingress describes an older source capture, not this combined checkout. The
retained old signing implementation and wire surface still need first-release
deletion review; they are not a supported compatibility path.

## Retired-path deletion graph

There is no safe isolated Sumeragi-only deletion in the old output corridor.
`v2_runner.rs::dispatch_lane_work_effects` has only test callers, but it shares
`dispatch_lane_work_effect` with the **production**
`canonical_recovery_ingress.rs::dispatch_canonical_executed_block_recovery_effects`.
Preativation recovery calls the latter from `lifecycle_run_inner.rs`; it emits
`V2LaneWorkEffect::PostLaneBlock` carrying an exact
`LaneHistoricalRecoveryRequest`. `ProductionV2Services::post_lane_block`
transfers that message through the live exact-output owner. Removing the
generic dispatch arm or sender would also remove required interrupted-tip body
recovery, while gating the arm only in tests would hide rather than retire the
old authority.

`BlockMessage` still declares old proposal, executable-payload, NewView,
vote/QC/certificate and historical-recovery tags. Its outbound check permits
those lane-local tags, although current ordinary ingress retires the old
consensus envelopes. NewView vote/certificate references span 26 crate files,
including non-test Core lifecycle/output match arms. The separate
`LaneRelayMessage` variants and Sumeragi handle wrappers also reach daemon
P2P translation; the current handle rejects every variant except QueuePlan
admission, but deleting their API or wire shape is not confined to Sumeragi.

The prerequisite is to give live canonical body recovery a dedicated, bounded
current-source request/response effect and exact transport owner, preserving
its authenticated return route, retained request and restart behavior. Then
remove the generic old lane output arm, old adapter signer/NewView state and
old wire tags together with daemon P2P mappings, source-contract fixtures and
tests, and regenerate the one V1 Norito identity. No alternate decoder,
parallel signer or unsigned availability path is acceptable. This review did
not make that cut or change production code.

## Evidence and precise remaining gate

The retained
`autonomous_initial_author_loss_counterexample_retains_work_without_preproposal_progress`
test exercises three isolated survivors of the old adapter with real QueuePlan
admission: no old reservation, payload or NewView clock appears. Its live-author
control takes the original Queue reservation. It is historical negative
evidence about that adapter, not a current Native runner liveness failure.

`native_driver_silent_initial_author_reaches_real_decision_without_global_view_input`
uses committed State contexts and three survivor `NativeLaneDriver` instances.
Their actual signed timeout controls advance to a successor proposal and an
authenticated three-share Commit Decision while the original Apply remains
held. It is an in-process component test, not a network actor, runner ingress,
canonical global application, or restart qualification.

The Sumeragi liveness ledger records matching daemon67/harness68 passing an
earlier four-validator silent-author, sole-input and genesis-restart diagnostic
in 115.45 seconds. Later seven-validator two-restart validation failed after
Native execution at historical replay; matching daemon73/harness74 then failed
earlier at whole-process ingress rollover. The separate closed-global ingress
cut and Native historical replay repair were under validation. Those prior
receipts are evidence for their captured source only; they cannot qualify the
current combined checkout or close L1–L6.

After the combined Core test binary rebuilt successfully on this checkout,
`native_driver_silent_initial_author_reaches_real_decision_without_global_view_input`
passed **1/1** on that fresh binary. This confirms the current source still
reaches the in-process signed Decision with an absent initial author. It does
not exercise the process network, global carrier, economic Apply or restart.

The bounded next cutover gate is a fresh, matching-source four- and
seven-validator campaign across the **process-lived** Native ingress and
global-height rollover: one admitted input, silent initial author, durable
three-of-four timeout certificate, replacement-author signed RS16 proposal,
Native Decision, one canonical global economic Apply and Queue terminal
settlement. Repeat with stopped-author restart and a second restart after
Native execution, then lost/reordered traffic, full ingress/output pressure,
stale incarnation and cross-route group advancement. Recheck exact WAL,
State/Kura/Queue, finality and application evidence; an old adapter test or a
driver-only Decision cannot substitute for this runner-to-Apply proof. Retire
the old signing/wire implementation in the final single V1 cut after its
remaining non-signing consumers are accounted for. F03 and all six multilane
milestones remain open.

## 24 September current-source canonical-body transport seam

This follow-up narrows the live non-signing dependency behind the old generic
lane output arm. `CanonicalExecutedBlockRecovery` in
`v2_lane_work/canonical_executed_block_application_repair.rs` owns a bounded
queue of missing finality-bound bodies, one outstanding request and responder,
retries, assembled chunks, retired request hashes and outbound effects. It
currently emits `V2LaneWorkEffect::PostLaneBlock` for both certificate-free
`CanonicalExecutedBlock` requests and `CanonicalExecutedBlockChunk` responses.
The preactivation and pending-Kura loops in `v2_runner/lifecycle_run_inner.rs`
and `v2_runner/lifecycle_pending_kura.rs` call the same live recovery dispatcher.
`v2_runner/canonical_recovery_ingress.rs` now restricts that dispatcher to the
two exact variants, but still hands them to the generic
`dispatch_lane_work_effect` arm in `v2_runner.rs`.

The transport dependency is wider than that match arm. The preflight in
`v2_worker_services_impl.rs::can_retain_lane_work_effect_from_snapshot` and
the guarded sender there construct matching historical-request/response
`ExactOutputRolloverClaim`s before a bounded exact-output fanout. The pending
output owner cancels superseded request hashes and treats an active request as
reconstructible from the recovery source; response reconstruction checks the
exact Kura body and finality. The applied-height handoff validates those
claims. The recovery ingress checks the original fair-queue ownership,
authenticated sender and any ingress reply-route evidence before it accepts a
request or chunk. Its current response effect then addresses that authenticated
sender through the topology route; it does not retain the ingress reply routes
as a separate outbound route owner. A replacement must preserve the actual
topology-targeted delivery, exact request cancellation, source-held retry and
Kura-backed response/handoff semantics across rollover and restart.

There is also a current-source serving gap. The responder builder is called
by the old adapter and by `CanonicalExecutedBlockRecovery` when that recovery
owner receives a request. The live runner constructs the latter only when its
own preactivation or pending-Kura plan has nonempty missing-body needs.
`v2_runner/ordinary_ingress_consumer.rs` instead retires historical lane
requests on a peer with no such recovery owner, and
`v2_runner/decided_lane_recovery.rs` retires that lane-local family during
terminal drain. Thus source inspection finds no live request-serving path for
an otherwise healthy archive peer after ordinary activation. A requester
retry cannot obtain a chunk from that peer until a dedicated responder is
connected. This is a call-graph finding, not a network failure measurement.

The bounded responder can use the chain-scoped `V2BlockSyncServer` service
boundary already passed through ordinary and decided terminal ingress. Add a
purpose-specific canonical-body serve task, using the existing pure Kura/finality
validator and chunk builder, independent of the server's local missing-body
needs. The active ingress owner must validate its exact fair-queue carrier,
authenticated sender and route evidence before moving an admitted request
into a bounded task; unadmitted requests remain retryable by the requester.
The task retains its original request hash and target through worker preparation
and exact-output acceptance. If output capacity refuses the response, the
prepared response remains with that server owner for local retry; a response
is not dropped after accepted admission. The preactivation recovery aperture
can use the same responder logic under its exclusive fair-ingress borrow,
without adding a competing queue consumer. A healthy active or terminal peer
must be able to answer from its verified Kura history without constructing a
requester `CanonicalExecutedBlockRecovery`.

The smallest production replacement is a dedicated bounded canonical-body
request/response V1 wire pair and typed recovery effect, carrying only the
requester, `CanonicalExecutedBlockNeedV1`, chunk index, request hash, exact
finality and bounded chunk bytes already used by this recovery owner. It needs
its own exact-output reservation, send, cancellation and rollover claims in
`v2_worker_services_impl.rs`, `v2_worker_exact_output.rs`,
`v2_worker/exact_output_rollover_claim.rs`,
`v2_worker/effect_services_impl.rs` and
`v2_worker/autonomous_lane_output_reconstruction.rs`. The same atomic cut
must update `sumeragi/message.rs` wire tags, `sumeragi/mod.rs` fair-ingress
kind/class, byte bounds and ownership matching, and `lib.rs` predecode/topic
maps. Daemon `main.rs` must classify the new pair for ingress rate, relay lane
and telemetry metadata. The ordinary and decided terminal ingress consumers
must continue to retire old consensus traffic while routing new canonical-body
requests to the dedicated responder; only an outstanding requester may consume
a matching new chunk response. Keep one V1 Norito layout: no fallback decoder
or second lane-signing path.

Only after that replacement is source-connected and tested can the generic
`PostLaneBlock` dispatch arm and `ProductionV2Services::post_lane_block` be
deleted with the old adapter signer, historical lane wire tags, daemon maps
and source-contract fixtures. Meaningful tests must cover source retention at
exact-output capacity, byte-identical retry and retired-hash cancellation,
wrong peer/request/finality/chunk rejection, Kura-backed response rollover,
healthy-peer service without local needs, interrupted-tip and pending-Kura
restart reconstruction, and current-height message delivery under the
fair-ingress byte/class limits. The existing canonical-body drift,
dispatch-allowlist and applied-height handoff tests are
starting points, not proof of this replacement. This follow-up changed no
Core or daemon source, ran no tests, and supplies no F03 qualification.

## 24 September bounded canonical source worker cut

The current checkout now has a source-only preparation cut in
`v2_canonical_executed_body_serve.rs`, joined to the existing chain-scoped
`V2BlockSyncServer` historical-body worker. A canonical-only task validates
the exact fair-ingress occurrence, authenticated requester and reply route,
then shares the worker's fixed queue, principal and global time budget. The
worker checks committed State, Kura body and finality through the existing
canonical builder, preencodes one bounded chunk response and mints a private
proof binding the request hash, source height, responder and warmed exact
output hash. It retains the original request and route owner with the
completion. Canonical admission returns that same owned task on Busy,
RateLimited, missing-worker and disconnected/error paths, so a future ingress
owner can retry or fail stop without treating local capacity as an invalid
consensus request. The already-live certified-body worker path keeps its
existing API and semantics.

This is **not** a production responder yet. Ordinary and terminal ingress
still retire the old lane request family, and no canonical completion is
posted to exact output. The required typed V1 wire, rollover claim,
`SourceRetained` deferral, healthy-peer network path and restart tests remain
open. No old signer, generic lane send or compatibility decoder was enabled.
The focused `canonical_executed_body_worker_` tests exercise a real committed
State/Kura source without local recovery needs, wrong requester/finality,
response-frame rejection, substituted output, and an admission refusal that
returns the unchanged owner for retry. The combined current-source Core rebuild
passed both tests **2/2**. Four cached existing recovery fixture consumers also
passed individually **1/1** each: drift/rotation/cache, remote-only carrier,
corrupt-chunk restart, and old-output ordering. The shared fixture now opens
its signed ordinary carrier with the pre-funded pristine-carrier State
constructor; its former header-only start omitted the ordinary membership
source and failed at `MissingInsertBlock`. These are focused component tests,
not healthy-peer network or rollover qualification.

F02 resource admission also remains open at this boundary. The task currently
clones the bounded request to compare the exact fair-ingress message before
the worker reservation, and the underlying canonical response builder
re-encodes the full body twice. The worker's fixed task and response-frame
time charge does not yet reserve the physical full-wire allocations, nested
finality/message capacities, Kura read I/O or CPU work from their original
owner. These costs must be charged before a production ingress handoff;
passing the source tests cannot qualify that resource gate or F03 liveness.

## 24 September typed output and rollover dependency audit

The next coherent V1 cut replaces the old historical-lane request/response
wire tags with a canonical-executed-body-only pair, then migrates the
requester's outstanding hash, response matching, effect dispatch and request
cancellation to those exact types. `sumeragi/message.rs`, `sumeragi/mod.rs`,
`lib.rs`, daemon P2P maps, `canonical_executed_block_application_repair.rs`
and `v2_runner/canonical_recovery_ingress.rs` must change together. Ordinary
and decided-terminal ingress must pass only the typed request's original
authenticated fair occurrence and reply routes to the chain-scoped
`V2BlockSyncServer`; the preactivation and pending-Kura recovery apertures
must use that same server, currently constructed after their recovery loops.
A worker-capacity refusal after dequeue needs a bounded server retry owner
for the unchanged task. The old lane signer and generic post cannot be
reactivated as a second route.

The exact-output handoff parallels the certified-body service, but needs a
distinct canonical claim and completion settlement in `v2_runner.rs`,
`v2_worker_services_impl.rs`, `v2_historical_body_serve.rs`,
`v2_worker/exact_output_rollover_claim.rs`, and
`v2_worker_exact_output.rs`. It must consume the worker's already encoded
message and source proof with the original fair-ingress/reply-route owner.
If exact output returns `SourceRetained`, the server must retain and retry
that same prepared frame; its pending count must continue to fence height
completion. Superseded requests must cancel by the new typed request hash,
without cancelling a current chunk or another requester's route.

The existing historical-lane response rollover check reads and re-encodes
the entire Kura block on the actor. Its replacement cannot weaken source
reconstructibility: `Kura::durable_block_payload_len_by_hash` deliberately
continues to succeed after body eviction, so finality and length metadata
alone do not prove that this responder can recreate the chunk. A bounded
Kura-backed body-availability or pin owner, or an off-actor durable source
lease, must support a cheap rollover proof; otherwise the accepted exact
response must remain owned until writer flush. The related F02 physical
charges include request/context/route copies, actual Kura I/O and work,
full-wire encoding, and any deferred frame or body lease.

Required focused tests cover healthy-peer service with no local needs,
wrong sender/request/proof/source, full exact-output capacity with unchanged
`SourceRetained` retry, route retirement, typed-hash cancellation, pending
completion across rollover, body eviction/corruption, and interrupted-tip
restart. A matching four-validator requester/archive run is still required.
This was a read-only dependency audit: it added no production route or test
and did not close F03 or F02.

## Authenticated original-inbound admission helper

The disconnected canonical task now has one constructor from the original
`InboundBlockMessage`. It borrows the request, authenticated sender/via and
reply routes, verifies the exact fair-ingress occurrence and canonical-only
kind, then moves that same carrier's request, route and owner into the worker
task. Shape or ownership rejection returns the unchanged inbound; missing
worker, Busy and RateLimited admission return the task owner. The unused
loose-field canonical constructor was removed. Historical-body live callers
are unchanged.

The combined Core build passed `canonical_executed_body_worker_` **2/2** on
this source, including wrong sender and retired-kind rejection, absent-worker
return, and pressure retention/retry. This is a typed source boundary, not
live ordinary or terminal routing. The request clone used for exact ingress
comparison, the bounded actor retry slot, exact output and rollover still
need physical charge and production connection before this gate can open.
