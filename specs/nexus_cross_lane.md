---
title: Nexus Cross-Lane Execution
sidebar_label: Cross-Lane Execution
description: Cross-lane lifecycle, certification, global merge, proof, and recovery rules.
---

# Nexus cross-lane execution

Nexus partitions transaction scheduling and lane-local certification while
retaining one deterministic, globally ordered WSV. Lanes can be created and
retired automatically to add horizontal execution capacity, but lane QCs never
mutate shared state directly. For an accepted embedded autonomous batch, only a
merge QC bound to a canonical global carrier can bridge its effects into WSV.

The production runtime originates reservation-bound autonomous payloads. A
deterministic height-rotated lane author fsyncs exact FIFO reservation
identities before queue ownership moves, carries the same ordered bytes through
lane availability and finality, and makes a certified bundle merge-eligible
only after it is durable. Losing, timed-out, reconfigured, or retired attempts
first persist an exact slot retirement and then release those reservations in
original enqueue order. Effects still execute only through the canonical
global carrier. The exact merge protocol and storage crash contract are
specified in [Merge ledger](merge_ledger.md); formal and release evidence are
tracked separately in the
[Sumeragi V2 multilane closure ledger](sumeragi_v2_multilane_closure_ledger.md).

Every Blake2b identity in the relay protocol has a distinct, length-framed
domain: settlement payloads, FASTPQ claims, merge hints, fee-sponsor source
state, and fee-sponsor allocation claims cannot be reinterpreted across
contexts even when their canonical Norito payloads share a prefix.

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
8. Only after canonical Kura/WSV application is durable do the exact queue
   reservations transition through Commit and ForgetCommit; startup reconciles
   any interrupted suffix idempotently.
9. Transaction queries expose both ordinary and merge-carried transactions,
   with a proof binding each merge result to the compact carrier.

This separation permits lane committees to progress independently without
allowing network arrival order, machine speed, or local inventory to choose a
global state order.

## Durable ingress and proxy admission

The first-release Torii proxy request carries one explicit transaction-admission
mode: `QueuePlanSynced`. The mode and exact `QueuePlanAdmissionBindingV1`
participate in request identity and survive forwarding unchanged. An
authoritative coordinator acknowledges only with an exact `f + 1` certificate
over the accepted transaction, routing plan, admission context, enqueue time,
and durable journal record. Before returning public `202 Accepted`, the ingress
node validates that certificate against the exact request and persists its
canonical bytes in local Kura. That durable certificate is the public
acceptance boundary; it does not assert that the binding is already in WSV.
There is no deferred, metadata-marker, or legacy proxy admission branch.

The proposal-native certificate is control work, not ordinary transaction
execution. The `QueuePlanSynced` transaction's physical FIFO position and then
its immutable live-reservation ordinal remain a strict global cut until the
autonomous outcome is terminal; only the resulting authenticated lane payload
and certified merge may execute it. Candidate assembly and its work provider
exclude that role even in a single-route topology. Common block validation
rejects every external `QueuePlanSynced` entrypoint, and locked/recovered-body
ingress performs the same check before Kura retention, session publication, or
losing-reservation retirement can mutate ownership.

For `QueuePlanSynced`, a failure before network dispatch is definitely
unavailable. A lost or unverifiable response after P2P or HTTP dispatch is
returned as `queue_plan_journal_outcome_unknown` with the canonical
accepted-transaction hash so the caller can reconcile ownership. Once the
validated certificate is locally durable, later dissemination, Sumeragi wake,
or proposal-native WSV application failure cannot downgrade the known `202`.
Hedged candidates are reduced
deterministically: any valid indeterminate result dominates definite failures,
non-retryable failures dominate generic retryable failures, and candidate order
breaks ties. A remote indeterminate response is accepted only when its status,
reject header, Norito error envelope, and transaction hash all match; malformed
or unrelated evidence becomes `invalid_proxy_response`.

Proxy and fanout response memory is admitted before route execution.
`torii.query_fanout_max_retained_bytes` is an aggregate pool: one quarter
provides four bounded signed-query ingress slots, while the remaining three
quarters admit complete fanout working sets. A promoted request acquires its
fanout reservation before releasing ingress. Within that reservation, request
decoding, a retained singular result, one sequential current route result,
bounded Core builders, route bodies, and final encoding use deterministic
phase-derived ceilings; the public listener body limit is not itself reserved
per fanout. The singular comparison high-water divides the variable remainder
into seven equal units: request decode, retained first result, retained Core
builder, current result, destination frame, and two canonicalization scratch
units. Iterable fanout is therefore limited to the canonical Norito
identity forms that Core can scan into a bounded accumulator;
unsupported predicates, selectors, sorting, JSON fanout, or oversized frames
fail before exhaustive route materialization. HTTP bridge and public-dataspace
responses are streamed under the same configured body ceiling, with the
smaller contract-view ceiling taking precedence when applicable.

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

Malformed ownership markers, manual occupants of reserved IDs, disabled
autoscale state, future creation heights, off-default dataspaces, or catalog
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
- has ordinary queued work, live reservations, delayed work, a pending merge
  entry, or an unmerged admissible relay/certified autonomous bundle;
- has a certified lane block without a matching global application receipt;
- has an unrepaired application marker;
- has an unapplied or unverifiable Native participant control, including a
  per-height receipt whose exact finality, per-height manifest proof,
  descriptor-bound latest pointer, checkpoint, or application block no longer
  revalidates;
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
markers are reset at the same incarnation boundary. Native participant
receipts and application manifests/proofs are immutable, versioned per-height
files; their route/incarnation latest value is a separate descriptor-bound,
replaceable derived pointer. All three are included in disk accounting,
archive validation, retirement, purge, and recreation allowlists. The archive
scanner enforces the configured retained-record count and existing shared
Native sidecar aggregate-byte budget and requires the exact
finality/manifest/receipt/latest-pointer join. Obsolete dense data/index
layouts and malformed, oversized, ambiguous temporary, unexpected,
non-regular, hardlinked, or symlinked evidence fail closed. Old files remain
historical proof material only where policy requires them; they cannot
authorize or route new work.

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

The V1 proposal, ownership, descriptor, vote, availability-QC, lane-QC, and
certificate JSON layouts are closed and exact. Nullable predecessor, carrier,
and availability-QC slots are always present as either their canonical value or
an explicit `null`; omitting a slot or adding an unknown field is a malformed
first-release message, not an older layout to infer.

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
Its settlement collections and aggregate fields are required even when empty or
zero, and its optional swap slot is encoded explicitly as a value or `null`.
The aggregate quantities equal the exact sums of the ordinary receipts, and
`tx_count` equals (rather than merely bounds) the union of source IDs across
ordinary, Nexus-fee, and Native-AMX receipts. Consequently, an empty receipt
union requires zero aggregate quantities and `tx_count = 0`.
Each ordinary receipt contains an exact-width `source_id`, exact canonical
decimal `local_amount`, `xor_due`, `xor_after_haircut`, and `xor_variance`
quantities, plus `timestamp_ms`. Commitment totals use the corresponding
`total_local_amount`, `total_xor_due`, `total_xor_after_haircut`, and
`total_xor_variance` quantity fields. The ordered `nexus_fee_receipts` cover
lane-relay-burn XOR fees; each ordered `native_amx_receipts` entry binds the
source transaction, coordinator context, routing-plan digest, and every
participant prepare/commit leg.
`LaneRelayEnvelope` carries that commitment and its hash alongside the lane
header, lane QC, DA commitment, RBC byte count, manifest root, and FastPQ proof
metadata. The lane QC authenticates the header and finality roots; it does not
by itself authenticate the settlement, descriptor, or FastPQ metadata.
The V1 envelope JSON layout is closed: every nullable proof/commitment slot is
present explicitly as a value or `null`, and unknown or omitted fields are
rejected instead of being interpreted as a pre-release layout.
The header height is the global proposal and authority context used for
lifecycle, committee, key-history, and policy checks. The envelope and
settlement `block_height` is the incarnation-scoped lane-local coordinate used
for relay identity and contiguous merge progression; a recreated lane therefore
restarts at lane-local height 1 without reusing its retired incarnation.

A relay becomes merge-authoritative only after all structural, committee,
signature, DA, settlement, and activation checks pass and an exact
contract-persisted record proves the FastPQ effect claim over that same
envelope. Nonzero proof metadata in the gossip cache is progress data only and
cannot create a merge candidate or block lane retirement. Verified relay
records are revalidated before hydrating the runtime cache. Envelope identity
includes immutable header, descriptor, DA, settlement, RBC, and manifest
fields, so a later “upgrade” cannot overwrite drifted evidence.

`RegisterVerifiedLaneRelay` is permissionless transport, not permissionless
authority. Before either contract-state key is written, Core derives the exact
lane committee from the transaction's consensus snapshot, requires commit
quorum, checks the QC roster and epoch, and verifies the aggregate BLS signature
with the committee's on-chain proofs of possession. It separately verifies the
FastPQ proof and its effect-specific claim digest over the exact descriptor,
settlement, QC, DA, and manifest fields before persisting the record. Pending or
metadata-only envelopes remain valid for local status/progress only and cannot
create durable contract-visible settlement state or merge authority.

Relay settlement candidates contain the exact envelope. Merge validators
preflight duplicate markers, fee schedule arithmetic, canonical asset
selection, aggregate burns, and payer balances before signing. Settlement is
then staged on the same pristine carrier overlay and is idempotent across crash
replay.

## Native AMX cross-dataspace transactions

Native-AMX control, attestation, and participant-receipt validation remain live
in both the ordinary global-body path and reservation-bound autonomous
payloads. A routing plan names its coordinator and every participant leg. The
producer must collect the required participant prepare/commit QCs;
coordinator-only evidence is not synthesized.
This ordinary Native-AMX path does not relax QueuePlan admission: a transaction
whose signature-bound intent is `QueuePlanSynced` is autonomous-only regardless
of whether its routing plan also contains Native-AMX participant legs.
On the ordinary path the authenticated global leader owns these requests. In
reservation-bound lookahead, only the independently frozen deterministic lane
author may issue them, including when that author is outside the current global
roster; a global leader cannot pre-empt that lane slot. Exactly one active lane
route owns autonomous Native-AMX coordination in each global view, using the
committed route order rotated by `(height + view)`. Other autonomous Native
coordinators retain their reservations until their turn, preventing concurrent
authors from splitting participant-slot votes between incompatible proposals.
One shared predicate determines whether a receipt leg requires separate
participant application. Validation, Kura sidecars, State frontiers, startup
repair, diagnostics, drain checks, and retirement all use it. A coordinator leg
whose participant route is the same route does not create a separate marker,
receipt, or application frontier.
`NativeAmxReceipt.authority_context_height` binds the global application
context, while each leg retains its lane-local height. Validation checks chain,
source ID, typed entrypoint hash, routing-plan digest, lane/dataspace roles,
authority height, participant committees, QCs, grouped bounds, and duplicate
sources before state execution.
The participant `LaneBlockProposalV1` keeps its canonical proposal-level
`payload_block_hint` key in Torii JSON. Because Native AMX legs are control-only,
that required key is always the explicit value `null`; a missing key, a non-null
hint, or any unknown proposal field is malformed. OpenAPI and every maintained
SDK decoder enforce the same closed shape.

All entrypoints in one merge batch execute in canonical order on one revertible
overlay. Any divergence in results, settlement evidence, write-set roots, or
expected post-state hash invalidates the complete candidate.
The global execution commitment also carries the canonical Native application
manifest root. Per-route leaves and Merkle proofs bind the active incarnation,
predecessor, proposal, settlement, ordered sources/results, and application
block. For each participant height, Kura publishes one immutable versioned
manifest file and then one immutable versioned receipt file with create-new,
no-clobber promotion and exact durable readback. It then replaces the
descriptor-bound route/incarnation latest pointer with the exact receipt
identity and advances the participant frontier only after that complete
finality/manifest/receipt/latest-pointer join is durable. Startup reconstructs
the pointer from the bounded standalone files; steady-state lookup does not
reverse-scan history.

The standalone manifest and receipt histories share the configured Kura
sidecar-retention count and the existing Native sidecar aggregate-byte budget,
with at most one bounded transient publication slot. Pair compaction first
fsyncs a versioned prune intent bound to lane, dataspace, incarnation, and
every `(kind, participant height, artifact hash)` removal. Restart recovers a
temporary-only intent, a stable intent before unlink, every individual
manifest/receipt unlink stage, a fully unlinked pair, and identical stable plus
temporary intent files idempotently; it never prunes the exact latest pair.
Valid lone publication temporaries are promoted, byte-identical duplicates
are removed, and malformed, conflicting, oversized, unexpected, legacy-dense,
non-regular, hardlinked, or symlinked material is rejected before mutation.
After body pruning, a QC-authenticated manifest proof is sufficient; legacy
hash-only evidence remains blocked unless authenticated storage or QC signers
recover the canonical executed wire.

## Compact carrier, recovery, and proofs

The full merge entry can be up to 16 MiB; the global block instead carries a
`CertifiedMergeLedgerReference`. Its merge QC identifies authenticated sidecar
holders for settlement and autonomous execution entries alike. Fetch uses
bounded 64-KiB chunks and global/per-peer resource caps, while authoritative
pending blocks retry through holder withholding without a fixed attempt
horizon. Responders materialize only the exact hash-addressed, height-context
authenticated entry for which they are a certified holder.

Kura maintains indexed full-entry and carrier stores. A node that restarts with
any live compact carrier, including an autonomous execution carrier, can fetch
the exact bounded sidecar from authenticated merge-QC holders. It re-executes
the certified entry against the exact current base state and accepts it only
when ordered results, write set, post-state, and batch hash match. Historical
execution reconstructs transaction history only from its already-durable exact
full entry. Torn, oversized, non-canonical, conflicting, symlinked, or
future-uncommitted storage is truncated or rejected according to the crash
boundary; incomplete network assemblies are never persisted.

Chain truncation publishes a fsynced prune intent before lowering the durable
block marker. Carrier/log, WSV-checkpoint, commit-manifest,
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

`/v1/sumeragi/status` exposes only the authoritative
`SumeragiV2Status`. Operational lane evidence is returned separately by
`/v1/sumeragi/diagnostics`, including proposals/commitments, relay envelopes,
payload ownership, committed lane sessions, configured lane/dataspace geometry,
autoscale transition data, and the bounded, route/incarnation-ordered Native
participant-application vector. The Native evidence records are reconstructed
from State and Kura evidence; conflicting same-height identities are reported
as `conflict` rather than silently selected. The bounded
`autonomous_lane_executions` vector reports restart-stable progress from
reservation durability through lane certification, merge/carrier application,
and queue finalization using durable State/Kura plus queue ownership evidence;
it is evidence, not consensus authority.

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

Production release requires the following corridor. Every gate in this list is
currently open until fresh artifacts satisfy it; a skipped required test is a
failure:

- unit tests for router activation/incarnation boundaries, committee and quorum
  authority, lane proposal/QC aggregation, payload recovery, merge
  re-execution, fee preflight, lifecycle atomicity, cleanup, and recovery;
- negative tests for wrong leader/round/parent/roster, equivocation, duplicate
  signatures, lost signing locks, malformed/corrupt chunks, oversized counts,
  stale/future lanes, forged/under-quorum drain certificates, post-close work,
  same-carrier retirement, delayed pre-close work loss, unsafe retirement,
  Native retained-count and aggregate-byte overflow, malformed or oversized
  publication temporaries, legacy dense evidence, and every Native
  temporary/stable/partial-unlink prune crash boundary;
- four-peer localnet autoscale expansion/contraction, repeated cycles, a
  full intent/certificate/later-retirement cycle, a certified elastic-lane merge
  with one offline/missing-sidecar peer, WSV/query proof convergence, and
  repeated restart idempotency;
- 13-peer global cross-dataspace/native-AMX/autonomous integration, with twelve
  lane-validator assignments across at least
  three independent four-validator dataspaces, 10/10 fresh deterministic
  seeds, rotating outages/restarts, scale-out/drain/scale-in/same-ID
  recreation, full convergence, and zero lost, rejected-after-acceptance, or
  duplicate transactions;
- a separate two-hour 13-peer global fault soak;
- five paired pinned-hardware one-versus-four-lane runs demonstrating at least
  1.5× median committed throughput and no worse than 1.25× p95 latency at
  matched offered load, within all configured resource bounds;
- TLC/Apalache models for autoscale lifecycle, pinned-incarnation authority
  under independent current-roster rotation, merge execution order, and exact
  merge-carrier safety, each with expected-failure mutations; and
- the prescribed isolated-target, locked/offline focused and SDK suites,
  formal runners, full workspace build/test, strict workspace Clippy,
  formatting, and legacy-codec guard.

This architecture description records current behavior, not a claim that the
four-peer, 13-peer global, soak, scaling, or full-workspace release evidence has
already passed.

Primary code lives in `crates/iroha_core/src/state.rs`,
`crates/iroha_core/src/lane_consensus.rs`,
`crates/iroha_core/src/lane_drain.rs`,
`crates/iroha_core/src/sumeragi/v2_lane_work.rs`,
`crates/iroha_core/src/sumeragi/v2_candidate.rs`,
`crates/iroha_core/src/sumeragi/v2_apply.rs`,
`crates/iroha_core/src/merge_sidecar.rs`, and
`crates/iroha_core/src/kura.rs`; canonical DTOs live under
`crates/iroha_data_model/src/{block,merge,nexus,query}`.
