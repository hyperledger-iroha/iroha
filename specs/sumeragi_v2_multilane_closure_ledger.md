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
from a release-evidenced end-to-end path. The mutable development source
contains autonomous production, Native application evidence, evidence-aware
drain, artifact-aware retirement, Rust-owned protocol-4 generators, and
authenticated Kura replica outbound/configuration. Checked-in generated
artifacts are not assumed current until they are regenerated from an immutable
candidate. Source presence and static inventory consistency do not prove the
required real-network corridors, fault soak, scaling target, complete SDK
parity, formal obligations, or clean full-workspace release gate.

## Status rules

Every in-scope row records three independent states:

- **Implementation: Open** means a required production path is absent,
  test-only, disabled, internally inconsistent, or still being changed.
- **Implementation: Implemented** means the cited production symbols implement
  the row's primitive. It does not imply that the wider milestone is complete.
- **Closure: Open** means at least one implementation, integration, or release
  obligation in the row remains.
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

Unless explicitly labelled as a private-index audit, all counts and static pass
statements in this document describe the mutable development checkout. They are
inventories and source-consistency observations, not immutable-candidate
execution or release receipts.

## 2026-08-19 mutable-development closure snapshot

- The checkout remains an unsigned, dirty mutable-development tree. Its
  volatile index/worktree counts are deliberately not treated as release
  identity; only a later signed, detached, source-sealed candidate may supply
  that evidence.
- The bound static inventory contains exactly 84 corridor legs, 864/864
  production tests across 43 modules, 522/522 G-UNIT rows, and four mandatory
  G-4P gates. The grouped fixture pin validates. The recursive SDK resolver
  enumerates 1,451 grouped and 1,453 diagnostics paths at the exact hashes in
  the owning corpus and `G-SDK` rows below. These are source inventories, not
  execution receipts.
- The reviewed closure has no remaining explicit in-scope multilane TODO. The
  crate-internal lifecycle owner covers durable selection, launch, completion,
  finalization, cleanup, restart, and exact successor handoff. Both reproduced
  clean-`d24` failures are closed in current source: QueuePlan acceptance is
  locally certificate-durable before later proposal-native global application,
  and merge-ledger epoch remains independent of membership epoch. Three focused regressions pass, and the distinct bounded Kura-backed
  tag-21 leader handoff is independently audited. A fresh exact four-peer run
  against this merge is still absent.
- The macOS framework-Python archive path now authenticates and bounds its
  Mach-O inputs and Apple tools, rewrites exactly two framework dependencies,
  ad-hoc signs and verifies the derived binaries, and carries exact source,
  derived, mode, dependency, tool, and inventory evidence through bootstrap,
  validation, receipts, and the outer runner. The 17 relocation cases, seven
  validator cases, two receipt cases, exact bootstrap launch, and general
  runtime populate/verify path pass on frozen bytes; an independent audit is
  clear. These focused local results are not a signed release archive.
- The schema-5 formal registry still names all 27 production actions, and the
  structural production-trace extraction reports no open action name. The
  checked-in revision-4 proof ledger declares 44 `tlaps_proved`,
  3 `cross_tool_proved`, 0 `specified_unproved`, 6 `trusted_contract`, and 1
  `out_of_scope`, with `machine_checked_completion: true`. Those declarations
  are release-proof inputs, not deductive revision-4 evidence. No current full
  Cargo, TLC, Apalache, TLAPS, Verus, network, trace-replay, cross-tool
  certificate, or immutable-candidate result is claimed.
- The source-file gate checks 7,990 paths with 193 exact exceptions and
  4,813,796 Rust lines, with 48 findings. The configuration retains the
  5,067,263-line baseline, 5,014,603-line ratchet, 4,540,000 hard ceiling, and
  4,500,000 working target. Ratchet headroom is 201,082 lines, but the hard
  objective remains open by 273,796 lines and the working target by 313,796
  lines. Worktree, combined-`HEAD`, and cached-only diff checks pass.
- Three companion sources remain untracked: the privacy exact-12 fixture binary
  and the C# and Java typed transaction-admission intent types. The Java path is
  declared in the exact SDK source closure, so its tracked-source gate remains
  blocked until the authorized staging actor includes it. The five checked-in
  OpenAPI artifacts remain dirty and unsigned, and immutable-candidate SDK and
  wire evidence remains Open.

Every Evidence field and every `G-*` execution or release gate therefore
remains Open. These source-consistency documentation updates do not promote the
mutable development checkout or replace immutable-candidate evidence.

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
9. **Coordinated first-release wire.** Every Norito persistence and wire layout
   advertises its explicit canonical version. Consensus accepts exactly that
   layout.

## In-flight first-release formal boundary

**Implementation:** Current first-release layouts are source-bound. The mutable
source structurally binds the complete 27-action first-release inventory,
including the reservation-journal primitive seam. The
fixed-width composed transition relation is implemented and source-bound. The
autonomous FIFO
selector consumes checked
`SelectQueuePlanV1Conjunction` and `FsyncReservationV1` projections derived
from the canonical slot authority and complete ordered reservation-group
identity. Queue then revalidates the live V1 group and retains an exact
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
locks immediately before the durable release append. A retired non-producer
replica may use that direct-release projection only after the complete durable
ReleasePending prefix and an exact signed actor/producer distinction. Queue
then proves the ordered group is already FIFO-only without manufacturing the
producer's reservation owner and retains the move-only exact-hash fence while
Kura advances the Released prefix and through terminal Queue evidence. The first
FIFO proof remains a state-changing `ComposedNext`; an unchanged retry from an
already `DirectReleased` state is accepted only as
`ReleaseReservationDirectProofStutter`, with identical before/after states, and
cannot masquerade as a state-changing `Next` step. Existing bounded consumers
also recheck durable execution input/READY QC/lane Commit at merge
source admission. Canonical `ApplyCarrier` projections remain in a move-only
batch that V2 boxes as a `StateBlockCommitAuthorization`; State consumes the
exact block/merge/cardinality-bound batch under `state_commit_lock` and the
lifecycle fence when present, after the final scale-in Queue veto and before
geometry or transaction publication. V2 takes the Queue observer only for an
exact pending scale-in. The three ordered post-carrier Queue cleanup prefixes
remain separately checked. `RepairPostCarrierEvidence` is now extracted from
three production paths: live committed-block publication, State merge-journal
replay, and unified finalized reverse-carrier startup repair. All three derive
one move-only stutter token per autonomous lane from the finality-bound
`ApplyCarrier` post-state, bind the exact entry/carrier/reservation-group
identity, prove WSV application before minting, and consume the complete token
set before the first applicable Kura receipt or reverse-index publication. The
ungated receipt-repair helper is test-only. The structural production trace
extraction is complete for all 27 registered action names; this statement does
not attest runtime correspondence or provide a formal execution certificate.
The source-certificate contract now includes the already-implemented pre-Kura
direct release, producer fanout, authenticated late-body service, and
post-carrier repair linearization points. Fresh lane-committee producer fanout
effects consume a Queue-fenced, exact-slot authorization after Kura readback;
periodic payload retransmission first reconstructs and matches that same
canonical slot. Operational cache copies to global validators outside the lane
committee have no validator bit in this model and are not claimed as
`FanoutFromProducer` actions. Fresh autonomous late-body effects consume exact
certified Kura evidence. In both
paths effect preflight classifies an already queued identity as a stutter before
minting authorization. `Crash`, `Recover`, `RecoverReservationSnapshot`, and
`RehydrateLocalKuraCustody` now have named extraction seams through signed
lifecycle bootstrap, generation takeover, Queue snapshot recovery, local Kura
rehydration, drain-queue installation, and one-shot activation revalidation.
The registry's open-action tuple is empty. Those structural seams do not prove
that concrete startup replay preserves the composed abstraction; that remains
a formal execution and cross-tool obligation.
**Closure:** Open.
**Evidence:** Open. The schema-5 structural/source binding covers all 27 named
actions, but no fresh immutable-candidate TLC, Apalache, TLAPS, Verus,
trace-replay, mutation, or cross-tool receipt is recorded here.

`SumeragiV2InFlightFirstRelease.tla` is a finite three-validator safety model
for the accepted schema V1 carried by the production
`LaneExecutablePayloadV1` container and its exact
`QueuePlanAdmissionBindingV1` preimage. It establishes authenticated custody
for the selected producer without inferring knowledge by every validator, and
uses the canonical strict 3-of-3 count quorum for its fixed cardinality. Its
fixed and mutation configurations cover producer-selected versus
replicated-carrier ownership; a selected-batch
conjunction over individual QueuePlan journal V1 `Put` records; reservation journal V1,
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
stutter, and direct release is an explicit named action with separate ordinary
live/no-owner and strict retired non-producer FIFO-only branches. The latter
requires the complete ReleasePending prefix, preserves the producer's absent
Queue reservation ownership, and permits only that exact replica owner to
advance the Released prefix. The schema-bound V1 journal operation inventory
contains no lane-wide removal action.
The deterministic autonomous selection linearization point now derives its
move-only authority from the canonical slot committee/author, revalidates the
exact QueuePlan registry and FIFO selection, derives the complete ordered
reservation-group identity, and consumes checked QueuePlan-selection and V1
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
directory sync order. The source-binding contract maps these seams to the
complete 27-action model inventory. That structural mapping is not a
production trace-extraction certificate or theorem; fresh backend, mutation,
trace-replay, and cross-tool evidence remains required before this row can
close.
The durable merge-source, canonical WSV commit, and post-carrier Queue cleanup
consumers check their named projections against that same group identity. The
pre-Kura reservation-batch direct-release linearization point separately
consumes its composed projection after locked V1, FIFO, group, and committee
revalidation. The structural checker binds the remaining named production
linearization points and fails closed when any action loses its checked
producer or move-only consumer.

This row remains **evidence-open**. When run, TLC can exhaust the stated finite
model and Apalache can typecheck/bound its abstract actions; neither a static
source check nor a certificate generated without fresh backend transcripts can
close the row. Schema 5 of `multilane_source_bindings.json` classifies the
implemented mapping as
`composed_state_action_relation_with_source_bound_trace_extraction`: its exact
version/field/order/relation bindings detect drift, while the release gate must
still authenticate the current positive runs and every named mutation
counterexample. See
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
`NativeAmxSigningSlotV3`; any journal-layout mismatch fails closed.

**Closure condition.** A versioned durable source claim must bind the source
ID, typed transaction-entrypoint hash, plan digest, round context, authority
height, coordinator route/incarnation/planned height/view/proposal, and every
participant route/incarnation. Grouped slot claims remain a separate
anti-equivocation dimension. State restoration accepts only the canonical
journal layout and fails closed before mutation on any mismatch.

**Focused and adversarial tests.** Cover source, entrypoint, plan, epoch,
context, authority-height, coordinator route/incarnation/height/view/proposal,
and participant-set drift before and after restart. Cover a crash before record
publication, after record fsync, before anchor publication, after anchor
publication, truncated and oversized files, duplicate sequences, unexpected
files, symlinks, and a stale noncanonical journal.

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
and settlement. `State::native_amx_participant_frontier_markers` derives the
replicated frontier, which `StateBlock::stage_native_amx_participant_frontiers`
encodes only after the durable evidence token authenticates the same markers.

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
`merge_native_amx_application_sources`,
`canonical_native_amx_application_sources`, and
`from_result_bearing_block_and_merge_entry` in
`crates/iroha_core/src/sumeragi/exec.rs` project the canonical Native source set
from the ordinary block or its exact merge entry. The same staged entry is
bound through proposal validation, apply validation, State frontier projection
and staging, Kura replay, artifact budgeting, and publication. Before WSV
publication, Kura receives `state_block.staged_merge_entry()` and authenticates
the planned association against the block reference and finality when the
committed association is not yet published. Manifest grouping also retains one
height token for each `(lane, dataspace, incarnation)` route and rejects a
single application block that tries to assign more than one participant height
to that route.

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
The mutable source-bound inventory in
`pytests/scripts/sumeragi_v2_multilane_native_merge_manifest_test.py` collects
17 static controls: removed live prepublication, per-route height uniqueness,
planned-carrier-map collision rejection, Startup witness forwarding,
merge-before-Native repair ordering, the historical association corridor, and
eleven consumer relation-drift cases spanning selection, proposal/apply,
State, replay, Kura budgeting/publication, and evidence construction. They are
static source tests, not an executed G-UNIT or formal receipt.

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
When the latest pointer already equals the incoming plan, Kura reads both
stable members as optional repair inputs, rejects either present member when
its bytes differ, and separately reauthenticates the incoming manifest against
available finality. A missing manifest, receipt, or both therefore remains in
the bounded all-item preflight for create-new reconstruction; the guarded
publication path still requires the complete pair and exact readback before it
returns.
`Kura::native_amx_manifest_for_committed_block` accepts the exact planned merge
entry, validates it against the block reference and finality, rejects drift
from an already committed association, and fails closed when a merge-reference
block has neither association. The guarded Native evidence preflight carries
that same optional planned entry. `NativeAmxMergeAssociation::Live` requires
the exact staged witness at pre-WSV publication; `Startup` accepts only the
exact committed or planned association, while `CommittedOnly` cannot stand in
for either earlier boundary.

**Closure condition.** Persist finality plus the immutable per-height manifest
first, then the matching immutable per-height receipt and exact-latest pointer,
and only then publish replicated WSV frontiers. Retain the canonical executed
wire until the manifest and receipt are durable.
After body pruning, validate through the QC-authenticated manifest root and
proof. Hash-only legacy evidence stays fail-closed unless the exact canonical
wire is recovered from authenticated storage or QC signers. Startup must
idempotently repair a finalized marker missing either immutable evidence
member or its latest pointer
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
Startup separately builds `planned_merge_entries_by_carrier`, keyed by exact
carrier height and block hash, from the canonical finalized repair plan. Both
planning and apply preflights receive the planned association; the merge
carrier repair is applied before Native evidence publication. Thus live pre-WSV
publication and startup reconstruction use the same authenticated association
contract without depending on premature committed-index publication.

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
after generic authenticated carrier recovery, respectively. The last test's
current source also verifies exact finality/wire recovery, idempotent receipt
repair, and that a drifted State marker fails preflight without mutating the
repaired receipt.

The existing
`historical_autonomous_recovery_reaches_exactly_once_canonical_merge_application`
macro test now source-binds the complete production corridor: injected live
prepublication failure leaves WSV untouched, `Live(None)` fails,
`Live(Some(&entry))` authenticates the exact staged witness, Startup without an
association fails, Startup with the same planned entry succeeds, unified
repair applies the merge carrier before one Native carrier/route, retained
receipt bytes are identical, WSV is unchanged, and the second plan is empty.
At the crash image the manifest is absent while the structural receipt and
exact-latest pointer remain; the strict reader returns `None` until the
QC-authenticated carrier plan reconstructs the missing manifest and completes
exact readback.
The source-bound Python controls reject association or ordering drift. These
definitions remain mutable-development inventory; no Cargo execution or fresh
`G-UNIT` transcript is claimed, so Evidence remains Open.

**Formal obligation and mutation.** Invariant
`MLNativeDurabilityPrecedesFrontier` orders durable finality, immutable
manifest, immutable receipt, exact-latest pointer, and replicated frontier.
Mutation `ML-MUT-NAT-06` reorders any two boundaries or drops idempotent
repair; the model must expose an unverifiable frontier or lost durable
application. Its `multilane_native_prune_namespace_rebind_bug.cfg` control
models a pathname rebound after authentication and must violate
`MLNativePruneExactObjectRemoval` when an unlink is allowed to target anything
other than the authenticated open object. Its unified-startup controls are
`multilane_native_mutating_unified_startup_plan_bug.cfg`,
`multilane_native_uncoalesced_canonical_body_needs_bug.cfg`,
`multilane_native_partial_unified_startup_preflight_bug.cfg`,
`multilane_native_queue_before_evidence_readback_bug.cfg`,
`multilane_native_missing_reverse_merge_carrier_bug.cfg`,
`multilane_native_orphan_merge_carrier_bug.cfg`, and
`multilane_native_skip_post_cache_carrier_reconcile_bug.cfg`; each must violate
`MLUnifiedStartupEvidenceRepairSafe`.
The pointer-preserving missing-member repair relation is bound as a static Rust
source contract; this reconciliation does not claim that the finite TLA+
kernel proves Rust refinement for that crash image.

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
Historical same-day isolated Rust 1.93.1 locked/offline slices passed the 18
exact Kura replica tests and four exact configuration tests. The mutable
focused inventory still names those tests, but this reconciliation makes no
immutable-candidate execution claim for them or for a multi-peer body-pruning
corridor. Those focused runs and source anchors are not the complete 522-test `G-UNIT` receipt,
so this row's evidence remains Open.

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

**Production map.** `QueuePlanAdmissionBindingV1` and
`validate_queue_plan_admission_certificate_for_network_digest_v1` in
`crates/iroha_core/src/torii_proxy.rs` define the exact request, transaction,
routing-plan, context, enqueue-time, and journal-record identity certified by
the coordinator quorum. `persist_queue_plan_admission_certificate` and
`submit_signed_transaction_for_ingress_queue_plan_certified` in
`crates/iroha_torii/src/lib.rs` validate and persist that exact `f + 1`
certificate before returning public acceptance. Persistence is the acceptance
boundary: Torii then schedules best-effort authoritative dissemination and
attempts to wake the current Sumeragi owner, but neither wake delivery nor
later WSV application may downgrade the known durable `202` outcome.
`StateBlock::stage_queue_plan_admissions_for_carrier` owns the proposal-native
immutable global CAS and stages a
V1 pending obligation plus one exact member for each deduplicated
route/incarnation. The lexically ordered route-member key range is the
authoritative roster; its consensus cap is exactly
`MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK`, and no count/XOR summary is removal or drain
authority. Each member payload binds the route, exact-network digest, typed entrypoint,
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
`V2LaneWorkAdapter::reconcile_pending_queue_plan_admissions` admits autonomous ownership only
for the exact binding, preserve the durable tombstone across restart/TTL, and
reject or clean up a definitive conflict through the authenticated loser path.

**Closure condition.** One transaction/request identity may acquire at most one
global QueuePlan binding. A public `202 Accepted` requires the exact coordinator
quorum certificate to be durable on the ingress node; it does not claim that a
carrier has applied the binding to WSV. Dissemination is scheduled and the
current Sumeragi owner is woken after persistence, while startup replay remains
the fallback when no owner is attached or the wake is deferred. `Applied` is a
separate later state established only by canonical block application.
Autonomous reservation and execution require that exact immutable binding.
Restart, local expiry, cancellation, guard drop, or a deferred path must neither
erase the durable owner nor bypass the global CAS; a conflicting binding must
fail closed and can never execute. Every
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
the block/proposal admission roster cap, compact canonical member validation plus exact
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

**Production map.** `LaneQueueReservationKeyV1`,
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
batch and fsyncs one exact ordered V1 batch of `LaneQueueReservationKeyV1`
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
staging. `CanonicalWsvMergeCommitAuthorization`,
`MergeExecutionCommitSurface`,
`StateBlock::merge_execution_external_event_bytes`,
`StateBlock::merge_execution_write_set_root_from_overlay_with_external_events`,
`StateBlock::validate_merge_execution_commit_surface`,
`StateBlock::validate_merge_execution_external_event_publication_surface`,
`StateBlock::validate_staged_merge_execution_authorization`,
`StateBlock::mint_canonical_carrier_commit_metadata_authorization`,
`StateBlock::apply_without_execution_with_commit_qc_inner`, and
`StateBlock::commit_inner` enforce the exact pristine,
post-block/pre-vote, event-publication, and finalized carrier boundaries. Duplicate
transaction membership is rejected in
`crates/iroha_core/src/block.rs`. The application path in
`crates/iroha_core/src/sumeragi/v2_apply.rs` finalizes queue reservations only
after canonical Kura/WSV commit.

**Closure condition.** Every peer deterministically re-executes the exact
certified batch on the committed base WSV and atomically commits canonical
Kura/WSV state. Staging requires no pending block hash or transaction-height
row; pre-vote validation requires no pending block hash and one canonical row
containing exactly the certified merge entrypoints at the carrier height; final
application requires that same row plus the exact singleton finalized carrier
hash. The move-only WSV authorization retains the
exact encoded autonomous external-event prefix and its event count. Pre-vote
validation proves that prefix unchanged, reconstructs the certified root from
only those retained autonomous bytes, and separately binds the complete
deterministic carrier event vector. Final application byte-compares that bound
complete vector before appending the ordinary Applied block event and draining
the live buffer. Metadata mint and State commit require that live buffer to be
empty and recheck the certified economic write-set root separately against the
batch-bound WSV authorization. Mint also snapshots the exact Native-frontier-
inclusive post-finality write-set root into a move-only metadata authorization;
State commit independently compares that sealed root with the current complete
post-finality overlay before publishing either metadata or WSV state.
Only afterward may the queue transition the exact reservation
through Commit and ForgetCommit. A losing proposal, timeout, reconfiguration,
or retirement releases the exact reservation in original enqueue order.
Restart reconciliation retains a reservation if and only if exact durable
payload/certification/application evidence owns it.

**Focused and adversarial tests.** Cover base-state mismatch, nondeterministic
result attempt, tampered results/write set/post state, duplicate ordinary/merge
membership, Kura-before-WSV and WSV-before-queue crash boundaries, each
Commit/ForgetCommit boundary, repeated reconciliation, losing/future/stale
proposal, timeout, reconfiguration, retirement, release-order restoration,
pre-vote absence/wrong-height/extra carrier membership, a premature pending
carrier hash, a mismatched finalized carrier hash, and unbound finality-time
event-surface drift.
The focused inventory names
`state::tests::finalized_merge_execution_commit_surface_borrows_exact_carrier_hash`,
`state::tests::autonomous_execution_commit_rejects_post_publication_event_surface_drift`,
`state::tests::autonomous_execution_pre_vote_rejects_due_start_of_block_effect`,
`state::tests::autonomous_execution_pre_vote_requires_exact_merge_carrier_membership`,
`state::tests::autonomous_execution_pre_vote_rejects_wrong_carrier_membership_height`,
`state::tests::autonomous_execution_pre_vote_rejects_extra_carrier_membership`,
`state::tests::autonomous_execution_pre_vote_rejects_premature_pending_carrier_hash`,
and
`state::tests::autonomous_execution_finality_rejects_unbound_event_surface_drift`.
The final test also corrupts the QC-bound autonomous event prefix before
pre-vote and requires rejection before retained-root reconstruction can mask
the drift.
Assert exactly one history/query inclusion result.

**Formal obligation and mutation.** Invariant `MLCarrierExactlyOnce` states
that each accepted reservation has one of three terminal outcomes: globally
applied once, released once in FIFO position, or durably retained by one live
certified owner. `MLCarrierCommitSurfaceExact` additionally preserves the
ordered pristine, exact merge-only post-block/pre-vote, and exact finalized-carrier
surfaces, including the exact autonomous event prefix, separately bound full
carrier publication vector, and drained final live buffer. Mutation
`ML-MUT-AUT-05` advances queue state before
Kura/WSV, drops re-execution, treats any local payload as ownership, or admits
drifted pre-vote carrier metadata, an altered autonomous event prefix, or a
post-validation publication-vector change; the model must expose loss,
duplication, stale retention, or premature carrier authorization.

**Release gates.** `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, and `G-FINAL`.

### ML-AUT-06 — restart ownership reconciliation

**Implementation:** Implemented.
**Closure:** Implemented.
**Evidence:** Open.

**Production map.** Startup repeatedly calls
`plan_lane_reservation_ownership` and then
`apply_lane_reservation_reconciliation_plan` from
`crates/iroha_core/src/sumeragi/v2_apply.rs` through
`crates/iroha_core/src/sumeragi/v2_runner.rs`. Before the first Queue ownership
snapshot, `reconcile_pending_autonomous_lifecycle_terminal_outcomes` runs with
the Queue startup gate closed and requires a second Kura inventory readback to
contain zero Pending outcomes. The immutable plan classifies
every reservation group together against State, Kura's bounded autonomous
evidence classifier, pending merge entries, and exact committed carrier
indices. Missing canonical bodies enter authenticated, fixed-chunk
Commit-QC-signer recovery; pruned historical autonomous carriers require an
exact durable recovery installation before any Queue mutation. Application
then retains one authenticated owner, releases orphans in original enqueue
order, resumes terminal Commit/ForgetCommit, and opens the Queue startup gate
only after all groups and crash barriers have completed.

Kura's versioned, hash-protected lifecycle outcome records distinguish
Pending from Complete. Pending is a durable recovery source, never Queue
terminal authority. Canonical recovery separately reconstructs the complete
carrier outcome set, authenticates every `ApplyCarrier` group, and performs
one all-group Queue preflight before any cleanup; release recovery requires
the exact retirement/finalization authority. Kura may promote Pending to
Complete only by consuming positive Queue terminal evidence.

Earlier-height recovery retains one
`HistoricalRecoveryRequestCadence` per exact historical identity. Each service
turn calls `persist_historical_recovery_session` before consulting the network
deadline, so local completion and supersession retire both request and reverse
hash ownership immediately. A changed request, request hash, or typed wait
reason discards the prior cadence and starts a fresh immediate owner; local-only
waits and missing finality also clean up any retained request. Only a due,
actually retained fanout advances the bounded retry tier, and its next deadline
is anchored at the current service time. The floor is the retransmission
interval and the ceiling is the round timeout derived from immutable signed
startup block cadence. Both ordinary and decided quiet retransmission branches
service one retained historical owner even without lane ingress.

**Closure condition.** Replace the production false stub with exact,
bounded, authenticated ownership reconciliation. Retain only reservations
matched by current-incarnation durable payload/certified-bundle/global
application state. Release every other reservation exactly once in original
enqueue order. Prune only terminal forgotten reservations. Recovery must not
use Pending as terminal ownership, must not clean up a prefix of a canonical
carrier batch before every later group has passed semantic authentication and
Queue preflight, and must not complete a release without the exact
retirement/finalization authority. Keep the startup gate closed through the
terminal sweep, require zero Pending outcomes on the final readback, and only
then take the first Queue ownership snapshot.

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
ownership. A historical current-source checkpoint passed this exact B/A/B
regression (`1/1`), the startup-replay binding regressions (`2/2`), the bounded
historical namespace/accounting suite (`6/6`), first-merge crash-window repair
(`1/1`), Native post-WSV retention (`1/1`), and authenticated geometry refresh
(`1/1`) under isolated Rust 1.93.1 locked/offline execution. The mutable
focused inventory still names those 12 tests, but this reconciliation makes no
immutable-candidate execution claim. These 12 focused tests are mapped row evidence, not the complete 522-test `G-UNIT` receipt,
so this row's evidence remains Open.

The source-bound focused inventory binds the runner startup order directly and
includes `production_adapter_stays_carrier_silent_until_exact_queue_activation`
in `crates/iroha_core/src/sumeragi/v2_lane_work.rs`, the positive and
fail-closed deferred-carrier completion regressions in
`crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_01.rs`,
and also includes
`startup_reconciles_lifecycle_before_lane_work_activation` and
`terminal_sweep_source_partitions_whole_units_before_any_mutation` in
`crates/iroha_core/src/sumeragi/tests/v2_runner_lifecycle_startup_order.rs`,
`generation_takeover_runs_crash_recover_and_rehydrate_then_stutters` and
`prepared_bootstrap_and_crash_boundaries_resolve_only_their_durable_side` plus
`empty_queue_reconciliation_returns_the_same_checked_receipt` in
`crates/iroha_core/src/sumeragi/tests/v2_lifecycle_recovery.rs`,
`lifecycle_release_terminal_outcomes_are_exact_idempotent_and_ordered` in
`crates/iroha_core/src/kura/tests/07f_canonical_carrier_terminal_recovery_tests.rs`,
and the Queue group preflight/prefix-replay regressions in
`crates/iroha_core/src/queue/lane_reservation_tests.rs`. This inventory is a
static source binding; it is not a fresh test receipt.

`historical_missing_canonical_block_schedules_authenticated_retry_then_completes`
source-binds pre-deadline local checks, exact request-byte/peer reuse,
backpressure that does not advance cadence, service-time deadline anchoring,
bounded floor/ceiling growth, and request/diagnostic cleanup after local
completion. `quiet_retransmission_tick_services_one_retained_historical_session`
binds the no-ingress runner turn. The 12-case passive/recovery Python contract
also mutates reason/request reset, local-check ordering, deadline anchoring,
signed bounds, cleanup, and each quiet branch. These are static controls only;
Evidence remains Open until the focused Rust suites execute from the immutable
candidate.

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
length. Invariant `MLTerminalOutcomeJoinAuthenticated` separates the durable
Pending source, canonical cleanup or exact release authority, physical Queue
terminal fact, positive Queue evidence, and Kura Complete promotion;
`ML-MUT-AUT-12` grants terminal ownership from Pending, omits release
finalization authority, or completes without positive Queue evidence.
Invariant `MLCanonicalTerminalBatchAtomic` requires complete carrier-outcome
reconstruction, independent authentication of every group, and one all-group
preflight before any Queue cleanup; `ML-MUT-AUT-13` recovers a Pending prefix
or omits a later conflicting group. Invariant `MLTerminalStartupSweepOrder`
requires the closed-gate sweep and zero-Pending readback before the first
Queue ownership snapshot; `ML-MUT-AUT-14` snapshots early or leaves Pending
after the sweep. Invariant `MLLocalProducerRecoveryRequiresQueueOwner`
requires a nonterminal retained attempt produced by the local validator to
carry its exact current Queue reservation group before Crash/Recover, while
an observer may recover from exact local Kura custody with an empty local
Queue. The network-ingress startup fence remains independent of Queue's
observed owner-quarantine bit, which may be false for that empty observer
snapshot; `ML-MUT-AUT-15` drops the local producer's Queue group immediately
before recovery.

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

### ML-LIFE-05 — bounded stage diagnostics and the 13-peer global stall

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
gap. `durable_lane_diagnostic_execution_status`, State's Native/durable/
autonomous projections, `V2LaneWorkAdapter::committed_lane_block_status_snapshot`,
and the Torii handler use Kura's bounded `*_without_sidecar_repair` readers.
The nested Kura provider refuses append or sidecar repair and payload
recoverability probes call the recovery implementation with mutation disabled.
Focused State and Torii controls repeat diagnostics over staged recovery
artifacts, assert unchanged paths and revision, and then demonstrate that only
an explicit recovery call promotes them. The 13-peer global rerun (twelve lane
validators) required to prove the corridor correction empirically remains
`G-12P`; static source binding does not close it.

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
same-height identity as `conflict`. The endpoint's State/Kura/V2 call graph is
observation-only: merge-ledger lookup, certified/latest artifact selection,
receipt/preflight inspection, and payload recoverability all use bounded
no-repair paths. Explicit Kura repair is exercised only after repeated State
and Torii diagnostic reads in the focused separation controls.

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

The 12-case static passive/recovery contract rejects a repairing State, V2, or
Torii projection, a missing nested Kura provider, or a focused control that no
longer proves explicit-recovery separation. It is a source contract, not an
endpoint execution receipt; Evidence remains Open.

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
**Closure:** Open.
**Evidence:** Open.

**Production map.** Rust client methods live in
`crates/iroha/src/client.rs`; its source-separated endpoint-parser controls live
in `crates/iroha/src/client/sumeragi_api_separation_tests.rs`. Swift
status/diagnostics methods live in
`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`, while their models live in
`IrohaSwift/Sources/IrohaSwift/ToriiSumeragiModels.swift`. Python surfaces live in
`python/iroha_torii_client/client.py` and
`python/iroha_python/src/iroha_python/client.py`. JavaScript methods live in
`javascript/iroha_js/src/toriiClient.js` and
`javascript/iroha_js/src/toriiBrowserClient.js`; their shared exact parsers and
models live in `javascript/iroha_js/src/sumeragiTyped.js`. Both Node and browser
clients expose distinct typed status and diagnostics methods. Kotlin status and
diagnostics models live in
`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/consensus/SumeragiStatusModels.kt`
and
`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/consensus/SumeragiDiagnosticsModels.kt`;
the mirrored Java status, diagnostics, and exact JSON support live in
`SumeragiStatusModels.java`, `SumeragiDiagnosticsModels.java`, and
`SumeragiJsonSupport.java` under
`java/iroha_android/src/main/java/org/hyperledger/iroha/android/consensus/`.
Each client keeps status and diagnostics on distinct methods, parsers, and
return models. Ten Rust-owned receipt-graph types and the typed SDK parsers
reject unknown fields and duplicate or over-bound input rather than accepting
an implicit compatibility shape.

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

The mutable source inventory contains exact diagnostics-parity counts for the
Rust client (`14`), both Python client layers (`129`), JavaScript source and
distribution (`88`), Swift (`34`), Kotlin core-jvm (`43`), and mirrored Java
(`42`). JavaScript uses the dedicated
`sumeragiDiagnosticsContract.test.js` entrypoint instead of a monolithic
name-pattern filter, and the inventory includes an explicit swapped
status/diagnostics payload negative. The development resolver
`ci/resolve_sumeragi_v2_sdk_source_closure.py` and manifest
`ci/sumeragi_v2_sdk_source_closure.json` cover transitive production sources
and Kotlin/Java Native model dependencies. The current mutable-tree inventory
is exactly 1,451 grouped and 1,453 diagnostics records. Their canonical hashes
are
recorded once in the owning corpus row and once in the release gate. The
release receipt must reproduce those values from its immutable candidate;
the mutable-tree values alone are not evidence. The two specialized static
Python modules are canonically runner-bound and the browser distribution
matches its source. The release corridor and receipt bind the Rust wire
consumer directly; the Swift, Kotlin,
and Java wire suites are included in the diagnostics runner and its receipt
counts. These are source-inventory constraints only;
OpenAPI artifacts still awaiting immutable-candidate regeneration and
missing immutable-candidate source-seal and execution evidence keep the suite
digest and `G-SDK` release claim ineligible.

**Differential release invariant and negative control.**
`MLApiAuthoritySeparation` states that no diagnostics-only field can satisfy an
authoritative status claim. This is not a TLA+ invariant: the source-bound
OpenAPI and SDK endpoint/parser corpus is the authoritative check. Negative
control `ML-MUT-API-02` aliases the two parsers or response types; a swapped
payload must then be accepted and the differential contract must fail. Its
source binding covers the Rust client and all six SDK surfaces, including
full status-for-diagnostics and diagnostics-for-status payload swaps in Rust,
Kotlin, and the mirrored Java HTTP client. This closes the known Rust source
gap without changing the counted 14-test Rust diagnostics suite; it does not
provide immutable-candidate execution evidence.

**Release gates.** `G-SDK` and `G-FINAL`.

### ML-API-03 — identical Native V2 validation across SDKs

**Implementation:** Implemented.
**Closure:** Open.
**Evidence:** Open.

**Production map.** Rust Native AMX wire types and validators are in
`crates/iroha_data_model` and `crates/iroha_core/src/native_amx.rs`; client
mirrors are distributed across the Python, JavaScript, Swift, Kotlin, and Java
paths named in `ML-API-02`.
The mutable transitive closure inventory includes those production mirrors and
their Kotlin/Java Native dependencies. Every reviewed input and both specialized
static tests are tracked, runner-bound, and represented in the mutable closure;
the browser JavaScript distribution matches its source. Cross-SDK accept-set
closure remains Open until the generated artifacts are reproduced from the
clean immutable candidate and one complete differential run agrees with Rust.

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
`crates/iroha_data_model/src/bin/native_amx_grouped.rs` generates
`fixtures/sumeragi_v2/native_amx_v2_grouped.json`, including grouped golden
data and application evidence.
`NativeAmxAttestationBodyV2::computed_grouped_participant_settlement` is the
explicit production builder; single-source construction is labelled as a test
fixture.
`ci/run_native_amx_v2_grouped_sdk_parity.sh` source-binds the exact fixture and
OpenAPI, Python, JavaScript source/distribution, Swift, Kotlin, and Java
consumers. The checked-in mutable-development corpus inventories 56 negative
controls: 46 validate receipt groups and 10 validate application evidence. The
corpus includes
`execution_commitment_merge_carrier_wrong_version` and
`execution_commitment_missing_merge_carrier_field`, plus the four-mutation
`coherent_duplicate_validator_set` and
`coherent_over_quorum_requirement` controls; `bounds.validators_max` is 128.
The harness and source-bound release inventory both require that exact count.
The source inventories now require OpenAPI 7, Python 63, JavaScript 61, Swift
5, Kotlin 7, and Java 6 tests. The current recursive mutable-tree closure
contains exactly 1,451 grouped and 1,453 diagnostics records. Its grouped and
diagnostics suite-source SHA-256 values are
`bdf4efd88885521e3806cfe610e7ab3d72d690ebe329a4b7acfc0b2fe9b22ae0`
and
`90235165ad20cc6e4363d4fd6935b8c25bc2e1856cdbbad3323dcc5c4843c2a3`.
The checked-in grouped fixture has SHA-256
`e4fb62addba3c3b8aecdbff55840e21620c770ab96d346ca55b156cf0239942b`.
The diagnostics closure directly includes the 48-line wire fixture whose
SHA-256 is
`79240b3b95d8c40dc8f1129177a88dca3f31fe08027fe9f5372b6a67b05e9a4c`.
The source-bound corridor now requires two disjoint Rust fixture generations,
and both JavaScript SDK harnesses require two byte-identical complete
distribution builds. Exact-five OpenAPI replay is source- and receipt-bound to
the protected schema-v3 OpenAPI Node closure; it has not run from an immutable
candidate. The five checked-in OpenAPI artifacts are dirty, unsigned
development outputs. Approved generator execution, parity hashes, and one
complete immutable-candidate harness replay are still pending.
Neither the checked-in bytes nor any mutable-development digest is a release
receipt.

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
**Closure:** Open.
**Evidence:** Open.

**Production map.** The signing guard V4, Native manifest V1, Native receipt
and latest-index layouts, serviced-candidate V4 store, queue reservation
journal/key, executable payload V1/envelope, lane QCs/NewView, merge entries,
application receipts, and diagnostics models all carry explicit versions or
exact typed Norito layouts in their owning modules. The serviced-candidate
store accepts V4 only. Unknown and retired layouts fail closed; the source tree
contains no implicit legacy consensus decoder or production feature/environment
compatibility switch for autonomous execution.
The current Kura artifact and historical-recovery enums use contiguous
zero-based Norito tags; exact-tag regressions freeze those layouts and unknown
discriminants fail typed decode.
`crates/iroha_data_model/src/bin/sumeragi_v2_wire_fixtures.rs` now generates
the positive `quorum_certificate_merge_carrier` row and negative
`execution_commitment_merge_carrier_wrong_version` and
`execution_commitment_missing_merge_carrier_field` rows. The checked-in
`fixtures/sumeragi_v2/wire_v2.tsv` is currently a 48-line
mutable-development artifact containing 44 data rows with SHA-256
`79240b3b95d8c40dc8f1129177a88dca3f31fe08027fe9f5372b6a67b05e9a4c`.
The static release binding directly seals its header and all three row keys,
the Rust generator output seam, the Rust `include_str!` consumer and its two
positive/negative test names, the Swift/Kotlin/Java wire consumer suites, the
diagnostics runner and source-closure manifest, the release runner, and the
receipt schema. The Rust two-test wire leg is release-runner/receipt-bound; the
Swift, Kotlin, and Java wire suites are diagnostics-runner/receipt-bound. Fresh
immutable-candidate regeneration and byte-parity execution remain Open; this
current hash is a mutable-development fact, not a release anchor.
The receipt-required
`merge_share_transport_rejects_omission_nonleader_body_and_legacy_version`
regression rejects a legacy merge-share version while leaving the signing guard
unauthorized. It replaced one required release-corridor selector without changing
the then-current count; the autonomous-retirement regression added here raised
that checkpoint to 857 tests. The retired-attempt, mixed-carrier, and two-link
cold-restart rows plus the two predecessor-durability handoff rows, followed by
retirement of the dormant generic persisted-continuation regression, leave the
current production inventory at 864 tests while
the G-UNIT inventory contains 522 tests. Source binding is not an execution receipt.
The finalized predecessor remains active while the shared ordinary/PendingKura
preflight rehydrates late canonical lane ownership, services bounded
historical recovery, and persists every winning certificate plus its
application witness. No incomplete lane session is transferred into the
successor. The two handoff rows instead bind independent Kura revalidation of
already durable ordinary and record-backed autonomous historical output.
`KuraReplicaAdvertV1` is explicit and clean-break. Its nested runtime policy
configures and validates TTL, refresh cadence, evictable window, replica floor,
and checked registry geometry. The direct authenticated ingress and the exact
source-bound, bounded production refresher both consume the V1 layout.
OpenAPI source additionally defines
`/v1/musubi/queries/provider-bundle-attestation` and
`/v1/musubi/instructions/provider-bundle-attestation-register` plus their
schemas, but the five checked-in OpenAPI artifacts are stale:
`artifacts/openapi/torii.json`,
`artifacts/openapi/versions/current/torii.json`, the corresponding root and
current-version manifests, and `artifacts/openapi/versions.json` still require
deterministic regeneration from a clean exact candidate. The dirty unsigned
development checkout cannot furnish release provenance for that refresh. The
runtime consumer is implemented, while wire/OpenAPI generation, cross-SDK
parity, and execution evidence remain Open.

**Closure condition.** Version every new persistence and wire layout. Reject
every non-current version before payload decoding or state mutation. Do not add
format migration, heuristic decode, mixed-layout consensus, a
feature/environment compatibility toggle, a new crate, direct Serde dependency,
Cargo.lock change, or ABI-version change.
Source every new runtime count/byte/time bound from `iroha_config`, give it a
sensible deterministic default, and reuse the existing sidecar/request budgets
where they already express the same resource.

**Focused and adversarial tests.** Round-trip the current version; reject
zero, old, future, truncated, oversized, unknown-field, noncanonical, and
wrong-Norito-flag payloads. Exercise a mixed-version committee and require it to
fail closed before signing. Run the legacy-codec guard and assert no implicit
fallback decoder exists.

**Static release invariant and negative control.**
`MLConsensusLayoutAgreement` states that all signers interpret one versioned
byte string as one identity. This is not a TLA+ invariant: the exact TSV,
generator, Rust consumer/tests, release-runner leg, and legacy-codec guard
source bindings are the authoritative static contract. Negative control
`ML-MUT-WIRE-01` removes or substitutes one of those paths or semantic tokens,
enables a retired fallback, or admits a mixed-layout path; the checker or exact
decoder tests must then fail.

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

The mutable source inventory is internally count-consistent. The production
inventory contains exactly 864 tests across 44 modules, including 448
source-sealed ownership/regression names. The duplicate inline V2 core network
simulations are retired; the standalone `iroha_sumeragi_core` harness remains. The three Kura recovery
regressions and governance-unlock audit are retained beside the prior source-bound closure.
The seven additional Native AMX regressions bind finality-aware merge
projection across canonical ordering, multi-height and same-height identity
conflicts, coordinator-only receipts, route conflicts, duplicate sources, and
decoded replay. The focused source inventory is now internally consistent. The
nine arrays in `scripts/run_sumeragi_v2_release_gates.sh` contain exactly 522 unique required
tests: 316 core, 143 queue-journal, 13 configuration, eight data-model,
39 Torii, one Torii-shared, and two integration. The runner and
`ci/check_sumeragi_v2_multilane_release_inventory.sh` both require that exact
522-row shape, including grouped Native prevote-budget rejection before
Kura/WSV mutation, historical source-bundle authentication, crash-safe latest-
index and prune-V2 recovery, cross-route manifest-barrier isolation, durable
Native signing-boundary drift rejection, atomic grouped reservation commit,
checked snapshot replay file/owner sealing, exact QueuePlan obligation
authentication, ApplyCarrier authorization, and canonical historical
autonomous recovery into exactly-once merge application. The G-UNIT static inventory checks establish exact `522/522` source consistency and also source-
bind the synchronized 56-control grouped corpus. The planned-
association Rust coverage described under `ML-NAT-06` is present in the focused
source inventory. The 17 merge-manifest cases under `ML-NAT-05` and 12 passive-
diagnostics/retry cases under `ML-AUT-06` and `ML-API-01` are static Python
source tests outside the 522 G-UNIT count.

On 2026-07-31, pinned Rust 1.93.1 locked/offline execution from isolated source
`/tmp/iroha-kura-final3.dvOYAN` and isolated target
`/tmp/iroha-kura-target2.Llklru` passed 12 then-current focused tests: six
bounded historical namespace/accounting cases, two startup-replay binding
cases, and one each for B/A/B historical recovery, first-merge crash-window
repair, Native post-WSV retention, and authenticated geometry refresh. That
checkpoint also passed `cargo check -p iroha_core --lib`; later focused reruns
covered startup binding, B/A/B recovery, the 18 Kura replica tests, and four
configuration tests. These are historical partial results, not fresh archived
execution of all 522 required tests. This reconciliation claims no immutable-
candidate Cargo run or full matrix execution: the 864 production, 522 G-UNIT,
and 56-control counts are mutable-development source inventory only. `G-UNIT`
remains Open until the exact no-skip suites run through the compliant isolated
wrapper and their logs and candidate identity are archived.

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

The mutable schema-5 binding registry contains 34 conceptual rows: 29 TLA
counterexample rows with 106 exact mutation configurations, two static-release
rows, and three differential-release rows. Its Native startup bindings name
`LaneApplicationEvidenceRepairSummary::publication_count`, explicit ordinary
pair repairs, read-only certified frontier/artifact access, reverse merge-
carrier preflight/application, planned finalized merge associations, and the
Queue-gated runner ordering. The Kura-retention production symbol inventory is
also structurally bound, including its split finality-wire owner in
`crates/iroha_core/src/kura/retained_finality_replica_authority.rs`.

The in-flight contract registers exactly 27 production actions. Its composed
state/action relation has a source-extraction seam for every name, including
`Crash`, `Recover`, `RecoverReservationSnapshot`, and
`RehydrateLocalKuraCustody`; the declared open-action tuple is empty. The
twenty-two exact TLC mutation witnesses remain mandatory source inventory, not
executed results. This is only a structural partition of current production
symbols: it does not establish operational correspondence and is not a formal
completion certificate.

No fresh immutable-candidate TLC, Apalache, TLAPS, Verus, trace-replay,
cross-tool, or mutation execution is claimed by this reconciliation. Model
hashes, tool versions, logs, expected counterexamples, and terminal completion
receipts must still be archived, so `G-FORMAL` remains Open.

### G-4P — four-peer DA and lifecycle suites

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
artifact is recorded here. Rotating message-loss faults use the
feature-isolated authenticated consensus message controller and must prove the
installed hold/drop command and matched-message evidence before its atomic heal;
retired `[sumeragi.debug.rbc]` keys are rejected configuration, not evidence.

### G-12P — twelve lane validators on a 13-peer global committee

**Evidence:** Open.

The stable gate identifier counts the twelve lane-validator assignments. Each
run must use an exact 13-member revision-4 global committee, with the final
global voter excluded from all three lane committees. Run at least three
independent four-validator dataspaces with grouped DvP and
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

The mutable grouped inventory is OpenAPI `7`, Python `63`, JavaScript `61`,
Swift `5`, Kotlin `7`, and Java `6`, with exactly 56 grouped Native negative
controls. The diagnostics inventory is Rust `14`, Python `129`, JavaScript
source/distribution `88`, Swift `34`, Kotlin `43`, and Java `42`. The recursive
source-closure design covers every transitive production input, including the
browser JavaScript distribution, SoraFS orderbook JavaScript implementation
and types, the standalone Python orderbook module, Kotlin/Java Native models,
grouped JSON, and wire TSV. Its record totals and suite-source digests must be
derived and receipt-bound from the exact immutable candidate. The current
mutable-tree closure contains exactly 1,451 grouped and 1,453 diagnostics
records, with grouped and diagnostics suite-source SHA-256 values
`bdf4efd88885521e3806cfe610e7ab3d72d690ebe329a4b7acfc0b2fe9b22ae0`
and
`90235165ad20cc6e4363d4fd6935b8c25bc2e1856cdbbad3323dcc5c4843c2a3`.
The current grouped JSON and wire TSV SHA-256 values are
`e4fb62addba3c3b8aecdbff55840e21620c770ab96d346ca55b156cf0239942b`
and
`79240b3b95d8c40dc8f1129177a88dca3f31fe08027fe9f5372b6a67b05e9a4c`.
Those are development fixture inventories, not SDK results. The changed
JavaScript production roots require fresh deterministic source/distribution
regeneration from a clean exact candidate, as do the five OpenAPI artifacts.
No complete immutable-candidate grouped or diagnostics harness execution,
parity hash, or archived result is claimed, so `G-SDK` remains Open.

### G-FINAL — clean release validation

**Evidence:** Open.

Both PR and production first reproduce one clean committed identity in an
independent no-local/no-hardlink/no-alternates clone. Before any build child,
copy the reviewed runtime and caller Cargo-cache inputs to inode-independent
private roots, bind canonical path-withheld input/output inventories, and keep
private HOME, temporary, cache, Rustup, target, and artifact roots. PR is
disposable developer validation and identity-cleans the whole invocation on
every terminal path. Production validates the receipt with the protected
archived validator, requires its no-clobber acknowledgment, then prunes all
runtime/cache/target state and retains only the authenticated source,
receipt/identity, and exact retained inventory/result.

Before every Cargo invocation, acquire the invocation-local owner-private
directory lock below the authenticated external artifact root and fail closed
if that lock is held; never inspect or log unrelated processes. Use only the
authenticated absolute Cargo 1.93.1 executable, `--locked --offline -j1`, and a
fresh isolated `CARGO_TARGET_DIR`. Run focused crate tests, SDK parity suites, formal runners,
`cargo build --workspace`, full `cargo test --workspace`, strict workspace
Clippy, formatting check, and
`scripts/check_no_legacy_codec.sh`. The release record must contain commands,
exit status, source revision, target directory, toolchain, and artifact hashes.
`scripts/run_sumeragi_v2_release_gates.sh` must require the completed multilane
focused, SDK, formal, four-peer, 13-peer global, and scaling gates and must fail on
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
- The Rust-owned grouped and wire generators and the 56-control corpus are
  present. Current checked-in fixtures, OpenAPI files, JavaScript distribution,
  and parity hashes remain mutable artifacts awaiting immutable-candidate
  regeneration, so `ML-API-04`, `ML-WIRE-01`, and `G-SDK` stay Open.
- **Autonomous lifecycle-cursor reconciliation:** the former
  `TODO(multilane)` between
  `V2LaneWorkAdapter::install_lane_drain_queue` and
  `V2LaneWorkAdapter::activate_after_lane_drain_queue_install` is gone.
  Production startup now performs signed lifecycle bootstrap, generation
  takeover, Queue snapshot recovery, local Kura rehydration, drain-queue
  installation, and activation-time owner revalidation. Its `Crash`, `Recover`,
  `RecoverReservationSnapshot`, and `RehydrateLocalKuraCustody` extraction
  names are structurally bound; formal and execution evidence remains Open.
- **Lifecycle-coordinator cutover:** one crate-internal
  `LifecycleCoordinator`/`LifecycleLedgerV1` owner now controls ordinary and
  recovered production lifecycle admission, immutable ordinals, launch,
  retries, completion, finalization, restart recovery, and successor rollover.
  Certified-Serve payload durability and the concrete work registry are joined
  to that owner; the former Serve snapshot, persistent predecessor witness,
  latch, and producer-episode scheduler authorities are retired. Fresh
  immutable-candidate execution and release evidence remains Open.

### No unresolved in-scope explicit TODO marker

The current reviewed closure contains no explicit TODO in lane routing,
autoscale, merge, reservation ownership, Native AMX, drain, retirement, or
multilane diagnostics. The former SafetyWal filesystem-identity marker is now
implemented and source-bound: production runner cutover mints the opened WAL
directory authority from Kura's retained opened root, and the adapter consumes
that move-only authority only for the exact Kura instance. Any newly introduced
marker must be classified here before release.

### Unresolved in scope without a TODO marker

- The former authenticated Kura V1 producer/configuration and wire-consumer
  implementation gaps are resolved and source-bound. Their focused Rust,
  formal-engine, SDK, and multi-peer execution receipts remain open; structural
  source validation alone cannot close those gates.
- `G-UNIT`, `G-SDK`, and `G-FORMAL` remain Open. The exact 864-production-test,
  522-G-UNIT-test, and 56-control counts, SDK group counts, recursive closure
  shapes, and 27-action formal extraction partition are mutable-development
  inventories only. Historical focused Rust and direct SDK subsets do not
  substitute for a complete SDK harness, formal-engine result, or network
  receipt; no complete 522-test execution from an immutable candidate is
  claimed by this reconciliation.

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
- **Generic Sumeragi formal evidence:** the concrete historical Decision-fetch
  mapping is now stated under the promoted
  `ProgressWitnessProductionRefinementObligation`; no formal-directory TODO
  remains for that mapping. Its fresh strict TLAPS, Verus, and derived
  cross-tool evidence are generic release gates, not multilane identities or
  substitutes for lane-specific safety/evidence rows.
- **Generic causal-scheduler projection and liveness:** production now contains
  `ProductionEffectToCandidateTraceProjection`,
  `check_production_effect_to_candidate_transition`,
  `production_effect_to_candidate_trace_refines_async_ownership`, and the
  `production_fresh_causal_successors` sequence lemmas. The source-only
  causal-FIFO seam check passes. This establishes source binding only: it is not
  pinned Verus, TLAPS, or derived cross-tool evidence and does not discharge
  Completion-capacity temporal liveness. The generic scheduler refinement does
  not model lane reservations, autonomous merge carriers, Native participant
  evidence, autoscale, drain, or retirement and remains outside this multilane
  ledger.
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

- QueuePlan and reservation journal V1 key/FIFO/release layouts, Native signing
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
