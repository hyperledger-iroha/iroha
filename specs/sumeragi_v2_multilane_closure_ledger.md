---
title: Sumeragi V2 Multilane Closure Ledger
sidebar_label: Multilane Closure Ledger
description: Source-bound implementation, test, formal, and release obligations for production multilane completion.
---

# Sumeragi V2 multilane closure ledger

This ledger is the release-closure record for Sumeragi V2 multilane execution.
It complements the protocol description in [Sumeragi V2](sumeragi_v2.md), the
activation contract in [Nexus cross-lane execution](nexus_cross_lane.md), and
the persistence contract in [Merge ledger](merge_ledger.md). It does not replace
those documents.

The ledger deliberately distinguishes a reachable production implementation
from a release-evidenced end-to-end path. The current source contains
autonomous production, Native application evidence, evidence-aware drain, and
artifact-aware retirement. The Rust-generated protocol-4 consensus fixtures
are synchronized, while authenticated Kura replica outbound/configuration and
its formal/release closure remain open below. Source presence, synchronized
fixtures, and partial direct SDK results do not prove the required real-network
corridors, fault soak, scaling target, complete SDK parity, or clean full-
workspace release gate.

## Status rules

Every in-scope row records three independent states:

- **Implementation: Open** means a required production path is absent,
  test-only, disabled, internally inconsistent, or still being changed.
- **Implementation: Implemented** means the cited production symbols implement
  the row's primitive. It does not imply that the wider milestone is complete.
- **Closure: Open** means at least one implementation, integration, or
  compatibility obligation in the row remains.
- **Closure: Implemented** may be used only when the complete production path
  described by the row is reachable without a test, feature, or environment
  switch.
- **Evidence: Open** means the required fresh test, formal, or release artifact
  is absent, incomplete, skipped, failing, stale, or not source-bound.
- **Evidence: Evidenced** may be used only after the named gate has passed from
  a clean source revision and its logs, hashes, configuration, seed, and
  hardware identity have been archived.

No user-visible release gate in this ledger is currently **Evidenced**.
Existing unit tests and historical status reports are useful leads, not
multilane release evidence.

## 2026-07-31 closure-marker audit

The audit used case-insensitive whole-word `TODO|FIXME|XXX` scans. Raw token
matches were then inspected so sentinel strings, ISO currency code `XXX`, test
assertions, scanner commands, and historical prose were not misclassified as
unfinished implementation.

- `crates/` plus `integration_tests/` contain 55 raw matches and 11 actual
  source TODO comments. None is an in-scope multilane TODO. Eight are
  consensus- or lane-adjacent but explicitly classified below, and three are
  unrelated privacy/Kagemusha/PTX work.
- `formal/sumeragi_v2/` contains 13 TODO markers. One is the generic Decision
  fetch refinement already classified below; the other 12 are generic locked-
  body/producer/corridor liveness-composition obligations. None models a lane
  reservation, Native participant application, merge carrier, autoscale drain,
  or lane retirement.
- `roadmap.md` contains five raw matches: two active wallet-query TODOs and
  three narrative/scanner mentions. The two active items concern an account
  activity feed and an outgoing-value aggregate, not multilane consensus.
- `status.md` contains 274 raw matches, all in chronological reports, quoted
  commands, scanner descriptions, or sentinel discussions; it has no active
  TODO directive. `docs/` contains no marker.

Thus the current explicit in-scope marker count is zero, but release evidence
remains incomplete. The authenticated replica-advert outbound/configuration
path in `ML-KURA-01` and its `ML-WIRE-01` runtime-bound dependency are now
source-bound; their focused Rust, formal-engine, and network evidence remains
open. The former generated-
fixture mismatch in `ML-API-04` is resolved byte-for-byte, but its full SDK
evidence remains open. Static source inventories and partial direct SDK runs
do not close `G-UNIT`, `G-SDK`, `G-FORMAL`, or any execution/network gate. The
statements in `roadmap.md` and historical `status.md` that no marker remains
are only marker inventory claims; they are not implementation or release
closure evidence.

## Non-negotiable invariants

The implementation and evidence rows below must preserve all of these
invariants together.

1. **One state machine.** Lane and Native AMX participant controls certify
   routing and settlement only. Economic effects mutate WSV exactly once,
   through a merge batch carried by one canonical global block.
2. **One participant predicate.** A single production predicate decides whether
   a Native AMX leg requires separate participant application. A coordinator
   leg on the same route is never materialized as a participant marker,
   receipt, diagnostic row, drain blocker, or retirement artifact.
3. **Exact incarnation.** Every proposal, QC, reservation, journal claim,
   marker, sidecar, index entry, merge source, and diagnostic identity binds the
   active `(lane, dataspace, incarnation)`. Reusing a lane ID never reuses its
   incarnation.
4. **Contiguous history.** A lane-local proposal binds the exact current
   predecessor height and hash and advances by one. Stale, skipped, ambiguous,
   or conflicting histories fail closed.
5. **One reservation owner.** A queued transaction is either ordinarily queued
   or owned by one durable lane reservation. Its exact reservation bytes do not
   change while moving through payload, lane certification, merge
   certification, global application, and final queue retirement.
6. **Durability before visibility.** Canonical block/finality and the Native
   application manifest are durable before exact sidecars and indexes; exact
   sidecars and indexes are durable before replicated WSV frontiers become
   visible.
7. **Evidence before retirement.** A drain frontier derived from Native or
   autonomous work is usable only while its exact durable application evidence
   revalidates. Missing, ambiguous, pruned-without-proof, or malformed evidence
   blocks retirement.
8. **Bounded authenticated recovery.** Missing material is fetched only within
   configured byte/count budgets and only from authenticated committee or QC
   sources. Local cache contents and network arrival order never choose
   canonical state.
9. **Coordinated first-release wire.** Every new Norito persistence and wire
   layout has an explicit version. Consensus never implicitly decodes a legacy
   shape or runs mixed old/new layouts.

## In-flight first-release formal boundary

**Implementation:** Current first-release layouts are source-bound, as is the
reservation-journal primitive seam. The fixed-width composed transition relation is implemented
and source-bound. The autonomous FIFO selector consumes checked
`SelectQueuePlanV4Conjunction` and `FsyncReservationV5` projections derived
from the canonical slot authority and complete ordered reservation-group
identity. Queue then revalidates the live V4/V5 group and retains an exact
per-transaction transition fence while Kura recomputes the slot hashes from
the frozen height context, checks `ActivateKura`, and persists the local
producer payload. The first autonomous execution-input append revalidates a
repair-disabled exact payload/input pair, consumes a move-only authorization,
and checks `PersistExecutionInput` before its indexed data/index sink; exact
replay is a storage stutter. The one-shot READY signer rederives that shared identity and exact
producer/signer committee bits, then consumes a checked `SignReady` projection
immediately before signature construction. The first READY-QC Kura write now
consumes an exact payload/certificate authorization, checks `PersistReadyQc`,
and reaches the durable view-state sink only afterward; exact replay is a
stutter. The first autonomous certified-session write consumes an exact
source-bound authorization and checked `LaneCommit` projection before its
durable latest-frontier sink; exact replay is also a stutter. The first Kura
slot-retirement write now consumes an exact payload/group/retirement/path
authorization and checked `PersistKuraRetirement` projection before the
validated atomic view-state sink. Its claim-release continuation validates the
whole on-disk group before mutation, accepts only canonical crash prefixes, and
consumes a path-and-replacement-bound `AdvanceReleasePendingPrefix` or
`AdvanceReleasedPrefix` authorization immediately before each synced atomic
replacement. Invalid mixed-stage groups fail before any claim or temporary-file
write, while exact restart prefixes and fully released retries stutter. A separate pre-Kura
reservation-batch direct-release path uses the same complete-group predicate and consumes
its checked `DirectReleased` projection under the Queue transition and FIFO
locks immediately before the durable release append. Existing bounded
consumers also recheck durable execution input/READY QC/lane Commit at merge
source admission. Canonical `ApplyCarrier` projections remain in a move-only
batch that V2 boxes as a `StateBlockCommitAuthorization`; State consumes the
exact block/merge/cardinality-bound batch under `state_commit_lock` and the
lifecycle fence when present, after the final scale-in Queue veto and before
geometry or transaction publication. V2 takes the Queue observer only for an
exact pending scale-in. The three ordered post-carrier Queue cleanup prefixes
remain separately checked. The complete production trace extraction is not implemented.
The source-certificate contract now includes the
already-implemented pre-Kura direct release linearization point. Six named
actions remain deliberately open rather
than inferred: `FanoutFromProducer`, `ServeLateBody`, `Crash`, `Recover`,
`RecoverReservationSnapshot`, and `RepairPostCarrierEvidence`. In particular,
the abstract snapshot stutter is not yet a proof that concrete startup replay
preserves the composed abstraction.
**Closure:** Open.
**Evidence:** Current schema-5 structural/source binding, local checked
reservation-journal transition evidence, and Rust/Verus composed relation.
The dated bounded TLC/Apalache checkpoint below does not attest the current
schema-5 source; the production trace-extraction theorem remains open.

`SumeragiV2InFlightFirstRelease.tla` is a finite three-validator safety model
for the accepted schema V2 carried by the production
`LaneExecutablePayloadV1` container and its exact
`QueuePlanAdmissionBindingV2` preimage. It establishes authenticated custody
for the selected producer without inferring knowledge by every validator, and
uses the canonical strict 3-of-3 count quorum for its fixed cardinality. Its
fixed and mutation configurations cover producer-selected versus
replicated-carrier ownership; a selected-batch
conjunction over individual QueuePlan journal V4 `Put` records; reservation journal V5,
Kura Active, execution-input, READY authorization/signature/QC,
and lane-commit ordering; volatile body loss; crash-prefix durable recovery;
atomic WSV application; separate post-carrier reservation
per-key Commit/QueuePlan-tombstone/ForgetCommit prefixes; prefix-recoverable four-stage
release; exact lane-commit/release scope; conflicting/ABA bindings; and the
4096 entry limit. Auxiliary post-WSV repair is explicit stuttering.

The local production binding covers
`PreparedReservationJournalTransition`,
`prepare_checked_transition`, `apply_checked_transition`, runtime
`transition_commit`, durable append, snapshot compaction, and the primitive
refinement checker. It binds the exact frame/bound, state-instance domain,
structural pre-state shape, generation/history identities, and ordered
owner-token coverage through post-I/O revalidation and bounded direct
publication without a full-state clone, including fail-closed post-durable
restart reconstruction. The composed Rust/Verus relation generalizes the
committee bitmap to 1 through 128 validators while the TLA+ instance remains
explicitly bounded. It mirrors QueuePlan, Kura/input/READY-QC, volatile
fanout/late-body custody, lane commit, canonical WSV application, Commit
cleanup, and ordered release. Its reverse
terminal-owner projection distinguishes canonical-WSV Commit ownership from
ordinary-FIFO ordered/direct release. Snapshot recovery maps to an abstract
stutter, and direct release is an explicit named action. The retired lane-wide
removal operation is absent from the schema-bound V5 journal, and its old
bootstrap claim and operation bytes fail closed without compatibility replay.
The deterministic autonomous selection linearization point now derives its
move-only authority from the canonical slot committee/author, revalidates the
exact QueuePlan registry and FIFO selection, derives the complete ordered
reservation-group identity, and consumes checked QueuePlan-selection and V5
fsync projections at the journal append. The READY signature boundary consumes
its Kura-minted move-only authority only after checking the exact proposal,
availability body, height context, committee geometry, and reservation group.
The execution-input persistence boundary separately reconstructs the exact
producer-authenticated payload and complete input, binds its canonical
reservation group and authenticated writer witness, and consumes a checked
`PersistExecutionInput` projection before the indexed Kura append.
The READY-QC persistence boundary separately binds the exact certificate,
payload, chain/epoch, producer, committee bitmap, and shared reservation group
into a move-only authority, consumes it, and checks `PersistReadyQc` before the
Kura view-state write. The autonomous certified-session boundary reconstructs
the exact repair-disabled merge source, binds the immutable payload, durable
input, READY/Commit signer intersection, canonical source bytes, and reservation
group into a move-only authority, and consumes a checked `LaneCommit`
projection before publishing the durable latest frontier. Exact replay is a
storage stutter. The slot-retirement boundary independently binds the immutable
payload and ordered reservation group, active committee/writer witness, exact
retirement, and view-state path into a move-only authorization, then checks
`PersistKuraRetirement` immediately before the durable view-state writer. The
claim continuation preflights every main/temporary claim and the full canonical
stage ordering before creating any replacement plan. Each planned replacement
is pre-encoded and receives a one-shot exact-path/exact-bytes transition
authorization; the authorization is consumed and its projection rechecked at
the atomic synced replacement. The source ledger also binds the underlying
temporary-file write, file sync, rename, persisted-file sync, symlink check, and
directory sync order. These are bounded production seams, not the still-missing
complete Rust trace-extraction theorem.
The durable merge-source, canonical WSV commit, and post-carrier Queue cleanup
consumers check their named projections against that same group identity. The
pre-Kura reservation-batch direct-release linearization point separately
consumes its composed projection after locked V4/V5, FIFO, group, and committee
revalidation. Other production linearization points do not yet extract and
consume the relation, so the end-to-end refinement remains open.

This row is deliberately **not** a production-refinement claim. When run, TLC
can exhaust the stated finite model and Apalache can typecheck/bound its
abstract actions; neither checker proves that Rust filesystem/restart traces
refine those actions. Current schema-5 static checks execute neither engine.
The open theorem is a production trace extraction from the actual QueuePlan
journal V4, reservation journal V5, Kura, recovery, Commit, WSV, and Release
linearization points into the implemented pre/post-state and terminal-owner
relations. Schema 5 of `multilane_source_bindings.json` deliberately classifies
this as `composed_state_action_relation_no_trace_extraction`: its exact
version/field/order/relation bindings detect drift but are insufficient to
promote this row or a release status. See
`formal/sumeragi_v2/INFLIGHT_FIRST_RELEASE_EVIDENCE.md`.

## Native AMX application closure

### ML-NAT-01 — shared participant-application predicate

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.**
`native_amx_participant_application_role` and
`native_amx_receipt_requires_separate_participant_application_for` in
`crates/iroha_core/src/native_amx.rs` are the shared typed classifiers.
Validation, Kura persistence/recovery, State diagnostics and drain derivation,
and retirement consume that classification. Exact same-route coordinator
identity yields no separate receipt, marker, diagnostic row, frontier, or
retirement blocker; same-route identity drift fails closed.

**Closure condition.** Add one typed predicate whose inputs include coordinator
route/incarnation/proposal identity and participant route/incarnation/proposal
identity. Use it at every production consumer. The false branch must create no
separate participant receipt, marker, latest-pointer update, diagnostic record,
frontier, or drain blocker.

**Focused and adversarial tests.** Cover exact same-route coordinator legs,
same lane with a different dataspace, same route with a stale incarnation,
different proposal identity, mixed coordinator/participant roles in one block,
and conflicting same-height identities. Assert identical group membership in
block validation, Kura receipt derivation, State marker derivation, recovery,
diagnostics, drain, and retirement.

**Formal obligation and mutation.** Invariant `MLSeparateParticipantApplication`
states that all consumers derive the same participant set. Mutation
`ML-MUT-NAT-01` changes one consumer to route-only or always-true membership;
TLC and Apalache must expose an extra marker/receipt or a missing drain blocker.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-SDK`.

### ML-NAT-02 — durable signing claim and restart equivocation safety

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** `NativeAmxSigningGuard`,
`NativeAmxSigningGuard::load_validated_journal`, and
`NativeAmxSigningGuard::record_locked` in
`crates/iroha_core/src/native_amx.rs` own the version-4 durable decision.
`NativeAmxSourceSessionClaimV4` binds source ID, typed entrypoint hash, plan,
round/context, authority height, coordinator route/incarnation/height/view and
proposal, while `NativeAmxSourceParticipantClaimV4` binds every participant
route/incarnation. Grouped signing slot claims remain separately keyed by
`NativeAmxSigningSlotV3`; unsupported or malformed journal layouts fail
closed.

**Closure condition.** A versioned durable source claim must bind the source
ID, typed transaction-entrypoint hash, plan digest, round context, authority
height, coordinator route/incarnation/planned height/view/proposal, and every
participant route/incarnation. Grouped slot claims remain a separate
anti-equivocation dimension. Unsupported journal versions fail closed unless
an authenticated canonical source rebuilds the exact stronger claim; no
best-effort legacy projection is allowed.

**Focused and adversarial tests.** Cover source, entrypoint, plan, epoch,
context, authority-height, coordinator route/incarnation/height/view/proposal,
and participant-set drift before and after restart. Cover a crash before record
publication, after record fsync, before anchor publication, after anchor
publication, truncated and oversized files, duplicate sequences, unexpected
files, symlinks, and a stale legacy journal.

**Formal obligation and mutation.** Invariant `MLNativeSourceClaimInjective`
states that one source claim cannot authorize two distinct bound sessions.
Mutation `ML-MUT-NAT-02` removes each claim field in turn; every projection
weakening must produce a restart equivocation trace.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, and `G-FINAL`.

### ML-NAT-03 — active incarnation, predecessor, and contiguous admission

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** Native AMX signing/body/QC checks in
`crates/iroha_core/src/native_amx.rs` bind the active authority context.
`ValidBlock::validate_execution_context_with_state` and
`ValidBlock::validate_native_amx_participant_groups` in
`crates/iroha_core/src/block.rs` resolve exact proposal-height incarnations and
canonical predecessor application receipts through State/Kura before
admission. Height jumps, stale incarnations, and predecessor identity drift
fail closed.

**Closure condition.** Immediately before Prepare signing, Commit signing, and
block admission, resolve the exact active incarnation and require the exact
current predecessor height/hash plus the contiguous next height. The check must
use authenticated State/Kura evidence and must not trust a proposer-local
journal or cache.

**Focused and adversarial tests.** Reject a height jump, wrong predecessor
height, wrong predecessor hash, zero/noncanonical predecessor, stale or future
incarnation, A/B/A lane-ID reuse, authority-height drift, a delayed old QC, and
a correct body whose active catalog changes before admission. Repeat the
signing cases across restart.

**Formal obligation and mutation.** Invariant `MLNativeContiguousActiveRoute`
allows signing/application only at the active incarnation's unique successor.
Mutation `ML-MUT-NAT-03` replaces exact successor with `>=`, removes the
predecessor hash, or resolves by lane ID alone; each mutation must admit a
height jump or ABA trace.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, and `G-12P`.

### ML-NAT-04 — grouped application is atomic, exact, and bounded

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** Bounds originate in
`MAX_NATIVE_AMX_PARTICIPANT_CONTROL_SOURCES`,
`MAX_NATIVE_AMX_PLAN_LEGS`, and `MAX_NATIVE_AMX_VALIDATORS` in
`crates/iroha_core/src/native_amx.rs`.
`ValidBlock::validate_native_amx_participant_groups` validates block-wide
grouping.
`Kura::native_amx_participant_application_evidence_for_block_under_publication_guard`
joins ordered source/entrypoint/result membership to the participant proposal
and settlement. `State::native_amx_participant_frontier_marker_payloads`
publishes the replicated frontier.

**Closure condition.** Require 1–4,096 ordered, unique sources; exact
transaction count and timestamp; the current source exactly once; zero
participant effects; no nested fee or Native receipts; and valid mixed-role
block-wide anchoring. Prepare and Commit must carry identical participant
payloads. Group persistence and frontier publication are all-or-nothing.

**Focused and adversarial tests.** Cover zero, one, 4,096, and 4,097 sources;
partial, duplicate, reordered, or foreign sources; wrong timestamp or
transaction count; missing current source; duplicate entrypoint; nonzero
effects; nested receipts; mismatched Prepare/Commit; conflicting settlement or
proposal; and valid/invalid mixed-role and same-route blocks.

**Formal obligation and mutation.** Invariant `MLNativeGroupExactCover` states
that the ordered group is a bijection over the committed entrypoint/result
members and applies atomically. Mutation `ML-MUT-NAT-04` drops uniqueness,
order, lower/upper bounds, or exact-cover checks; the model must expose partial
or duplicate application.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-SDK`.

### ML-NAT-05 — canonical application manifest, root, and proofs

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** `NativeAmxApplicationManifestMemberV1`,
`NativeAmxApplicationManifestLeafV1`, and `ExecutionCommitment` in
`crates/iroha_data_model/src/block/consensus_v2.rs` define the versioned
canonical manifest, empty root, leaf validation, root, and leaf count. Kura
persists the leaf/proof artifact and exact participant receipt as separate,
immutable, versioned files named by participant height. Each file binds route,
incarnation, predecessor, descriptor, proposal, settlement, ordered
source/result membership, global application identity, and executed wire;
same-height publication is no-clobber and accepts only byte-identical replay.

**Closure condition.** Define a versioned canonical Native application
manifest. Its Merkle leaves and proofs bind route, incarnation, predecessor,
participant proposal, settlement, ordered source/result membership, and
application block height/hash. Commit its root in every globally finalized
execution commitment, using a canonical empty root when no Native application
exists. The manifest must be independently reconstructible from the canonical
executed wire.

**Focused and adversarial tests.** Round-trip the empty, singleton, grouped,
mixed-role, and multi-route manifests. Reject forged roots, omitted or reordered
leaves, wrong predecessors, route/incarnation drift, source/result substitution,
proof path/position drift, application-block substitution, malformed proofs,
and a commitment that advertises a root inconsistent with the canonical wire.

**Formal obligation and mutation.** Invariant `MLNativeManifestAuthenticates`
states that every durable participant frontier has one manifest leaf
authenticated by the carrier QC. Mutation `ML-MUT-NAT-05` removes each leaf
field or accepts an unauthenticated receipt; the model must admit a forged or
ambiguous frontier.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, `G-SDK`, and
`G-FINAL`.

### ML-NAT-06 — publication order, pruning safety, and startup repair

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** `Kura::persist_native_amx_participant_application_evidence`
and `Kura::repair_native_amx_participant_application_evidence` in
`crates/iroha_core/src/kura.rs` run under the publication guard and validate
the canonical block, checkpoint, V2 finality, manifest root/proof, executed
wire, exact receipt group, and active incarnation.
`StateBlock::stage_native_amx_participant_frontiers` in
`crates/iroha_core/src/state.rs` publishes WSV markers only after durable
evidence, while `crates/iroha_core/src/sumeragi/v2_apply.rs` invokes live
persistence in durability order. Unified startup planning/application lives in
`crates/iroha_core/src/sumeragi/v2_lane_work.rs`; `run_inner` in
`crates/iroha_core/src/sumeragi/v2_runner.rs` keeps the Queue gate closed until
that repair and exact readback complete. Pruned bodies remain verifiable
through QC-authenticated manifest evidence; weaker hash-only evidence remains
fail-closed. The Kura namespace contains one
immutable versioned manifest file and one immutable versioned receipt file per
participant height, followed by a version-2 route/incarnation- and
application-identity-bound replaceable exact-latest pointer. The pointer binds
the executed-wire, finality-artifact, and manifest-artifact hashes and startup
accepts it only when the exact retained receipt or QC-authenticated manifest
backs every field. Publication uses create-new temporaries, no-clobber
promotion, file and directory durability sync, and exact readback.

**Closure condition.** Persist finality plus the immutable per-height manifest
first, then the matching immutable per-height receipt and exact-latest pointer,
and only then publish replicated WSV frontiers. Retain the canonical executed
wire until the manifest and receipt are durable.
After body pruning, validate through the QC-authenticated manifest root and
proof. Hash-only legacy evidence stays fail-closed unless the exact canonical
wire is recovered from authenticated storage or QC signers. Startup must
idempotently repair a finalized marker missing its receipt or latest pointer
by revalidating block, checkpoint, finality, manifest, roots, and exact group
under the publication guard without recursive locking. Recovery must either
promote a valid lone publication temporary, remove a byte-identical temporary
beside its stable file, or reject malformed, oversized, conflicting, or
ambiguous temporary state.

The startup owner plans ordinary certified lane receipts and Native frontier
markers together through `plan_lane_application_evidence_repair`, without
publishing either class. Owners naming the same canonical carrier coalesce
into one `CanonicalExecutedBlockNeedV1`; after authenticated body recovery the
complete plan is rebuilt. The ordinary owner is the immutable
`State::lane_application_certified_repair_snapshot_cached` projection: it uses
`Kura::preflight_latest_certified_lane_block_frontier_with_authority` and
`Kura::read_certified_lane_block_artifact_read_only`, and every receipt,
predecessor, and snapshot-anchor lookup selects Kura's no-sidecar-repair read
path. Frontier-backed indexed-pair repairs are returned as explicit plan items;
they are not published by planning. `apply_lane_application_evidence_repair`
repeats every ordinary and Native preflight before its first write, explicitly
publishes those lifecycle-bound ordinary pair repairs and every exact evidence
item, and performs authoritative readback while the Queue startup gate remains
closed. Only then may lane-reservation ownership reconciliation run.
The carrier index is validated in both directions: finality `Some(carrier)`
has one exact carrier record, finality `None` has none, and caching a recovered
body preflights reverse carrier reconstruction before retiring the body need.
The rebuilt all-item plan then persists and revalidates that carrier record
before evidence readback and Queue reconciliation.

**Focused and adversarial tests.** Inject a crash before/after finality,
manifest, receipt, latest pointer, frontier, and body-retention release. Cover
body eviction, missing/corrupt checkpoint or finality, forged manifest proof,
conflicting marker, repair repeated twice, repair interrupted at every write,
valid lone and duplicate publication temporaries, malformed/conflicting/
oversized temporaries, recursive-lock regression, and bounded authenticated
wire recovery. Exercise the versioned prune intent as a temporary only, as a
stable intent before deletion, after each individual manifest/receipt unlink,
after the complete pair unlink, and with identical stable and temporary copies;
each restart must finish idempotently without deleting the exact latest pair.
Current source anchors include
`pair_only_application_evidence_repair_counts_as_progress`,
`canonical_body_recovery_batches_all_ordered_heights_before_gate_close`,
`read_only_certified_frontier_preflight_plans_reused_slot_without_mutation`,
`certified_lane_predecessor_rejects_nonzero_height_without_descriptor_hash`,
and
`native_participant_missing_carrier_uses_generic_chunk_recovery_then_repairs_receipt`.
They cover pair-only progress accounting, ordered coalesced body recovery,
read-only reused-slot planning, fail-closed predecessor identity, and repair
after generic authenticated carrier recovery, respectively. These are mapped
source tests, not a fresh `G-UNIT` transcript.

**Formal obligation and mutation.** Invariant
`MLNativeDurabilityPrecedesFrontier` orders durable finality, immutable
manifest, immutable receipt, exact-latest pointer, and replicated frontier.
Mutation `ML-MUT-NAT-06` reorders any two boundaries or drops idempotent
repair; the model must expose an unverifiable frontier or lost durable
application. Its unified-startup controls are
`multilane_native_mutating_unified_startup_plan_bug.cfg`,
`multilane_native_uncoalesced_canonical_body_needs_bug.cfg`,
`multilane_native_partial_unified_startup_preflight_bug.cfg`,
`multilane_native_queue_before_evidence_readback_bug.cfg`,
`multilane_native_missing_reverse_merge_carrier_bug.cfg`,
`multilane_native_orphan_merge_carrier_bug.cfg`, and
`multilane_native_skip_post_cache_carrier_reconcile_bug.cfg`; each must violate
`MLUnifiedStartupEvidenceRepairSafe`.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-FINAL`.

### ML-NAT-07 — bounded standalone evidence and exact-latest pointer

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** Kura persists immutable versioned manifest and receipt
files keyed by participant height. A separate version-2 route/incarnation-,
descriptor-, application-, executed-wire-, finality-, and manifest-bound latest
pointer is replaceable derived state used for bounded exact lookup and
explicitly reconstructed through
`Kura::rebuild_native_amx_participant_receipt_latest_indexes_on_startup`.
Startup rejects legacy V1 filenames and any current pointer not backed by its
exact retained receipt or QC-authenticated manifest, including binding drift
while the receipt is missing.
`Kura::ensure_first_release_lane_retirement_admissible_locked` in
`crates/iroha_core/src/kura/lane_geometry.rs` recognizes, accounts, validates,
archives, and purges the per-height manifests and receipts plus the exact-latest
pointer while rejecting legacy dense data/index layouts and malformed,
oversized, temporary, unexpected, non-regular, hardlinked, or symlinked
artifacts. A versioned prune intent binds route, incarnation, and every
`(kind, participant height, artifact hash)` removal before either member of an
old complete pair is unlinked.

**Closure condition.** Persist and rebuild one bounded exact-latest pointer
keyed by `(lane, dataspace, incarnation)`. Validate it against the highest
retained receipt and the exact finality/manifest/receipt/application join.
Retain each evidence kind within the configured Kura sidecar-retention count
and the existing shared Native sidecar aggregate-byte budget, allowing only one
bounded transient publication slot; derive the prune-intent count and byte
bounds from those same limits. Include per-height receipt and manifest files,
their proofs, the latest pointer, and an attributable prune intent in disk
accounting, archive authentication, retirement, purge, and same-ID recreation
allowlists. Reject legacy dense layouts, temporary ambiguity, unexpected,
malformed, truncated, oversized, non-regular, hardlinked, and symlinked
artifacts. Pointer reconstruction is an explicit startup operation, never an
unbounded read-path scan.

**Focused and adversarial tests.** Cover a missing, stale, conflicting,
truncated, oversized, or corrupt latest pointer; duplicate and conflicting
same-height immutable files; route/incarnation ABA; configured retained-count
overflow; per-kind aggregate-byte addition overflow and budget overflow;
unexpected, legacy-dense, malformed, and oversized temporary files; symlinked
or hardlinked file/directory components; exact disk-accounting boundaries;
archive/recovery/purge at each crash boundary; and exact recreation cleanup
without deleting a sibling lane.

**Formal obligation and mutation.** Invariant `MLNativeLatestIndexExact`
states that one bounded derived pointer names the unique latest revalidated
artifact for each active route/incarnation. Mutation `ML-MUT-NAT-07` binds only
the lane ID, accepts ambiguous same-height evidence, accepts a legacy dense
layout, or omits an artifact class from retirement; the model must expose stale
authorization or unsafe destruction.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-FINAL`.

## Canonical Kura body-retention closure

### ML-KURA-01 — authenticated deterministic keepers before body eviction

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open; current evidence is focused/source-bound only.

**Production map.** `KuraReplicaAdvertV1` and its domain-separated signature
preimage in `crates/iroha_core/src/sumeragi/message.rs` now bind chain, height,
block, executed-wire length/hash, exact finality-artifact hash, keeper index,
and keeper identity. `BlockMessage::is_live_auxiliary` admits that versioned
shape to the live-message family. `FairV2Ingress` and
`admit_kura_replica_advert_ingress` in
`crates/iroha_core/src/sumeragi/v2_runner.rs` require the signature-bound
keeper to be both semantic sender and direct `via` peer before
`Kura::admit_kura_replica_advert` revalidates exact durable chain/finality/wire
identity and deterministic keeper membership. `Kura::evict_block_bodies`
requires fresh adverts from every selected remote keeper and
`has_all_selected_remote_keepers` pins a selected local keeper when the local
identity is bound. The standard launcher in `crates/irohad/src/main.rs` now
binds the configured node `PeerId` before `Kura::start`, making that pin
reachable in production; `Kura::start` rejects an unbound local identity.
`KuraReplicaAdvertPolicy::validate` supplies configured replica count,
evictable window, TTL, and refresh cadence and checks the complete registry key
and entry geometry, including the typed two-millisecond TTL floor and
one-millisecond refresh floor, before `Kura::new_inner` constructs the store.
Refresh-owner construction repeats the refresh minimum as a defense-in-depth
boundary.
`KuraReplicaAdvertSourceV1` is the body-free exact source token: it binds chain,
height, block, executed-wire length/hash, finality artifact, keeper index, and
keeper. `exact_kura_replica_advert_tip` captures height and hash under the prune
and canonical-chain guards; source probing authenticates the durable index,
finality, and keeper without reading the body; and construction plus rollover
revalidation repeat the exact source and complete-body checks before signing or
publication. The runner creates one process-lifetime
`KuraReplicaAdvertRefreshOwner` outside its height loop, shares it with each
`ProductionV2Services`, advances it after accepted durable application and on
bounded service turns, and publishes only through the guarded exact-output
corridor. Its configured evictable-first arithmetic window is anchored to one
exact tip, probes at most eight heights and transfers at most one fanout per
turn, retains one reconstructible source across backpressure, and anchors the
next deadline when a scan starts so a slow scan does not add a second refresh
interval. Applied-height handoff validates each pending advert's exact rollover
claim, completes durable reconstruction first, then wakes the owner with only
the unique source heights inside its active window. That bounded urgent-height
set is pruned on tip change and serviced before ordinary cursor work. The owner
also schedules a follow-up for same-height tip replacement without resetting
the current cursor. Restart creates a fresh owner from the exact durable tip and
the authenticated remote advert registry remains restart-empty.

**Closure condition.** Admit only a bounded, versioned advert whose signature
and direct transport sender/via identity are the same keeper. Kura must
revalidate the exact canonical block, V2 finality artifact, complete-wire
length/hash, frozen roster, CommitQC signer bitmap/quorum, keeper index, and
deterministic keeper selection. Select at least `f + 1` keepers from the exact
CommitQC signer set using canonical chain/context/block/finality identities,
where `f` is derived from the frozen roster and quorum requirement rather than
the observed signer surplus. A local selected keeper may never evict its body;
a non-keeper may evict only after every selected keeper has one fresh exact
advert. Registry count, TTL, refresh scan, and per-turn output must come from
bounded `iroha_config` values. Production receive, post-durable-apply send,
bounded historical refresh, restart-empty reconstruction, and exact-output
rollover must all use the same authority predicate.

**Focused and adversarial tests.** Reject forged version, chain, block,
executed-wire length/hash, finality identity, keeper index, keeper identity,
signature, transport sender, and relay/via identity. Reject a valid signer that
is not selected, an alternate same-decision QC/finality artifact, under-`f+1`
keeper selection, expired adverts, registry overflow, and a refresh/output
burst above its bound. Pin a selected local keeper. Cover validator rotation,
restart-empty registry reconstruction, exact-output height rollover, concurrent
eviction/advert admission lock order, and body recovery from every advertised
keeper in four- and twelve-peer corridors. Current source anchors are
`kura_replica_advert_signature_binds_every_eviction_identity`,
`kura_replica_advert_is_live_auxiliary_not_lane_or_global_v1`,
`authenticated_replica_admission_rejects_forgery_non_qc_peer_and_alternate_finality`,
`deterministic_commit_qc_keepers_use_f_plus_one_and_pin_a_local_keeper`, and
`expired_replica_adverts_do_not_allow_eviction`; startup and producer ownership
are pinned by `kura_start_rejects_unbound_local_peer_identity`,
`standard_launcher_binds_kura_local_peer_before_start`,
`refresh_window_is_evictable_first_and_overflow_safe`,
`refresh_turn_retains_one_source_and_attempts_at_most_one_fanout`, and
`same_height_tip_rewrite_requests_follow_up_without_starving_current_cursor`.
`durable_kura_replica_advert_rollover_claim_rejects_identity_and_recipient_drift`
now source-pins the actual-Kura path from refresh publication through exact-
output backpressure, successful durable-handoff urgent wake-up, predecessor
drop, successor retry, frozen exact recipients, and final Kura revalidation.
Fresh same-day isolated Rust 1.93.1 locked/offline slices passed the 18 exact
Kura replica tests and four exact configuration tests. Those focused runs and
source anchors are not the complete 418-test `G-UNIT` receipt, and no
multi-peer body-pruning corridor was run, so this row's evidence remains Open.

**Formal obligation and mutation.** `ML-MUT-KURA-01` now owns the source-bound
`KuraReplicaRetentionProductionRefinementObligation` and all fourteen exact
Kura mutation configurations. `SumeragiV2KuraReplicaRetention.tla` models
signature and direct sender/via authentication, exact finality and wire
identity, deterministic `f + 1` selection, non-signer rejection, local-keeper
pinning, all-selected-remote freshness, TTL and restart invalidation, finite
registry capacity, bounded refresh, and the final pre-stage recheck. The
structural checker binds the settled receive/authority/eviction symbols and
the checked configured registry geometry, exact tip/source construction, and
the bounded process-lifetime refresh owner. The schema-5 binding contract has
no pending Kura source check. TLC and Apalache still must produce fresh archived
positive and expected-counterexample evidence, so this model addition does not
close `G-FORMAL`.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, `G-SCALE`, and
`G-FINAL`.

## QueuePlan admission closure

### ML-QUEUE-01 — globally unique durable admission before autonomous ownership

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** `QueuePlanAdmissionBindingV2` and
`validate_queue_plan_admission_certificate_for_chain_digest_v2` in
`crates/iroha_core/src/torii_proxy.rs` define the exact request, transaction,
routing-plan, context, enqueue-time, and journal-record identity certified by
the coordinator quorum. `persist_and_wait_for_queue_plan_admission` and
`submit_signed_transaction_for_ingress_globally_synced` in
`crates/iroha_torii/src/lib.rs` persist the certificate before wakeup and wait
for the exact replicated WSV registry value before returning public acceptance.
`State::stage_queue_plan_admissions` owns the immutable global CAS and stages a
V1 pending obligation plus one exact member for each deduplicated
route/incarnation. The lexically ordered route-member key range is the
authoritative roster; its consensus cap is exactly
`MAX_MERGE_QUEUE_PLAN_ADMISSIONS`, and no count/XOR summary is removal or drain
authority. Each member payload binds the route, chain digest, typed entrypoint,
binding hash, and domain-separated member identity. Its decoder rejects empty
or greater-than-1,024-byte payloads before Norito decoding, then requires exact
canonical bytes and the exact derived key. Bounded prefix enumeration rejects
cap overflow, malformed or wrong-route members, and any member without its
exact pending obligation. Registry/application-state reads enumerate that
roster and require the obligation's exact member instead of trusting a lossy
summary. The obligation and all of its members are inserted or removed through
one nested MV transaction; whole-list admission staging, ordinary bulk
resolution, and autonomous required resolution call `apply` only after every
item succeeds. Both native marker prefixes are in
`OPAQUE_SYSTEM_CONTRACT_STATE_PREFIXES`, so generic contracts cannot read,
forge, overwrite, enumerate, or delete either half of the paired state.

Pending application state is lifecycle-aware. It is `Pending` only when every
bound lane/dataspace remains in the current Nexus catalog and
`lane_incarnation_at_height` matches the admission proposal height; a coherent
obligation on a retired or recreated incarnation is `PendingStale`. Queue
ownership and the public binding/presence APIs reject `PendingStale`, while the
authenticated durable-certificate reconciler classifies it as `Stale` for
exact cleanup. An already-applied registry owner remains historical `Applied`
and is not reclassified by current lane geometry.
`Queue::reserve_transactions_for_lane_bounded`, queue replay/expiry, and
`V2LaneWorkAdapter::refresh_merge_candidates` admit autonomous ownership only
for the exact binding, preserve the durable tombstone across restart/TTL, and
reject or clean up a definitive conflict through the authenticated loser path.

**Closure condition.** One transaction/request identity may acquire at most one
global QueuePlan binding. A public `202` requires the exact coordinator quorum
certificate to be durable, its wakeup to be published, and the matching WSV
registry value to be visible. Autonomous reservation and execution require
that exact immutable binding. Restart, local expiry, cancellation, guard drop,
or a deferred path must neither erase the durable owner nor bypass the global
CAS; a conflicting binding must fail closed and can never execute. Every
pending obligation contributes exactly one member to every deduplicated bound
route and may be removed only with that exact member. The bounded lexical
roster validates every enumerated compact canonical member claim and requires
its exact obligation key to exist without decoding a potentially 2 MiB
obligation per roster item. Target application and resolution still decode and
compare the exact obligation/member pair, and every known obligation is checked
for its exact member on each bound route.
Missing, phantom, orphaned, malformed, oversized, wrong-key, or over-cap
evidence must fail closed without publishing a stage or resolution prefix;
same-route coordinator and participant roles contribute only one route member.
The drain path performs this bounded authoritative route-range check, not an
unbounded reverse history scan. Arbitrary coordinated mutation of opaque
post-state is rejected by canonical WSV/block-root validation. A lane-ID
recreation must make the old still-pending owner stale and must never authorize
ownership on the new incarnation.

**Focused and adversarial tests.** Cover split-route public acceptance,
execution before global CAS, two conflicting CAS attempts, restart ABA, local
TTL expiry, deferred ingress, cancellation, guard drop, missing or mismatched
binding material, and duplicate execution. Inject every crash boundary between
certificate persistence, wakeup, WSV publication, queue reservation, carrier
application, and authenticated loser cleanup. Also cover a later-route orphan
member during all-route stage preflight, two obligations sharing one route,
missing and wrong-key exact members, malformed and greater-than-1,024-byte
member payloads, a phantom roster member, an omitted real member, cap plus one,
exact last-member removal, same-ID incarnation recreation, and byte-identical
state after any failed whole-list stage or bulk-resolution preflight.

**Formal obligation and mutation.** The bounded
`SumeragiV2QueuePlanAdmissionRegistry` kernel checks
`MLAdmissionCasUnique`, `MLCertificateDurable`, `MLPublic202Exact`,
`MLExecutionRequiresExactBinding`, `MLQueueEligibilityExact`,
`MLAdmissionAtMostOnceExecution`, `MLImmutableAdmissionTombstone`, and
`MLCancellationStopsExecution`. Conceptual mutation `ML-MUT-QUEUE-01` maps
only to the ten QueuePlan `_bug.cfg` controls recorded in
`multilane_source_bindings.json`; each must produce its exact named TLC
counterexample. The positive Apalache run checks the fixed kernel only and is
not a mutation runner or deductive proof. The structural refinement guard
source-binds the concrete exact-member identity, the 1,024-byte pre-decode cap,
the merge-admission roster cap, compact canonical member validation plus exact
obligation-key existence without full-obligation decoding during enumeration,
exact-member checks for known obligations, the opaque native namespaces, the
active-incarnation classifier and all of its Queue/public/cleanup consumers,
and the apply-only-after-complete-preflight stage and resolution order. It
also rejects any return to count/XOR authority and does not invent an
unbounded drain-time history scan. These source guards preserve the existing
formal action inventory; they do not claim a new TLC transition or close
`G-FORMAL`.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-FINAL`.

## Autonomous lane execution closure

### ML-AUT-01 — durable FIFO queue reservation

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** `LaneQueueReservationKeyV2`,
`LaneQueueReservationStore`, `Queue::reserve_transactions_for_lane_bounded`,
`Queue::retain_lane_reservation`, `Queue::release_lane_reservation`,
`Queue::commit_lane_reservation`,
`Queue::reconcile_orphaned_lane_reservations`, and
`Queue::prune_lane_reservations` live in
`crates/iroha_core/src/queue.rs`. `crates/irohad/src/main.rs` installs the
journal. The deterministic production producer in
`V2LaneWorkAdapter::schedule_autonomous_lane_production` performs bounded FIFO
selection and calls `Queue::reserve_transactions_for_lane_bounded` before
payload publication; losing and retired work use the exact release path.

**Closure condition.** The deterministic lane leader selects a non-empty FIFO
batch and fsyncs one exact ordered V5 batch of `LaneQueueReservationKeyV2`
records before ownership leaves the ordinary queue. Selection binds active route/incarnation and
canonical enqueue order, and requires the admission binding to be an exact
immutable WSV registry match. No transaction can be visible to both owners or
to neither owner at any crash point.

**Focused and adversarial tests.** Cover empty and bounded batches, the 4,097
entry rejection before FIFO or journal mutation, canonical slot authority, FIFO order,
duplicate transactions, two lanes racing for one transaction, stale
incarnation, reservation-key duplication, fsync/write/rename failure, crash
before and after ownership transfer, restart reconciliation, and bounded prune
that cannot remove a live reservation.

**Formal obligation and mutation.** Invariant `MLReservationSingleOwner`
states that queued and reserved ownership are disjoint and exhaustive.
Mutation `ML-MUT-AUT-01` transfers ownership before durable reservation or
allows duplicate reservation keys; the model must expose loss or double
ownership.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, and `G-12P`.

### ML-AUT-02 — immutable reservation identity through lane consensus

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** Versioned executable payload, reservation, handoff, vote,
QC, and NewView messages are defined in
`crates/iroha_core/src/sumeragi/message.rs` and
`crates/iroha_core/src/lane_consensus.rs`.
`V2LaneWorkAdapter::accept_lane_message_owned` admits and independently
validates the live autonomous variants. Kura payload, availability, NewView,
certified-bundle, merge, and application artifacts retain the exact
reservation keys through the carrier.

**Closure condition.** Carry the byte-identical reservation identity through
the executable payload, routing plans, Native receipts, lane proposal,
availability QC, lane Commit QC, durable autonomous bundle, global merge
candidate, and canonical application. Validators independently verify
transaction/routing identity, active incarnation, predecessor, reservation
uniqueness, payload hashes, and identical Prepare/Commit payloads without
consulting proposer-local journal state.

**Focused and adversarial tests.** Mutate one reservation byte at every
handoff. Cover transaction reorder/substitution, routing-plan drift, Native
receipt drift, wrong incarnation/predecessor, duplicate reservation, payload
hash mismatch, Prepare/Commit mismatch, malicious proposer-local journal state,
delayed handoff, and valid follower reconstruction with no local reservation.

**Formal obligation and mutation.** Invariant `MLReservationIdentityStable`
states that the reservation digest is constant from queue acquisition through
global Commit. Mutation `ML-MUT-AUT-02` permits any handoff to reconstruct or
drop the identity; the model must expose acceptance of a different batch or
unreleasable ownership.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-SDK`.

### ML-AUT-03 — live availability, lane QC, and certified-bundle durability

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** Production persists and revalidates autonomous work through
`Kura::persist_lane_executable_payload`,
`Kura::persist_lane_payload_availability_certificate`,
`Kura::persist_lane_new_view_certificate`,
`Kura::autonomous_lane_merge_bundle`,
`Kura::validate_autonomous_lane_merge_bundle`, and
`Kura::decode_autonomous_lane_merge_bundle`.
`V2LaneWorkAdapter` drives bounded authenticated ingress, availability,
Prepare/Commit, timeout, NewView, durability, and fanout; only an exact durable
certified bundle becomes merge-eligible.

**Closure condition.** Enable bounded authenticated payload, handoff,
availability, Prepare, Commit, timeout, and NewView handling. Persist the exact
certified autonomous bundle before it is merge-eligible. Hydration accepts only
fully revalidated current-incarnation artifacts and protects quorum-certified
work from ordinary eviction.

**Focused and adversarial tests.** Cover unavailable/tampered payloads,
wrong committee or quorum, forged availability/Prepare/Commit QCs, mismatched
payloads, duplicate/conflicting slots, stale view/incarnation, malicious
NewView, partial sidecar publication, restart at every QC boundary, protected
cache pressure, delayed messages after recreation, and Byzantine/offline
validator rotation.

**Formal obligation and mutation.** Invariant `MLCertifiedBundleDurable`
states that merge eligibility implies durable exact payload plus matching
availability, Prepare, and Commit evidence. Mutation `ML-MUT-AUT-03` makes
eligibility precede persistence or permits a mismatched QC; the model must
expose an unrecoverable or uncertified merge source.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, and `G-12P`.

### ML-AUT-04 — canonical merge candidate, QC, and bounded sidecar recovery

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** `State::build_merge_execution_batch`,
`State::build_merge_execution_batch_for_consensus`, and
`State::build_merge_execution_batch_from_source_prefix` in
`crates/iroha_core/src/state.rs` build the canonical contiguous execution
prefix against the exact current base state. Production
`V2LaneWorkAdapter::refresh_merge_candidates` in
`crates/iroha_core/src/sumeragi/v2_lane_work.rs` synthesizes and signs
execution-bearing candidates without the former `execution_batch.is_none()`
production exclusion.
`V2LaneWorkAdapter::accept_certified_merge_sidecar_request` serves exact
authenticated entry bytes only from designated holders and the bounded
sidecar client validates completed entries before installation.

**Closure condition.** Remove the `execution_batch.is_none()` exclusion only
after `ML-AUT-01` through `ML-AUT-03` close. The global leader selects a
contiguous source prefix in canonical lane order against the exact current base
WSV. Merge QC signs source identities, base state, ordered execution/results,
write set, post state, and batch hash. Missing bounded sidecars are served and
fetched only from authenticated committee/QC sources.

**Focused and adversarial tests.** Cover gaps, reordered lanes, duplicate
sources, stale incarnation, wrong base state, tampered execution/result/write
set/post state/batch hash, forged QC, non-signer source, over-budget chunk or
entry, timeout/retry/duplicate chunks, unavailable sidecar, and exact-view
leader failover. Assert the producer exclusion is absent in production.

**Formal obligation and mutation.** Invariant `MLMergeCandidateExactPrefix`
states that one merge QC authenticates one canonical contiguous source prefix
and one base-state transition. Mutation `ML-MUT-AUT-04` drops contiguity,
canonical order, base state, a result/write-set field, or authenticated fetch
origin; each mutation must admit divergence or equivocation.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-FINAL`.

### ML-AUT-05 — canonical re-execution, atomic application, and reservation retirement

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** `MergeExecutionBatch` and
`MergeLedgerEntry::execution_batch` are defined in
`crates/iroha_data_model/src/merge.rs`.
`State::validate_merge_candidate_for_global_round`,
`State::validate_merge_execution_batch`, and
`StateBlock::stage_certified_merge_entry` in
`crates/iroha_core/src/state.rs` provide deterministic follower validation and
staging. Duplicate transaction membership is rejected in
`crates/iroha_core/src/block.rs`. The application path in
`crates/iroha_core/src/sumeragi/v2_apply.rs` finalizes queue reservations only
after canonical Kura/WSV commit.

**Closure condition.** Every peer deterministically re-executes the exact
certified batch on the committed base WSV and atomically commits canonical
Kura/WSV state. Only afterward may the queue transition the exact reservation
through Commit and ForgetCommit. A losing proposal, timeout, reconfiguration,
or retirement releases the exact reservation in original enqueue order.
Restart reconciliation retains a reservation if and only if exact durable
payload/certification/application evidence owns it.

**Focused and adversarial tests.** Cover base-state mismatch, nondeterministic
result attempt, tampered results/write set/post state, duplicate ordinary/merge
membership, Kura-before-WSV and WSV-before-queue crash boundaries, each
Commit/ForgetCommit boundary, repeated reconciliation, losing/future/stale
proposal, timeout, reconfiguration, retirement, and release-order restoration.
Assert exactly one history/query inclusion result.

**Formal obligation and mutation.** Invariant `MLCarrierExactlyOnce` states
that each accepted reservation has one of three terminal outcomes: globally
applied once, released once in FIFO position, or durably retained by one live
certified owner. Mutation `ML-MUT-AUT-05` advances queue state before Kura/WSV,
drops re-execution, or treats any local payload as ownership; the model must
expose loss, duplication, or stale retention.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-FINAL`.

### ML-AUT-06 — restart ownership reconciliation

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** Startup repeatedly calls
`plan_lane_reservation_ownership` and then
`apply_lane_reservation_reconciliation_plan` from
`crates/iroha_core/src/sumeragi/v2_apply.rs` through
`crates/iroha_core/src/sumeragi/v2_runner.rs`. The immutable plan classifies
every reservation group together against State, Kura's bounded autonomous
evidence classifier, pending merge entries, and exact committed carrier
indices. Missing canonical bodies enter authenticated, fixed-chunk
Commit-QC-signer recovery; pruned historical autonomous carriers require an
exact durable recovery installation before any Queue mutation. Application
then retains one authenticated owner, releases orphans in original enqueue
order, resumes terminal Commit/ForgetCommit, and opens the Queue startup gate
only after all groups and crash barriers have completed.

**Closure condition.** Replace the production false stub with exact,
bounded, authenticated ownership reconciliation. Retain only reservations
matched by current-incarnation durable payload/certified-bundle/global
application state. Release every other reservation exactly once in original
enqueue order. Prune only terminal forgotten reservations.

**Focused and adversarial tests.** Restart after queue reserve, payload fsync,
availability QC, Prepare QC, Commit QC, certified-bundle fsync, merge-QC
certification, Kura commit, WSV commit, reservation Commit, and ForgetCommit.
Cover stale-incarnation sidecars, duplicate reservation files, ambiguous
evidence, corrupt/oversized artifacts, and repeated startup. The focused
`historical_autonomous_recovery_is_safe_across_same_lane_b_a_b_recreation`
regression in
`crates/iroha_core/src/kura/tests/07b_autonomous_reservation_reconciliation_tests.rs`
exercises one lane ID across B/A/B storage generations, including a deliberate
incarnation-hash ABA with a fresh activation fence: delayed B/A recovery
records, QCs, payloads, and physically copied archive bytes fail closed, the
recreated-B evidence remains byte-exact, and restart recovers only recreated-B
ownership. The final current-source checkpoint passed this exact B/A/B
regression (`1/1`), the startup-replay binding regressions (`2/2`), the bounded
historical namespace/accounting suite (`6/6`), first-merge crash-window repair
(`1/1`), Native post-WSV retention (`1/1`), and authenticated geometry refresh
(`1/1`) under isolated Rust 1.93.1 locked/offline execution. These 12 focused
tests are mapped row evidence, not the complete 418-test `G-UNIT` receipt, so
this row's evidence remains Open.

**Formal obligation and mutation.** Invariant `MLRestartOwnershipPartition`
states that startup reconstruction preserves the single-owner partition.
Mutation `ML-MUT-AUT-06` treats all or no payloads as owners, ignores
incarnation, or performs non-idempotent release; the model must expose loss,
duplication, or ABA retention. Invariant
`MLRecoveredCarrierBodyAuthenticated` requires one retained Commit-QC signer
to own the whole bounded canonical-body assembly; `ML-MUT-AUT-07` accepts an
unauthenticated body or mixes chunks across signers. Invariant
`MLHistoricalRecoveryContextExact` binds the installed task to the exact
historical route, incarnation, predecessor, proposal, committee, quorum, and
validator PoPs; `ML-MUT-AUT-08` drifts that context. Invariant
`MLHistoricalQueueGateOrder` keeps ordinary Queue selection closed until body
recovery, durable task installation, and reconciliation preflight complete,
then permits it to open before quorum certification; `ML-MUT-AUT-09` opens the
gate early. Invariant `MLHistoricalAllGroupsPreflight` preserves the exact
owner until every reservation group has passed preflight;
`ML-MUT-AUT-10` publishes a partial group prefix. Invariant
`MLRecoveredCarrierLengthAuthenticated` requires the exact complete-wire
length to be Commit-QC-signed and cross-checked against retained Kura/index
evidence before allocation; `ML-MUT-AUT-11` lets a recovery peer inflate that
length.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, and `G-12P`.

## Automatic lifecycle closure

### ML-LIFE-01 — deterministic create and activate

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** Configuration-driven sampling, cooldown, managed-lane
selection, pinned committee, future activation height, incarnation allocation,
and geometry journaling are implemented across
`crates/iroha_core/src/state.rs` and
`crates/iroha_core/src/kura/lane_geometry.rs`.

**Closure condition.** Keep production behavior exclusively under
`iroha_config`. Initialize new lane storage and catalog state atomically before
activation. The first eligible proposal height must remain after the lifecycle
carrier, and the new incarnation must be visible consistently to routing,
queue, lane consensus, Native AMX, Kura, diagnostics, drain, and retirement.

**Focused and adversarial tests.** Cover threshold edges, sampling windows,
cooldown, committee selection/quorum, insufficient committee, configuration
restart, crash at every geometry journal phase, future-activation messages,
concurrent queue arrival, failed preflight rollback, and deterministic
expansion across four and twelve peers.

**Formal obligation and mutation.** Invariant `MLActivationAfterAtomicCreate`
states that no route or proposal can observe a lane before catalog and storage
for the same incarnation are durable. Mutation `ML-MUT-LIFE-01` exposes routing
before geometry or permits same-height activation; the model must expose
partial activation.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-SCALE`.

### ML-LIFE-02 — one evidence-aware drain frontier and complete blockers

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** `State::pending_autoscale_lane_drain_body` and
`StateBlock::select_autoscale_scale_in_action` in
`crates/iroha_core/src/state.rs` derive and recheck the same
`evidence_aware_lane_drain_frontier_from_world`.
`State::lane_has_drain_blocking_evidence` covers ordinary queued work,
live reservations, certified/unmerged and delayed lane work, pending merge
entries, and unapplied/unverifiable Native controls. Kura revalidates the exact
per-height manifest, matching per-height receipt, descriptor-bound latest
pointer, finality, checkpoint, and application identity before a
Native-derived frontier can be used.

**Closure condition.** Use one drain predicate and one frontier for intent,
vote/certificate validation, global commitment, archive, and removal. Ordinary
queue work, live reservations, certified-unmerged autonomous bundles, delayed
work, pending merge entries, and unapplied or unverifiable Native controls are
blockers. A Native-derived frontier is drainable only while its exact manifest
proof, receipt, latest pointer, application block, finality, and checkpoint
revalidate.

**Focused and adversarial tests.** Exercise every blocker alone and in
combination, including a blocker arriving after intent or certificate but
before retirement. Cover locally missing evidence, body pruning, corrupt
receipt/latest-pointer/proof, pending fetch, delayed pre-close work, close-height
boundaries, conflicting same-height frontiers, and no skipping of a blocked
highest lane.

**Formal obligation and mutation.** Invariant `MLDrainImpliesNoOwnedWork`
states that every drain phase uses the same evidence-aware frontier and that
retirement implies no work owner remains. Mutation `ML-MUT-LIFE-02` removes
each blocker or substitutes a hash-only Native frontier; each mutation must
produce lost work or unsafe retirement.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, and `G-12P`.

### ML-LIFE-03 — live drain vote and certificate collection

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** `SumeragiHandle::try_incoming_lane_drain_vote` in
`crates/iroha_core/src/sumeragi/mod.rs` admits authenticated votes into the
bounded live relay. `V2LaneWorkAdapter` installs the queue and durable signing
guard, validates/aggregates the pinned committee, relays votes, and calls
`State::merge_drain_candidate_for_next_carrier` for the global carrier.
`StateBlock::select_autoscale_scale_in_action` requires a later block and
rechecks the committed frontier and every live blocker before retirement.

**Closure condition.** Authenticate, bound, validate, journal, aggregate, and
relay drain votes from the pinned historical committee. A signer cannot vote
below a Commit it signed, vote for conflicting/regressing bodies, or sign a
later Commit after close. The deterministic global leader carries the exact
certificate, and retirement occurs only in a later block.

**Focused and adversarial tests.** Cover wrong sender/committee/incarnation,
under-quorum and bitmap drift, duplicate/conflicting votes, regression,
post-close Commit, oversized frame, malformed PoP/signature, restart before and
after each journal/certificate/carrier boundary, offline/Byzantine rotation,
late pre-close work, and rejection of same-carrier retirement.

**Formal obligation and mutation.** Invariant `MLDrainCertificateMonotonic`
states that a certificate is quorum-authenticated, nonregressing, and strictly
precedes retirement. Mutation `ML-MUT-LIFE-03` removes the signed-Commit floor,
close fence, quorum, or later-carrier requirement; the model must find its
expected counterexample.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, and `G-12P`.

### ML-LIFE-04 — atomic archive, destruction, and same-ID recreation

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** Geometry transitions, recovery archives, and archive GC
live in `crates/iroha_core/src/kura/lane_geometry.rs`. Production retirement
uses `Kura::ensure_first_release_lane_retirement_admissible_locked`, whose
bounded scanner recognizes and authenticates autonomous payload, availability,
NewView, certificate, merge/application receipt, immutable per-height Native
manifest/receipt pairs, their descriptor-bound latest pointer, and release
evidence. It requires the exact finality/manifest/receipt/latest-pointer join,
rejects obsolete dense Native data/index files, and applies the configured
retention and shared aggregate-byte budgets before archive. Geometry journaling
archives the exact incarnation before removal, resumes interrupted archive/GC
idempotently, and same-ID provisioning allocates a fresh incarnation.

**Closure condition.** Archive every recognized lane evidence class atomically
before removing active storage. Bind every archive and purge operation to the
exact retired incarnation. On recreation allocate a fresh incarnation and
reject all delayed QCs, reservations, markers, receipts, manifests, latest
pointers, prune intents, journal claims, merge entries, and diagnostic evidence
from every earlier incarnation. Preserve authenticated historical proof only
where policy requires it.

**Focused and adversarial tests.** Cover archive validation and every journal
crash phase; malformed, oversized, temp, unexpected, non-regular, and symlinked
or hardlinked artifacts; configured count and aggregate-byte overflow; disk
accounting; partial archive pairs; purge/recovery idempotence; same-ID A/B/A
recreation; delayed old QC/reservation/marker/sidecar/claim/merge artifact; and
sibling-lane preservation.

**Formal obligation and mutation.** Invariant `MLRetirementConsumesExactIncarnation`
states that destruction consumes all and only the certified retired
incarnation's work/evidence. Mutation `ML-MUT-LIFE-04` keys cleanup or
admission by lane ID, omits an artifact class, or publishes removal before the
archive; the model must expose ABA acceptance, loss, or sibling corruption.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-FINAL`.

### ML-LIFE-05 — bounded stage diagnostics and the twelve-peer stall

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** `SumeragiAutonomousLaneExecution`,
`SumeragiAutonomousLaneExecutionStage`, and the bounded stuck-reason enum in
`crates/iroha_data_model/src/block/consensus.rs` identify reservation,
payload, availability, lane certification, bundle, merge, carrier,
Kura/WSV receipt, queue-finalization, and conflict stages. State derives the
ordered bounded vector from replicated State plus Kura evidence and Torii
publishes it only on diagnostics. Live production candidate synthesis and
canonical carrier application now connect the former recovery-to-application
gap. The twelve-peer rerun required to prove that correction empirically
remains `G-12P`.

**Closure condition.** Add bounded, deterministic, operator-safe counters or
records for every stage without storing unbounded transaction material.
Correlate by typed source/reservation/route/incarnation identity. Use the
instrumentation to identify and fix the stall; keep it in production
diagnostics after the fix.

**Focused and adversarial tests.** Verify deterministic ordering, configured
count/byte caps, eviction behavior, no secret/raw payload leakage, conflict
reporting, restart derivation from State/Kura rather than cache, and one test
for every stage transition and stuck-stage reason. Reproduce the former stall
and assert carrier application completes.

**Formal obligation and mutation.** Invariant `MLStageEvidenceMonotonic`
requires stage evidence to advance only when its durable predecessor exists and
never to authorize state. Mutation `ML-MUT-LIFE-05` advances a diagnostic stage
from volatile cache or lets diagnostics affect selection; the model must expose
false progress or consensus dependence.

**Release gates.** `G-UNIT`, `G-4P`, `G-12P`, and `G-FINAL`.

## Public interface and cross-SDK closure

### ML-API-01 — authoritative status and Native application diagnostics

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** `/v1/sumeragi/status` returns
`SumeragiV2Status` from
`crates/iroha_data_model/src/block/consensus_v2.rs`.
`/v1/sumeragi/diagnostics` returns `SumeragiDiagnosticsStatus` from
`crates/iroha_data_model/src/block/consensus.rs`, assembled in
`crates/iroha_torii/src/routing.rs`. Its bounded, ordered
`native_amx_participant_applications` vector is derived by
`State::native_amx_participant_applications_diagnostics` from State plus Kura
evidence, filters exact active route/incarnations, and reports conflicting
same-height identity as `conflict`.

**Closure condition.** Keep authoritative reducer facts exclusively on
`/v1/sumeragi/status`. Add a bounded, deterministically ordered
`native_amx_participant_applications` diagnostics vector with one row per
active route/incarnation: lane, dataspace, incarnation, participant
height/view, predecessor/descriptor/proposal/settlement hashes, source count,
optional application block height/hash, and state
`certified_pending_carrier`, `committed_evidence_pending`,
`durably_applied`, or `conflict`. Derive it from State plus Kura evidence;
never select silently between conflicting same-height identities.

**Focused and adversarial tests.** Cover every state transition, empty and
bounded vectors, deterministic order independent of insertion/filesystem
order, active-incarnation filtering, same-height conflict, missing/pruned
evidence, malformed sidecars/indexes, restart, and assurance that no
authoritative consensus field migrates to diagnostics.

**Static release invariant and negative control.**
`MLDiagnosticsAreDerived` states that diagnostics are a bounded read-only
State/Kura projection and conflict is explicit. This is not a TLA+ invariant:
its source-bound Rust derivation and endpoint contract tests are the
authoritative check. Negative control `ML-MUT-API-01` resolves conflict by
arrival order, reads a volatile cache, or feeds diagnostics back into
consensus; the static/unit contract must then fail.

**Release gates.** `G-UNIT`, `G-12P`, `G-SDK`, and `G-FINAL`.

### ML-API-02 — separate client methods and mirrored models

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** Rust client methods live in
`crates/iroha/src/client.rs`. Swift status/diagnostics methods and models live
in `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`. Python surfaces live in
`python/iroha_torii_client/client.py` and
`python/iroha_python/src/iroha_python/client.py`. JavaScript methods live in
`javascript/iroha_js/src/toriiClient.js` and its generated distribution.
Kotlin diagnostics models live in
`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/consensus/SumeragiDiagnosticsModels.kt`;
the mirrored Java compatibility models live in
`java/iroha_android/src/main/java/org/hyperledger/iroha/android/consensus/SumeragiDiagnosticsModels.java`.
Each client keeps status and diagnostics on distinct methods, parsers, and
return models.

**Closure condition.** Give status and diagnostics separate parsers, return
types, and methods in every client. Extend Rust and Swift with the Native
application row and state enum. Add the full diagnostics surface to Kotlin
core-jvm and mirror it in Java. Python and JavaScript must not parse diagnostics
with the authoritative status parser or expose lane evidence through the
status-only method.

**Focused and adversarial tests.** For every SDK, test each endpoint and type
independently; reject swapped payloads, unknown/retired fields, missing required
fields, invalid enum states, malformed hashes/integers, over-bound vectors, and
nondeterministic order. Run source and generated/distribution JavaScript tests,
Swift tests, Kotlin core-jvm tests, and mirrored Java tests.

The source-sealed release corridor now owns exact no-skip inventories for the
Rust client (`14`), both Python client layers (`114`), JavaScript source plus a
freshly generated distribution (`88`, split `44 + 44`), Swift (`17`), Kotlin
core-jvm (`15`), and mirrored Java (`10`). JavaScript uses the dedicated
`sumeragiDiagnosticsContract.test.js` entrypoint instead of a monolithic
name-pattern filter, and the inventory includes an explicit swapped
status/diagnostics payload negative. The suite-source manifest SHA-256 is
`712bec0bf752ed650346c7588963d7e77117120c20f1c926e69f6ce21c3677b7`.
These source contracts do not close the still-unrun aggregate `G-SDK` gate.

**Differential release invariant and negative control.**
`MLApiAuthoritySeparation` states that no diagnostics-only field can satisfy an
authoritative status claim. This is not a TLA+ invariant: the source-bound
OpenAPI and SDK endpoint/parser corpus is the authoritative check. Negative
control `ML-MUT-API-02` aliases the two parsers or response types; a swapped
payload must then be accepted and the differential contract must fail.

**Release gates.** `G-SDK` and `G-FINAL`.

### ML-API-03 — identical Native V2 validation across SDKs

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** Rust Native AMX wire types and validators are in
`crates/iroha_data_model` and `crates/iroha_core/src/native_amx.rs`; client
mirrors are distributed across the Python, JavaScript, Swift, Kotlin, and Java
paths named in `ML-API-02`.

**Closure condition.** Every SDK enforces a tagged phase object; independent
global, coordinator, and participant views; distinct source-ID and typed
entrypoint-hash types; grouped bounds; the mixed-role deferred-validation
marker; ordered validator sets; exact bitmaps/quorum; 96-byte PoPs and
signatures; and unique bounded receipt legs. SDK convenience parsing must not
weaken consensus validation.

**Focused and adversarial tests.** Feed the same positive and negative corpus
to every SDK. Include phase/view drift, source/entrypoint type swap, group bound
edges, absent/forged deferred marker, reordered/duplicate validators, bitmap
padding and out-of-range bits, under/over quorum, 95/97-byte PoP or signature,
duplicate/oversized legs, wrong route/incarnation, and nested receipt material.

**Differential release invariant and negative control.**
`MLSdkAcceptSetEqualsRust` defines the Rust decoder/validator accept set as the
first-release contract. This is not a TLA+ invariant: the source-bound grouped
corpus executed by every SDK is the authoritative differential check. Negative
control `ML-MUT-API-03` weakens one SDK check; corpus execution must detect an
accept/reject mismatch.

**Release gates.** `G-SDK` and `G-FINAL`.

### ML-API-04 — Rust-owned grouped fixtures and explicit builders

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.**
`crates/iroha_data_model/src/bin/native_amx_grouped.rs` generates
`fixtures/sumeragi_v2/native_amx_v2_grouped.json`, including grouped golden
data and application evidence.
`NativeAmxAttestationBodyV2::computed_grouped_participant_settlement` is the
explicit production builder; single-source construction is labelled as a test
fixture.
`ci/run_native_amx_v2_grouped_sdk_parity.sh` source-binds the exact fixture and
OpenAPI, Python, JavaScript source/distribution, Swift, Kotlin, and Java
consumers. Fresh Rust generation of the protocol-4 corpus matches the checked-
in JSON byte-for-byte. It contains 52 negative controls, including
`execution_commitment_merge_carrier_wrong_version` and
`execution_commitment_missing_merge_carrier_field`; the harness and source-
bound release inventory both require that exact count. The synchronized
fixture SHA-256 is
`0fb9bf6a490f4974e65a5a03985bfe75321e3de1f54be064c3b088ccffc061d1`
and the current suite-source manifest SHA-256 is
`5cf86f7b08fdcb1bb95d144548e55efb18daa81f9194d8b9dd599313f7fc6d39`.
Direct OpenAPI, Python, and JavaScript grouped checks provide partial evidence
recorded under `G-SDK`; Rust, Swift, Kotlin, Java, and one complete archived
harness replay remain unexecuted, so this row's evidence is still Open.

**Closure condition.** Generate one canonical grouped fixture and negative
corpus from Rust and consume the exact files in OpenAPI, Python, JavaScript,
Swift, Kotlin, and Java. Replace any misleading singleton settlement helper
with an explicitly grouped production builder or a clearly named test-only
singleton fixture helper.

**Focused and adversarial tests.** Assert byte-for-byte/JSON parity and
deterministic regeneration. Include every `ML-NAT-04` and `ML-API-03` negative,
plus stale incarnation, same-route, mixed-role, manifest/proof, and application
block identity cases. A generated fixture diff must fail CI.

**Differential release invariant and negative control.**
`MLFixtureHasOneCanonicalOwner` states that Rust generation defines one
versioned corpus consumed without hand-edited SDK copies. This is not a TLA+
invariant: the source-bound regeneration guard, fixture hash, and per-SDK
suite-source manifest are the authoritative differential check. Negative
control `ML-MUT-API-04` changes one consumer fixture or uses a singleton helper
for a grouped case; parity CI must fail.

**Release gates.** `G-SDK` and `G-FINAL`.

### ML-WIRE-01 — explicit versioning and no implicit legacy decode

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** The signing guard V4, Native manifest V1, Native receipt
and latest-index layouts, queue reservation journal/key, executable payload
V2/envelope, lane QCs/NewView, merge entries, application receipts, and
diagnostics models all carry explicit versions or exact typed Norito layouts
in their owning modules. Unknown and retired layouts fail closed; the source
tree contains no implicit legacy consensus decoder or production
feature/environment compatibility switch for autonomous execution.
`crates/iroha_data_model/src/bin/sumeragi_v2_wire_fixtures.rs` now generates
the positive `quorum_certificate_merge_carrier` row and negative
`execution_commitment_merge_carrier_wrong_version` and
`execution_commitment_missing_merge_carrier_field` rows. Fresh Rust generation
matches the checked-in 48-line `fixtures/sumeragi_v2/wire_v2.tsv` byte-for-
byte, including all three rows; its SHA-256 is
`f4ed50fb3db8aba9a8f50c542a58b72099c162e5cb927637c83634bca5120ae7`.
`KuraReplicaAdvertV1` is explicit and clean-break. Its nested runtime policy
configures and validates TTL, refresh cadence, evictable window, replica floor,
and checked registry geometry. The direct authenticated ingress and the exact
source-bound, bounded production refresher both consume the V1 layout. The
former runtime-consumer gap and the wire-fixture regeneration gap are therefore
implemented; fresh cross-SDK and execution evidence remains open.

**Closure condition.** Version every new persistence and wire layout. Reject
unknown and legacy versions unless an explicitly authenticated migration
rebuilds the exact new identity. Do not add heuristic decode, mixed old/new
consensus, a feature/environment compatibility toggle, a new crate, direct
Serde dependency, Cargo.lock change, or ABI-version change.
Source every new runtime count/byte/time bound from `iroha_config`, give it a
sensible deterministic default, and reuse the existing sidecar/request budgets
where they already express the same resource.

**Focused and adversarial tests.** Round-trip each supported version; reject
zero, old, future, truncated, oversized, unknown-field, noncanonical, and
wrong-Norito-flag payloads. Exercise a mixed-version committee and require it to
fail closed before signing. Run the legacy-codec guard and assert no implicit
fallback decoder exists.

**Static release invariant and negative control.**
`MLConsensusLayoutAgreement` states that all signers interpret one versioned
byte string as one identity. This is not a TLA+ invariant: versioned decoder
tests plus the source-bound legacy-codec guard are the authoritative check.
Negative control `ML-MUT-WIRE-01` enables a retired fallback or mixed-layout
path; the static guard or exact decoder tests must then fail.

**Release gates.** `G-UNIT`, `G-SDK`, and `G-FINAL`.

## Release gate registry

The gates below remain **Evidence: Open** until all stated artifacts are fresh,
source-bound, archived, and unskipped.

### G-UNIT — focused unit and adversarial suites

**Evidence:** Open.

Every modified production function has a focused positive test and at least one
negative test. The combined matrix covers source/entrypoint/session drift,
restart equivocation, height jumps, wrong predecessors, stale incarnations,
ABA, zero/partial/duplicate/4,097-source groups, forged QCs/manifests/proofs,
tampered results, body eviction, corrupt/truncated/oversized latest pointers,
configured retained-count overflow, aggregate-byte addition/budget overflow,
malformed/conflicting/oversized publication temporaries, legacy dense evidence,
symlinks/hardlinks, every Native prune stage (temporary-only, stable before
unlink, each partial pair unlink, complete pair unlink, and stable plus
identical temporary), reservation duplication, base-state mismatch, bounded
fetches, and every persistence crash boundary. Tests that exercise only
`#[cfg(test)]` producer helpers do not close a live-path obligation.

The focused source inventory is now internally consistent. The nine arrays in
`scripts/run_sumeragi_v2_release_gates.sh` contain exactly 418 unique required
tests: 215 core, 140 queue-journal, 13 configuration, eight data-model, 39
Torii, one Torii-shared, and two integration. The runner and
`ci/check_sumeragi_v2_multilane_release_inventory.sh` both require that exact
418-row shape, including grouped Native prevote-budget rejection before
Kura/WSV mutation, exact durable QueuePlan obligation authentication and route
accumulation, registry corruption checks, ApplyCarrier authorization, and the
retirement transition/lifecycle-fence lock order. The G-UNIT static inventory
checks establish exact `418/418` source consistency and also
source-binds the synchronized 52-control grouped corpus. On 2026-07-31, pinned
Rust 1.93.1 locked/offline
execution from isolated source `/tmp/iroha-kura-final3.dvOYAN` and isolated
target `/tmp/iroha-kura-target2.Llklru` passed 12 current-source focused tests:
six bounded historical namespace/accounting cases, two startup-replay binding
cases, and one each for B/A/B historical recovery, first-merge crash-window
repair, Native post-WSV retention, and authenticated geometry refresh. The
isolated checkpoint also passed `cargo check -p iroha_core --lib`; the
post-reader-refactor reruns covered startup binding and B/A/B recovery. Earlier
same-day isolated slices passed the 18 Kura replica tests and four
configuration tests. These are focused partial results, not a fresh archived
execution of all 418 required tests, so `G-UNIT` stays Open.

### G-FORMAL — source-bound models and expected mutations

**Evidence:** Open.

Port current production behavior for autoscale, Native application,
autonomous reservation/carrier ownership, QueuePlan admission, and Kura
replica retention into
`formal/sumeragi_v2`. Positive TLC and Apalache models must pass the TLA+
invariants named by their five kernel rows. Every conceptual `ML-MUT-*` case is
classified and machine-mapped in `multilane_source_bindings.json`: a
`tla_counterexample` case must own its exact non-empty `_bug.cfg` set and
produce each named TLC counterexample, while a `static_release` or
`differential_release` case must own no TLA mutation config and is enforced by
its exact source-bound release-check contract. The structural mapping checker
is non-Cargo; execution of Rust unit and cross-SDK checks remains in `G-UNIT`
and `G-SDK`. CI must reject missing, duplicate, or reassigned mappings and
source hash drift, then archive model, configuration, tool-version, result,
and source hashes. Existing generic Sumeragi models are not substitutes for
these multilane models.

The current schema-5 binding registry contains 30 conceptual rows: 25 TLA
counterexample rows with 73 exact mutation configurations, two static-release
rows, and three differential-release rows. Its Native startup bindings name
`LaneApplicationEvidenceRepairSummary::publication_count`, explicit ordinary
pair repairs, read-only certified frontier/artifact access, reverse merge-
carrier preflight/application, and the Queue-gated runner ordering. Those
structural bindings execute no model checker. The fifth authenticated Kura
retention kernel and its conceptual mutation mapping are now structurally
present. Its direct-ingress and settled Kura authority/eviction bindings are
checked, and configured checked registry capacity is bound through the
overflow-checked key/entry helpers and Kura initialization. The exact durable
tip/source token, complete-source revalidation, configured evictable-first
window, eight-probe/one-fanout owner turn, retained retry, scan-start interval
anchor, active-window-bounded rollover wake-up after durable handoff,
durable-tip update, guarded publication, and process-lifetime runner ownership
are all bound; the Kura retention contract has no pending production symbol. The source binding for
`validate_v2_finality_wire_bindings` follows
its split production owner at
`crates/iroha_core/src/kura/retained_finality_replica_authority.rs`, instead of
the former `kura.rs` location. Static validation of this registry confirms
only structural source binding; it is neither a TLC/Apalache result nor a
fresh Rust or network execution result.

The separate in-flight carrier contract has a composed state/action relation
but remains release-classified as layout-only until production trace
extraction exists. Its twenty-two exact TLC mutation witnesses and sixth positive
Apalache row are mandatory release evidence after the five refinement rows,
but they do not promote the missing extraction theorem.

A 2026-07-31 non-Cargo structural checkpoint taken immediately before this
self-documenting ledger update reported source manifest
`01754c06f060330a30cddd03c48203a55909832dfcb8475aee3f5b95651a4d5c`
validated schema 5, all 30 conceptual rows, all 25 TLA counterexample rows and
73 exact mutation configurations, the two static and three differential rows,
the five refinement kernels plus the separately classified layout-only row,
and an empty Kura pending-source list. This checkpoint did not run TLC,
Apalache, Cargo/Rust tests, SDK harnesses, or any network corridor and is not a
release execution receipt. The isolated Python source-contract suite passed all
13 Kura cases, including the TTL floor, owner-minimum, scan-start deadline, and
durable-handoff wake-up negative controls; those are structural checks, not
Rust execution evidence. The fresh aggregate structural/model-source suite
passed `100/100`, the proof-ledger release-inventory subset passed `54/54`, and
the static Apalache runner contract passed all 14 fail-closed controls. None
executes TLC or Apalache.

A 2026-07-24 source-bound checkpoint for source manifest
`af1361d00f08bbf340c57e6b4992c0a8166a7e9e67f9f4c5771827ce5c69e7a6`
passed direct pinned TLC positives (autoscale `14/14`; Native `1,121`
generated/`304` distinct; autonomous `294` generated/`169` distinct), all
`27/27` named mutation witnesses, all three Apalache v0.52.2 typecheck/positive
bounds, and `8/8` runner negative controls. The three-result Apalache evidence
TSV SHA-256 is
`1e972edae1804b9996fdeeadd4f89df6f9dd2f2bd27756e6eba6310b7fbfe92f`.
This is supporting bounded evidence, not gate closure: the checksum-pinned
TLAPM standard library was installed for the rerun, but no clean aggregate
release receipt was archived, and bounded checks are not deductive proof
evidence.

### G-4P — four-peer DA/RBC lifecycle suites

**Evidence:** Open.

Fresh four-peer suites must cover automatic expansion/contraction, restart
recovery, same-ID A/B/A recreation, grouped and mixed-role Native application,
same-route handling, offline/Byzantine validator rotation, body pruning,
evidence-aware drain, archive, and useful autonomous execution through the
canonical carrier. Body-pruning cases must establish the `ML-KURA-01`
deterministic keeper set, authenticated direct adverts, local-keeper pinning,
and recovery from every advertised keeper; tuple-count or test-only advert
injection is insufficient. Required anchors include
`nexus_autoscale_certified_merge_recovers_missing_sidecar_after_restart`,
`nexus_autoscale_two_phase_drain_closes_certifies_then_retires_after_restart`,
and the strict autoscale cycle tests in
`integration_tests/tests/nexus/autoscale_localnet.rs`, after their production
prerequisites are reachable. A skipped test or test-only producer is a failure.
The mandatory four-peer lifecycle and rotating-validator Native tests are now
non-ignored and source-bound into the release runner, but no fresh completion
artifact is recorded here.

### G-12P — twelve-peer multilane corridor

**Evidence:** Open.

Run at least three independent four-validator dataspaces with grouped DvP and
autonomous work, rotating outage/restart, scale-out, drain, scale-in, and
same-ID recreation. The corridor must also rotate deterministic Kura keepers,
expire and refresh adverts within configured bounds, pin local keepers, and
recover an evicted canonical body from authenticated advertised sources.
Require 10/10 fresh deterministic seeds and a two-hour
fault soak, full peer convergence, durable participant receipts, and zero lost,
rejected-after-acceptance, or duplicate transactions. The existing
`cross_dataspace_atomic_swap_is_all_or_nothing` corridor is a starting point:
the former payload-recovery-to-canonical-application stall has a source-side
implementation correction and bounded diagnostics, but only the strict
10/10-seed corridor plus two-hour fault soak can establish this gate.

### G-SCALE — one-lane versus four-lane scaling proof

**Evidence:** Open.

On pinned hardware and a pinned software/configuration revision, run five
paired one-active-lane versus four-active-lane trials. Four lanes must achieve
at least 1.5 times median committed throughput and p95 latency no worse than
1.25 times the one-lane result at matched offered load. Archive raw samples,
warmup policy, queue depths, offered/accepted/committed counts, CPU/memory/disk
limits, lane/index/disk maxima, hardware identity, configuration, and source
hash. No current benchmark artifact closes this gate.

### G-SDK — cross-SDK diagnostics and Native V2 parity

**Evidence:** Open.

Run Rust/OpenAPI, both Python surfaces, source and distribution JavaScript,
Swift, Kotlin core-jvm, and mirrored Java suites against the same Rust-owned
grouped corpus. Archive the corpus hash and per-SDK results. No SDK may skip a
negative or substitute a hand-authored fixture.

The Rust generator, checked-in protocol-4 grouped corpus, wire TSV, harness
count, and static release inventory are now synchronized. The grouped corpus
contains exactly 52 negative controls and hashes to
`0fb9bf6a490f4974e65a5a03985bfe75321e3de1f54be064c3b088ccffc061d1`;
the source-suite manifest hashes to
`5cf86f7b08fdcb1bb95d144548e55efb18daa81f9194d8b9dd599313f7fc6d39`.
Fresh direct results pass OpenAPI `7/7`, Python `58/58`, JavaScript grouped
parity `56/56`, and the separate JavaScript status/diagnostics contract
`44/44`. The exact no-skip diagnostics release inventories are Rust `14`,
Python `114`, JavaScript source/distribution `88`, Swift `17`, Kotlin `15`, and
Java `10`; their suite-source manifest SHA-256 is
`712bec0bf752ed650346c7588963d7e77117120c20f1c926e69f6ce21c3677b7`.
These are partial results, not the complete source-and-distribution
release harness: no Rust, Swift, Kotlin, or Java parity suite and no full
aggregate harness replay was run for this evidence refresh. `G-SDK` therefore
stays Open despite the resolved generation drift and passing direct subsets.

### G-FINAL — clean release validation

**Evidence:** Open.

Before every Cargo invocation, record
`ps -axo pid,etime,command` and wait while Cargo or rustc is active. Use the
isolated target directory and `--locked --offline`. Run focused crate tests,
SDK parity suites, formal runners, `cargo build --workspace`, full
`cargo test --workspace`, strict workspace Clippy, formatting check, and
`scripts/check_no_legacy_codec.sh`. The release record must contain commands,
exit status, source revision, target directory, toolchain, and artifact hashes.
`scripts/run_sumeragi_v2_release_gates.sh` must require the completed multilane
focused, SDK, formal, four-peer, twelve-peer, and scaling gates and must fail on
a skipped required test.
Only then may architecture/operator documentation, `status.md`, or
`roadmap.md` describe a row or gate as closed.

## TODO classification

This section is the disposition record for explicit source TODOs found by the
multilane audit. A newly discovered TODO touching lane routing, autoscale,
merge, reservation ownership, Native AMX, drain, retirement, or multilane
diagnostics must be added here or mapped to a ledger row before release.

### Resolved in source; release evidence remains open

- The former `V2LaneWorkAdapter::refresh_merge_candidates` TODO and
  `execution_batch.is_none()` production exclusion are absent.
  `schedule_autonomous_lane_production`, exact reservation propagation,
  certified-bundle durability, execution-bearing merge-candidate synthesis,
  canonical re-execution/application, and restart ownership reconciliation now
  provide the reachable production path mapped to `ML-AUT-01` through
  `ML-AUT-06`.
- The former first-release autonomous and drain limitations are retired in the
  canonical English architecture/operator documents. Historical entries in
  `status.md` and `roadmap.md` remain historical and are not evidence.
- The former grouped JSON/general wire TSV drift is resolved: fresh Rust
  generation matches both checked-in protocol-4 artifacts byte-for-byte, the
  grouped harness and release inventory require 52 negatives, and
  `ML-API-04` is implemented. Complete cross-SDK evidence remains open under
  `G-SDK`.
- A fresh source scan found no remaining in-scope TODO in lane routing,
  autoscale, merge, reservation ownership, Native AMX, drain, retirement, or
  multilane diagnostics. Any newly introduced marker must be classified here
  before release.

### Unresolved in scope without a TODO marker

- The former authenticated Kura V1 producer/configuration and wire-consumer
  implementation gaps are resolved and source-bound. Their focused Rust,
  formal-engine, SDK, and multi-peer execution receipts remain open; structural
  source validation alone cannot close those gates.
- `G-UNIT`, `G-SDK`, and `G-FORMAL` remain open even though the exact 418-row
  inventory, 52-control corpus, partial direct SDK subsets, and structural
  formal checker now pass. Focused Rust subsets now pass, but no complete
  418-test execution, complete SDK harness, or formal-engine result was
  produced by this refresh, and no static source list or focused slice is a
  release execution receipt.

### Explicitly out of scope

- **Generic finalized-view archive retention:** the TODO beside
  `provider_ingest_finalized_archive` in
  `crates/iroha_core/src/sumeragi/v2_apply.rs` concerns a governed
  indefinite-retention/deployment policy for optional provider-ingest and
  reputation finalized-view archives. Those archives consume the same
  authenticated commit receipt but do not own lane reservations, merge
  carriers, Native participant receipts, autoscale frontiers, drain
  certificates, or lane-retirement artifacts. Their bounded fail-stop ceiling
  is a separate operator archive-policy obligation and is outside this
  multilane ledger.
- **Generic block scheduling:** the TODO on
  `PreparedBlockExecution::LiveBatch` in `crates/iroha_core/src/block.rs`
  concerns routing classified mixed batches through the ordinary quarantine
  scheduler while preserving its live-state barrier. It does not own a lane
  reservation, autonomous lane QC, merge carrier, Native participant
  application, autoscale frontier, or retirement decision. It is therefore
  outside this multilane ledger.
- **Fee sponsorship:** the TODO in
  `ensure_global_fee_sponsor_asset` in
  `crates/iroha_core/src/smartcontracts/isi/world.rs` concerns propagating
  `DataspaceRestricted` balance scope through sponsor vault keys, queue
  reservations, relay leases, and settlement accounting. It changes the
  economic fee-sponsor policy, not the safety or durability of multilane
  reservation/carrier ownership. It is therefore outside this ledger.
- **Authority-paid receipt settlement:** the TODO on
  `reject_authority_lane_relay_burn_fee` in
  `crates/iroha_core/src/executor.rs` concerns adding a proof-bound authority
  spend lease to the fee subsystem. Although a future lease would be consumed
  by admission, reservations, execution, and merge settlement, authority
  payment is not an admitted first-release receipt-settlement mode: the
  supported exact-sponsor path fails closed before receipt creation or balance
  mutation, as pinned by
  `receipt_settled_quote_rejects_authority_payer_with_sponsor_remediation` and
  `receipt_settled_execution_rejects_authority_before_recording_receipt`. This
  is deferred fee-policy functionality, not a gap in multilane reservation or
  carrier ownership, and is outside this ledger.
- **Generic Sumeragi equivocation penalties:** the TODO in the
  `reducer::Effect::ReportEquivocation` adapter branch in
  `crates/iroha_core/src/sumeragi/v2.rs` concerns retaining a complete
  conflicting generic consensus-message pair before enabling penalties. It is
  not specific to lane/autoscale/merge/Native AMX behavior and remains in the
  generic consensus proof ledger.
- **Generic P2P exact-writer deadline corridor:** the TODO above
  `full_exact_writer_queue_times_out_closes_route_and_releases_actor_budget`
  in `crates/iroha_p2p/src/network.rs` requests a four-peer socket-level test
  for a Byzantine peer that keeps its inbound timer alive while refusing to
  read outbound bytes. It tests connection-actor isolation for all P2P
  payloads; it does not define Kura keeper authority, authenticate a lane
  artifact, own a reservation, or decide drain/retirement. `G-4P` still must
  exercise multilane Byzantine traffic, but this generic transport test is not
  a multilane implementation row.
- **Generic reputation finalized-view index scaling:** the TODO on
  `ReputationFinalizedVirtualBaseCheckpointV1::journal_prefix_source_heads` in
  `crates/iroha_core/src/query/reputation_finalized.rs` concerns replacing a
  bounded inline reputation-source snapshot with content-addressed sharded or
  Merkle indexing before an unrelated source population can become
  unbounded. It neither retains canonical execution bodies nor participates
  in Native, merge, reservation, autoscale, drain, or lane-archive authority.
- **Generic Decision fetch refinement:** the TODO in
  `SumeragiV2CertifiedRequestHashAuthorityProofs.tla` concerns the concrete
  `ExecuteDecisionFetch` trace mapping for generic historical Decision body
  recovery. The corresponding debt is already owned by
  `ProgressWitnessProductionRefinementObligation`; it does not model lane
  reservations, autonomous merge carriers, Native participant evidence,
  autoscale, drain, or retirement and is therefore outside this multilane
  closure ledger.
- **Other generic Sumeragi liveness composition:** the remaining 12 TODOs in
  `formal/sumeragi_v2/` are five locked-body reproposal provider/composition
  obligations, one finite-producer descent proof, one adequate-leader corridor
  deadline service property, two asynchronous finite-producer route/rank
  obligations, and three finite-producer production/descent/source-isolation
  obligations. They concern generic proposal/timeout/producer temporal
  liveness, not the multilane kernels in
  `multilane_source_bindings.json`. They remain proof debt, but cannot stand in
  for or block the lane-specific safety/evidence rows.
- **Generic causal-scheduler projection and liveness:** the TODO on
  `production_fresh_causal_successors` in
  `crates/iroha_sumeragi_core/src/verus_proofs.rs`, mirrored in
  `crates/iroha_sumeragi_core/VERIFICATION.md`, concerns the machine-checked
  production effect-to-TLA candidate identity/ownership mapping and
  Completion-capacity product-rank proof needed for temporal liveness. It is a
  generic scheduler-refinement obligation, already recorded as
  `specified_unproved` in `formal/sumeragi_v2/PROOF.md`; it does not model
  lane reservations, autonomous merge carriers, Native participant evidence,
  autoscale, drain, or retirement and is outside this multilane ledger.
- **Unrelated source markers:** the TODOs in
  `crates/iroha_core/src/privacy_profiles.rs`, the Kagemusha SHA-256 table
  implementation, and `crates/ivm/build.rs` concern privacy-engine activation,
  an optional spread-word representation, and reproducible CUDA PTX artifacts.
  None consumes a multilane identity or evidence class. They account for the
  three non-consensus source TODO comments in the audit and are outside this
  ledger.
- **Wallet query roadmap items:** the two active TODO bullets in `roadmap.md`
  request an authenticated account-activity feed and a timestamp-bounded
  outgoing-value aggregate. They are query/index product work and do not
  participate in consensus ownership or lane lifecycle.

### Intentional first-release rejections

These are current fail-closed policy, not hidden TODOs. Their negative tests
remain required by the mapped rows.

- QueuePlan, reservation journal V5/key V2/FIFO/release layouts, Native signing
  claims, lane executable/QC/NewView artifacts, merge entries, execution
  contexts, application evidence, and `KuraReplicaAdvertV1` reject retired,
  zero, or future versions. This is the coordinated clean break mapped to
  `ML-WIRE-01`, not authorization to accept legacy bytes.
- `require_validator_storage_platform` and Kura geometry recovery reject a
  voting/retirement role when the host cannot provide the first-release
  Linux/macOS crash-safe storage contract. This is a declared platform
  restriction mapped to `ML-LIFE-01`, `ML-LIFE-04`, and `G-FINAL`; it must not
  be silently bypassed.
- `ConfigError::NexusMultilaneDisabled` rejects lane catalogs, dataspaces, or
  routing overrides when `nexus.enabled` is false. That preserves the
  configuration-only production boundary in `ML-LIFE-01`; it is not an
  environment-controlled autonomous-execution toggle.
- Queue durability ambiguity disables admission/selection until bounded
  restart recovery, and Kura retirement rejects obsolete, unexpected,
  malformed, temporary, hardlinked, or symlinked evidence. Those rejections
  implement `ML-AUT-06`, `ML-NAT-07`, and `ML-LIFE-04`; weakening them would
  reopen loss, ABA, or unsafe-retirement traces.
- The logging-only first-release equivocation limitation and the two fee-policy
  rejections are the explicitly out-of-scope TODOs above. They do not weaken
  any admitted multilane reservation/carrier path.

Out-of-scope classification means these TODOs do not block multilane closure.
It does not mark them implemented, safe for removal, or release-evidenced.

## Closure procedure

A row advances from **Open** only when its production symbols, focused tests,
formal invariant and mutation, and referenced release gates agree on the same
versioned behavior. If code changes invalidate an artifact hash or production
symbol binding, its evidence returns to **Open**.

Milestone ordering remains part of closure. The source implementation now
places Native evidence hardening before execution-bearing autonomous candidate
synthesis and places exact ownership plus evidence-aware drain before automatic
destruction. No feature or environment switch bypasses those boundaries.

The release is multilane-complete only when:

- every `ML-*` row has **Closure: Implemented**;
- every referenced `G-*` gate has **Evidence: Evidenced**;
- the former in-scope TODO remains resolved by a reachable production path,
  not merely by deleting its text;
- no newly discovered in-scope TODO lacks a ledger disposition; and
- `roadmap.md`, `status.md`, architecture, and operator documentation are
  updated from the same fresh archived evidence.
