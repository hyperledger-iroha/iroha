---
title: Nexus Cross-Lane Execution
sidebar_label: Cross-Lane Execution
description: Production lane lifecycle, certification, global merge, proof, and recovery rules.
---

# Nexus cross-lane execution

Nexus partitions transaction scheduling and lane-local certification while
retaining one deterministic, globally ordered WSV. Lanes can be created and
retired automatically to add horizontal execution capacity, but lane QCs never
mutate shared state directly. A merge QC bound to a canonical global carrier is
the only bridge from autonomous lane execution into WSV.

This document describes the current production path. The exact merge protocol
and storage crash contract are specified in [Merge ledger](merge_ledger.md).

## End-to-end flow

1. The live Nexus router assigns each accepted transaction to a canonical
   `(LaneId, DataSpaceId)` using explicit rules or the default-route shard set.
2. An autonomous lane leader creates a non-empty payload and authenticates its
   queue reservations, accepted transaction hashes, routing plans, and RBC
   ownership.
3. The authoritative lane committee forms an availability-backed prepare QC
   and a commit QC. The complete certified source is persisted independently of
   global block arrival.
4. The deterministic global-round leader chooses the next contiguous certified
   source from eligible lanes, orders the sources canonically, executes them on
   the committed WSV base, and disseminates that exact candidate.
5. Followers authenticate the leader, reconstruct and re-execute the exact
   embedded body, durably lock the round/digest, and contribute merge-QC
   signatures.
6. A canonical global block carries a compact reference to the certified full
   entry. Nodes missing the sidecar fetch it from merge-QC signers and defer the
   block until the exact body is available.
7. The merge entry is staged before ordinary block effects, Kura makes the
   block/entry/carrier durable, and WSV commits the complete deterministic
   overlay atomically.
8. Transaction queries expose both ordinary and merge-carried transactions,
   with a proof binding each merge result to the compact carrier.

This separation permits lane committees to progress independently without
allowing network arrival order, machine speed, or local inventory to choose a
global state order.

## Routing and horizontal capacity

The canonical account identity remains domainless. Dataspace and lane routing
come from Nexus configuration and committed lane lifecycle state.

Explicit routing rules target fixed operator-managed lanes. Automatic elastic
lanes are reserved for unmatched traffic on the default route and cannot be
made explicit-rule targets. Stateful routing uses the current committed catalog,
dataspace catalog, lane incarnation, activation height, and autoscale range;
stale router snapshots fall back to the fixed default lane rather than routing
work into a retired or future lane.

The default route is sharded deterministically across its fixed anchor and
currently active, valid `autoscale.managed` elastic lanes. A managed lane must:

- lie in the configured elastic ID range;
- use the default dataspace and the supported public base profile;
- carry the exact reserved ownership and creation-height metadata;
- carry the exact ordered BLS committee and aligned proofs of possession pinned
  for that incarnation;
- have a committed, never-reused incarnation; and
- have reached its first eligible proposal height.

Malformed ownership markers, manual occupants of reserved IDs, disabled Nexus
or autoscale state, future creation heights, off-default dataspaces, or catalog
drift fail closed.

## Automatic lane creation and retirement

Autoscale parameters are ordinary `iroha_config` values under
`nexus.autoscale`; production behavior does not depend on environment toggles.
Every validator computes the same decision while applying the same canonical
global block.

### Scale out

For the configured block window, validators derive deterministic p95 block
latency and utilization from canonical block data. Sustained latency or
utilization pressure selects the first free elastic lane ID. The new lane is
cloned from the supported default public profile, receives exact managed
metadata and a fresh incarnation, and is committed through the lane lifecycle
journal. Before publication, the node verifies that the prospective
authoritative lane committee has the exact required `3f+1` members. Lane
committees are capped at 128 validators by both configuration parsing and
runtime admission, matching the lane-QC and drain wire protocols.

The ordered committee and one verified BLS proof of possession per member are
pinned into reserved lane metadata *before* the catalog and incarnation hashes
are derived. Proposal, prepare/commit QC, availability, NewView, and Native AMX
authority for that incarnation all resolve this pin; they never fall back to a
later global roster, manifest, or live-key cache. Changing the pin therefore
requires retiring and recreating the lane. A roster/key-policy change that
removes too many pinned members may stall that lane safely, so operators must
drain and retire affected lanes before rotating away their required quorum.

The lifecycle block is the creation boundary. The lane becomes eligible for
proposals only at the following global height, so messages prepared before
catalog commitment cannot race activation.

### Scale in

A complete cold window and expired cooldown select the highest active managed
elastic lane. Selection begins an irreversible drain; it does not remove the
lane immediately. The committed intent records the exact chain, lane,
dataspace, incarnation, close height, current merged frontier, and ordered
`3f+1` lane committee. Routing and lane authority admit proposal heights up to
the close height and reject every later proposal, while already certified
pre-close work may still reach the global merge frontier.

Each historical committee member keeps a crash-safe signing journal. It will
not sign a drain below any Commit vote it signed, cannot sign conflicting or
regressing drain bodies, and cannot sign a later Commit vote after closing. It
may re-sign the same intent at a strictly higher canonical frontier when
delayed pre-close work is globally applied. Once the committee reaches its
canonical quorum on the exact final frontier, any current global validator can
assemble the self-contained certificate. The next deterministic merge leader
orders it in a certificate-only merge entry. The carrier atomically replaces
the intent-only metadata with a commitment to the certificate, merge-entry
hash, carrier height, and final frontier.

The journal directory has one exclusive `owner.lock`; a second process cannot
open the same signing identity concurrently. Lock files and decision records
must be regular, bounded, canonically encoded files and symlinks fail closed.
Inbound drain-vote frames are rejected before materialization when they exceed
16 KiB or the 128-member sequence bound, on both TCP and QUIC paths. This keeps
an unauthenticated oversized committee from turning control-plane decode into
an allocation attack.

Retirement is considered only in a global block whose height is strictly
greater than the certificate carrier. At most one lane retires per block, and
retirement is refused when the candidate:

- owns work in the committing block;
- has an unmerged admissible relay;
- has a certified lane block without a matching global application receipt;
- has an unrepaired application marker;
- has no exact globally carried drain certificate, or its replicated frontier
  differs from the certified frontier;
- is outside the managed range, manually owned, malformed, or non-default; or
- would violate the fixed base capacity.

The selector does not skip a blocked highest lane to destroy a lower lane.
Only after blockers are merged/repaired, the close certificate is carried, and
a later block observes the exact frontier does the highest lane retire. The
same process repeats in descending lane order until only the fixed base
capacity remains. Additional cold heartbeats then produce no lifecycle
transition. Malformed or ambiguous drain metadata fails closed and suppresses
both a second drain and scale-out rather than skipping the affected lane.

### Atomic geometry and cleanup

Lifecycle publication is consensus state. Kura journals physical geometry
preparation and reconciles it against the committed catalog after restart. A
failed block or catalog preflight cannot leak a partially created/retired lane.
On successful retirement, lane-scoped DA cursors and commitments, pin intents,
verified relays, merge history indexes, queue/session state, public validator
and economic rows, emergency overrides, AXT replay data, and application
markers are reset at the same incarnation boundary. Old files remain historical
proof material only where policy requires them; they cannot authorize or route
new work.

Consensus-owned smart-contract-state namespaces are not writable through
generic IVM state syscalls. Merge application/frontier markers, Nexus fee
replay markers, and sealed-transaction commitments are opaque even to reads or
enumeration; verified relay and fee-budget records remain readable where they
are a public contract surface but are read-only. Delimiter-aware negative tests
preserve similarly named user keys while preventing contracts from forging or
deleting lifecycle safety state.

## Lane certification and data availability

`LaneBlockProposalV1` binds the lane/dataspace/incarnation, global proposal
height, lane-local height/view and predecessor, exact accepted queue indices and
transaction hashes, payload ownership/RBC identities, ordered validator set,
canonical quorum, and proposal hash.

Prepare votes require payload availability. A certified source contains:

- the producer-authenticated origin payload and current proposal;
- the prepare QC with its availability QC;
- the commit QC;
- canonical signer proofs of possession;
- the exact payload bytes or recoverable canonical block hint; and
- chain/epoch/payload hashes sufficient for restart verification.

Ingress and local loopback use the same route, activation, reset-watermark,
committee, quorum, signature, conflict, and size checks. Senderless votes,
self-appointed committees, downgraded quorum, wrong dataspace/incarnation,
stale predecessors, duplicate/conflicting slots, and malformed RBC ownership
are rejected before cache, status, vote, or broadcast side effects.

Certified sources are durable even when the global leader has not yet selected
them. Queue backpressure does not evict quorum-certified sessions, and restart
hydrates only fully revalidated current-incarnation artifacts.

## Relay commitments and settlement

`LaneBlockCommitment` records the lane coordinates, ordered settlement receipts,
Nexus fee receipts, Native AMX receipts, totals, and optional swap evidence.
`LaneRelayEnvelope` binds that commitment and its hash to the lane header, lane
QC, DA commitment, RBC byte count, manifest root, and FastPQ proof material.
The header height is the global proposal and authority context used for
lifecycle, committee, key-history, and policy checks. The envelope and
settlement `block_height` is the incarnation-scoped lane-local coordinate used
for relay identity and contiguous merge progression; a recreated lane therefore
restarts at lane-local height 1 without reusing its retired incarnation.

A relay becomes merge-admissible only after all structural, committee,
signature, DA, proof, settlement, and activation checks pass. Contract-persisted
verified relay records are revalidated before hydrating the runtime cache.
Envelope identity includes immutable header, descriptor, DA, settlement, RBC,
and manifest fields, so a later “upgrade” cannot overwrite drifted evidence.

Relay settlement candidates contain the exact envelope. Merge validators
preflight duplicate markers, fee schedule arithmetic, canonical asset
selection, aggregate burns, and payer balances before signing. Settlement is
then staged on the same pristine carrier overlay and is idempotent across crash
replay.

## Native AMX cross-dataspace transactions

Native AMX execution uses the same globally certified batch. A routing plan
names its coordinator and every participant leg. The producer must collect the
required participant prepare/commit QCs; coordinator-only evidence is not
synthesized. `NativeAmxReceipt.authority_context_height` binds the global
application context, while each leg retains its lane-local height. Validation
checks chain, source ID, entrypoint, routing-plan digest, lane/dataspace roles,
authority height, participant committees, QCs, and duplicate sources before
state execution.

All entrypoints in one merge batch execute in canonical order on one revertible
overlay. Any divergence in results, settlement evidence, write-set roots, or
expected post-state hash invalidates the complete candidate.

## Compact carrier, recovery, and proofs

The full merge entry can be up to 16 MiB; the global block instead carries a
`CertifiedMergeLedgerReference`. Its merge QC identifies authenticated sidecar
holders. Fetch uses bounded 64-KiB chunks and global/per-peer resource caps,
while authoritative pending blocks retry through holder withholding without a
fixed attempt horizon.

Kura maintains indexed full-entry and carrier stores. A node that restarts with
only the compact block fetches or replays the exact sidecar, re-executes the
same WSV transition, and reconstructs the same transaction history. Torn,
oversized, non-canonical, conflicting, symlinked, or future-uncommitted storage
is truncated or rejected according to the crash boundary; incomplete network
assemblies are never persisted.

Chain truncation publishes a fsynced prune intent before lowering the durable
block marker. Carrier/log, commit-roster, WSV-checkpoint, commit-manifest,
pipeline-recovery, and roster-metadata sidecar suffixes are removed
forward-only, and the live block/query indexes remain on the old prefix until
all durable stages complete. Startup finishes an interrupted intent before
serving state; conflicting, tampered, symlinked, or non-canonical intent and
sidecar material is rejected. Durable lane writers share the prune and
lane-geometry locks, preventing stale-path resurrection while scale-in archives
an incarnation. A poisoned merge-height lookup is an error rather than an empty
history, so no candidate can restart lane height progression while recovery is
outstanding.

`FindTransactions` returns merge-carried entrypoints with
`CertifiedMergeTransactionInclusion`. Clients can verify both entrypoint and
result Merkle proofs against the compact reference and then bind that reference
to the exact signed carrier block. A merge transaction is not duplicated in the
ordinary block Merkle roots.

Hash/authority/time/status filters use Kura's unified ordinary-plus-merge
height index and read only selected carriers. Unindexed pagination scans and
drops one full sidecar at a time, retaining only the current page; exact-count
and sorted requests still validate all selected evidence without retaining all
proofs. Unsorted cursors are anchored to a canonical prefix and resume from a
compact height/intra-carrier checkpoint. Sorted windows are materialized once
only after full validation and are capped at 4,096 positions, so oversized or
unbounded requests fail at the materialization limit rather than becoming a
history-sized allocation. The eager transaction count of each touched carrier
is precharged before merge Merkle proof construction or predicate evaluation;
insufficient gas cannot trigger unaccounted carrier-sized work.

## Status and events

`/v1/sumeragi/status` exposes lane proposals/commitments, relay envelopes,
payload ownership, committed lane sessions and execution status, configured
lane/dataspace geometry, and autoscale transition data. Status rows are
validated before publication and conflicting latest identities remain visible
as ambiguity rather than being silently collapsed.

The first successful live publication of a globally carried entry emits one
`MergeLedgerEvent`. Kura/state retries and restart replay are silent, so event
consumers do not observe duplicates. Transaction pipeline events continue to
carry the routed lane ID before final merge approval.

Operational metrics include lane scheduler age/utilization, DA/RBC deferrals,
relay validation outcomes, lane lifecycle outcomes, and autoscale capacity.
Rollout evidence must use quorum-consistent status and transition rows; file
mtime changes, descriptorless relays, wrong-dataspace rows, ambiguous duplicate
rows, or stale prior-cycle logs do not prove expansion or safe contraction.

## Verification matrix

The production corridor includes:

- unit tests for router activation/incarnation boundaries, committee and quorum
  authority, lane proposal/QC aggregation, payload recovery, merge
  re-execution, fee preflight, lifecycle atomicity, cleanup, and recovery;
- negative tests for wrong leader/round/parent/roster, equivocation, duplicate
  signatures, lost signing locks, malformed/corrupt chunks, oversized counts,
  stale/future lanes, forged/under-quorum drain certificates, post-close work,
  same-carrier retirement, delayed pre-close work loss, unsafe retirement, and
  every durable crash boundary;
- four-peer localnet autoscale expansion/contraction, repeated cycles, a
  full intent/certificate/later-retirement cycle, a certified elastic-lane merge
  with one offline/missing-sidecar peer, WSV/query proof convergence, and
  repeated restart idempotency;
- twelve-peer cross-dataspace/native-AMX integration and soak corridors; and
- TLC/Apalache models for autoscale lifecycle, pinned-incarnation authority
  under independent current-roster rotation, merge execution order, and exact
  merge-carrier safety, each with expected-failure mutations.

Primary code lives in `crates/iroha_core/src/state.rs`,
`crates/iroha_core/src/lane_consensus.rs`,
`crates/iroha_core/src/lane_drain.rs`,
`crates/iroha_core/src/sumeragi/main_loop.rs`,
`crates/iroha_core/src/merge_sidecar.rs`, and
`crates/iroha_core/src/kura.rs`; canonical DTOs live under
`crates/iroha_data_model/src/{block,merge,nexus,query}`.
