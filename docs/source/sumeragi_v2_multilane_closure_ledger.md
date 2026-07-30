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
artifact-aware retirement; source presence and focused fixtures do not prove
the required real-network corridors, fault soak, scaling target, SDK parity, or
clean full-workspace release gate.

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

**Implementation:** In flight; V3 payload/execution-input symbols are not yet
total-source-projected into a formal semantics.
**Closure:** Open.
**Evidence:** Open.

`SumeragiV2InFlightFirstRelease.tla` is a finite three-validator safety model
for `LaneExecutablePayloadV3` carrying an exact
`QueuePlanAdmissionBindingV2` preimage. Its fixed and mutation configurations
cover producer-selected versus replicated-carrier ownership, QueuePlan V5
`PutBatch` then V9 reservation fsync then Kura Active then execution-input
durability then READY, missing/late bodies, producer death after fanout,
crash-prefix durable recovery, exact Commit/Release scope, duplicate carrier
application, conflicting/ABA bindings, and the 4096 entry limit.

This row is deliberately **not** a production-refinement claim. TLC exhausts
the stated finite model and Apalache typechecks/bounds its abstract actions;
neither checker proves that Rust filesystem/restart traces refine those
actions. The open theorem is a total Rust pre/post-state forward simulation
and reverse terminal-owner projection over QueuePlan V5, reservation V9,
Kura, recovery, Commit, and Release. Token-order/source-presence checks are
insufficient and must not promote this row or a release status. See
`docs/formal/sumeragi_v2/INFLIGHT_FIRST_RELEASE_EVIDENCE.md`.

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
evidence, while `crates/iroha_core/src/sumeragi/v2_apply.rs` invokes
persistence and idempotent startup/replay repair in durability order. Pruned
bodies remain verifiable through QC-authenticated manifest evidence; weaker
hash-only evidence remains fail-closed. The Kura namespace contains one
immutable versioned manifest file and one immutable versioned receipt file per
participant height, followed by a descriptor-bound, replaceable exact-latest
pointer. Publication uses create-new temporaries, no-clobber promotion, file
and directory durability sync, and exact readback.

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

**Formal obligation and mutation.** Invariant
`MLNativeDurabilityPrecedesFrontier` orders durable finality, immutable
manifest, immutable receipt, exact-latest pointer, and replicated frontier.
Mutation `ML-MUT-NAT-06` reorders any two boundaries or drops idempotent
repair; the model must expose an unverifiable frontier or lost durable
application.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-FINAL`.

### ML-NAT-07 — bounded standalone evidence and exact-latest pointer

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** Kura persists immutable versioned manifest and receipt
files keyed by participant height. A separate route/incarnation-bound,
descriptor-bound latest pointer is replaceable derived state used for bounded
exact lookup and explicitly reconstructed through
`Kura::rebuild_native_amx_participant_receipt_latest_indexes_on_startup`.
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
`State::stage_queue_plan_admissions` owns the immutable global CAS.
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
CAS; a conflicting binding must fail closed and can never execute.

**Focused and adversarial tests.** Cover split-route public acceptance,
execution before global CAS, two conflicting CAS attempts, restart ABA, local
TTL expiry, deferred ingress, cancellation, guard drop, missing or mismatched
binding material, and duplicate execution. Inject every crash boundary between
certificate persistence, wakeup, WSV publication, queue reservation, carrier
application, and authenticated loser cleanup.

**Formal obligation and mutation.** The bounded
`SumeragiV2QueuePlanAdmissionRegistry` kernel checks
`MLAdmissionCasUnique`, `MLCertificateDurable`, `MLPublic202Exact`,
`MLExecutionRequiresExactBinding`, `MLQueueEligibilityExact`,
`MLAdmissionAtMostOnceExecution`, `MLImmutableAdmissionTombstone`, and
`MLCancellationStopsExecution`. Conceptual mutation `ML-MUT-QUEUE-01` maps
only to the ten QueuePlan `_bug.cfg` controls recorded in
`multilane_source_bindings.json`; each must produce its exact named TLC
counterexample. The positive Apalache run checks the fixed kernel only and is
not a mutation runner or deductive proof.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-FINAL`.

## Autonomous lane execution closure

### ML-AUT-01 — durable FIFO queue reservation

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** `LaneQueueReservationKeyV2`,
`LaneQueueReservationStore`, `Queue::reserve_transactions_for_lane`,
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
batch and fsyncs one exact `LaneQueueReservationKeyV2` record before ownership
leaves the ordinary queue. Selection binds active route/incarnation and
canonical enqueue order, and requires the admission binding to be an exact
immutable WSV registry match. No transaction can be visible to both owners or
to neither owner at any crash point.

**Focused and adversarial tests.** Cover empty and bounded batches, FIFO order,
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

**Production map.** Startup calls
`reconcile_lane_reservation_ownership` from
`crates/iroha_core/src/sumeragi/v2_apply.rs` through
`crates/iroha_core/src/sumeragi/v2_runner.rs`.
`Kura::autonomous_lane_payload_matches_reservation` performs the production
current-incarnation, exact-reservation, payload/certification/application
evidence check. Reconciliation retains one authenticated owner, releases
orphans in original enqueue order, resumes terminal Commit/ForgetCommit, and
prunes only forgotten reservations.

**Closure condition.** Replace the production false stub with exact,
bounded, authenticated ownership reconciliation. Retain only reservations
matched by current-incarnation durable payload/certified-bundle/global
application state. Release every other reservation exactly once in original
enqueue order. Prune only terminal forgotten reservations.

**Focused and adversarial tests.** Restart after queue reserve, payload fsync,
availability QC, Prepare QC, Commit QC, certified-bundle fsync, merge-QC
certification, Kura commit, WSV commit, reservation Commit, and ForgetCommit.
Cover stale-incarnation sidecars, duplicate reservation files, ambiguous
evidence, corrupt/oversized artifacts, and repeated startup.

**Formal obligation and mutation.** Invariant `MLRestartOwnershipPartition`
states that startup reconstruction preserves the single-owner partition.
Mutation `ML-MUT-AUT-06` treats all or no payloads as owners, ignores
incarnation, or performs non-idempotent release; the model must expose loss,
duplication, or ABA retention.

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
**Closure:** Open.
**Evidence:** Open.

**Production map.**
`crates/iroha_data_model/src/bin/sumeragi_v2_wire_fixtures.rs` and its
`native_amx_grouped` module generate
`fixtures/sumeragi_v2/native_amx_v2_grouped.json`, including grouped golden
data, application evidence, and 50 negative controls.
`NativeAmxAttestationBodyV2::computed_grouped_participant_settlement` is the
explicit production builder; single-source construction is labelled as a test
fixture.
`ci/run_native_amx_v2_grouped_sdk_parity.sh` source-binds the exact fixture and
OpenAPI, Python, JavaScript source/distribution, Swift, Kotlin, and Java
consumers. Fresh standalone runs passed OpenAPI `4/4`, JavaScript `37/37`,
Kotlin `6/6`, Java `5/5`, and the Python 3.12 harness `35/35`. Closure remains
open until a rebuilt ABI-21 Swift bridge passes the same corpus and one release
gate archives every language consumer together.

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

The release runner now inventories 277 exact, non-ignored multilane focus
tests: 101 core multilane tests, 119 core queue-journal tests, seven
configuration tests, eight in `iroha_data_model`, 39 in Torii, one in
Torii-shared, and two in the integration support library. That source
inventory is not a passing test transcript; the full focused rerun and
archived receipt remain required.

### G-FORMAL — source-bound models and expected mutations

**Evidence:** Open.

Port current production behavior for autoscale, Native application,
autonomous reservation/carrier ownership, and QueuePlan admission into
`docs/formal/sumeragi_v2`. Positive TLC and Apalache models must pass the TLA+
invariants named by their four kernel rows. Every conceptual `ML-MUT-*` case is
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
canonical carrier. Required anchors include
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
same-ID recreation. Require 10/10 fresh deterministic seeds and a two-hour
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

The Rust generator, 50-control grouped corpus, six-surface parity harness, and
fixture/suite source-hash binding are present. Standalone OpenAPI, JavaScript,
Kotlin, Java, and Python runs are fresh and passing; Swift remains open because
the materialized XCFramework is ABI 19 and must be rebuilt at ABI 21 after the
atomic-publication fix. One archived release replay of every required surface
is not yet recorded, so neither `ML-API-04` closure nor this gate advances.

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
- A fresh source scan found no remaining in-scope TODO in lane routing,
  autoscale, merge, reservation ownership, Native AMX, drain, retirement, or
  multilane diagnostics. Any newly introduced marker must be classified here
  before release.

### Explicitly out of scope

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
- **Generic Decision fetch refinement:** the TODO in
  `SumeragiV2CertifiedRequestHashAuthorityProofs.tla` concerns the concrete
  `ExecuteDecisionFetch` trace mapping for generic historical Decision body
  recovery. The corresponding debt is already owned by
  `ProgressWitnessProductionRefinementObligation`; it does not model lane
  reservations, autonomous merge carriers, Native participant evidence,
  autoscale, drain, or retirement and is therefore outside this multilane
  closure ledger.
- **Generic causal-scheduler projection and liveness:** the TODO on
  `production_fresh_causal_successors` in
  `crates/iroha_sumeragi_core/src/verus_proofs.rs`, mirrored in
  `crates/iroha_sumeragi_core/VERIFICATION.md`, concerns the machine-checked
  production effect-to-TLA candidate identity/ownership mapping and
  Completion-capacity product-rank proof needed for temporal liveness. It is a
  generic scheduler-refinement obligation, already recorded as
  `specified_unproved` in `docs/formal/sumeragi_v2/PROOF.md`; it does not model
  lane reservations, autonomous merge carriers, Native participant evidence,
  autoscale, drain, or retirement and is outside this multilane ledger.

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
