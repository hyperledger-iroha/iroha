---
title: Sumeragi multilane and autoscale lanes
sidebar_label: Multilane
description: Lanes as independent Sumeragi instances merged by the global chain, and deterministic lane autoscaling.
---

# Sumeragi multilane and autoscale lanes

Status: design, 2026-09-28; normative once implemented (goal S5 for lanes). Dataspaces with their
own state and cross-dataspace atomicity are specified in `sumeragi.md` §11 (goal S6).

## 0. Summary

A **lane** adds dissemination and certification capacity to the one global world state. Every lane
incarnation is a separate Sumeragi core instance with its own committee, pinned at creation. A
lane block is an ordered batch of transactions that the lane committee has checked for
*admission* (signature, size, routing, deduplication) against the global state at an **anchor**
height; a lane never mutates world state. The global chain `G` **merges** certified lane blocks by
reference: a `G` block names, per lane, the next contiguous certified lane heights, and executing
the `G` block executes those transactions against the world state in one canonical order. `G` is
the only place state changes, so transactions on different lanes need no cross-lane protocol:
their atomicity and ordering are those of the `G` block that merges them.

**Autoscale** is a deterministic rule in `G`'s execution. From committed load samples it opens a
new elastic lane (a fresh incarnation with a pinned committee, active two `G` heights later) or
closes the highest one. Closing is one committed height `c`; the **freshness bound** `A` makes
every lane block anchored before `c` unmergeable after `G` height `c + A`, so the lane's final
frontier is fixed at `c + A` with no drain vote, and the lane retires at `c + A + 1`.

Why this shape:

- The core is reused unchanged: a lane is just another instance (`I`, committee, parameters), so
  lane safety and liveness are the core's (`sumeragi.md` §7–§8).
- One world state, one execution order: no merge QC, merge leader, sidecar custody, relay
  settlement or drain certificate is needed, because `G`'s own CommitQC orders the merge.
- Every node follows every public lane (member or observer), so the bodies `G` executes are in
  every node's local lane stores through ordinary core sync.

## 1. Terms

- `G` — the global instance: the node's existing Sumeragi instance, height `h`.
- Lane `ℓ` — a `LaneId` of the committed lane catalog. Lane `0` (the anchor lane) is sequenced by
  `G` itself and has no instance. Every other active lane has one instance per incarnation.
- Incarnation `ι` — `H("iroha/lane/incarnation/v1" ‖ network_id ‖ be32(ℓ) ‖ be64(dataspace) ‖
  be64(creation_height) ‖ be64(creation_counter))`; never reused.
- Lane instance id `I_ℓ,ι = H("iroha/sumeragi/instance/lane/v1" ‖ network_id ‖ be32(ℓ) ‖ ι)`.
- Lane height `x` — height in the lane instance; lane genesis is `x = 0` (§2.3).
- Anchor `a(B)` — the `G` height named by lane block `B` (§3.2).
- `A` — freshness bound, a `G` chain parameter (`lane_anchor_freshness`, default 16).

## 2. Lane lifecycle in `G`'s state

### 2.1 Lifecycle record

World holds, per lane, a `LaneLifecycleRecord`:

```text
LaneLifecycleRecord {
    lane: LaneId, dataspace: DataSpaceId, incarnation: [u8; 32],
    profile: LaneProfile,                 // routing class + lane chain parameters (block_time, max_block_bytes, ...)
    committee: Vec<(PeerId, pop)>,        // ordered, pinned for the whole incarnation
    created_at: u64,                      // G height of the creating block
    active_from: u64,                     // created_at + 2
    closing: Option<u64>,                 // G close height c
    merged: LaneFrontier,                 // highest merged lane height and its block hash / R
}
```

The record, including the committee and profile, is committed in `R` (the world-state root) of the
`G` block that writes it. A fixed lane (from the configured catalog) is created by genesis or by a
governance transaction; an elastic lane by autoscale (§6). Lane `0` has no record.

### 2.2 States

`Created (G height < active_from) → Active → Closing (c set) → Retired (record removed)`.

- **Active** from `G` height `active_from`: nodes start the lane instance when they apply that
  height (§4.1); the lag of two gives every node the record before the first lane block can be
  merged.
- **Closing** from the `G` block that sets `c` (autoscale scale-in, stall close §6.4, or a
  governance close of a fixed lane).
- **Retired** at `G` height `c + A + 1` (§6.3): the record is removed and the frontier is final.

### 2.3 The lane instance's configuration

The core needs `C_{x}`, `ChainParams_x` for every lane height and an `Init` (§12 of the core
spec). For a lane they are constants of the record:

- `C_{ℓ,x} = committee` for all `x` (BLS-normal keys with the pinned proofs of possession).
- `ChainParams_x = profile.chain_params` for all `x`; `R_x` still commits them (they never change).
- Lane genesis `x = 0`: `block_hash_0 = H("iroha/lane/genesis/v1" ‖ I_ℓ,ι)`,
  `R_0 = H("iroha/lane/genesis-result/v1" ‖ norito(record at creation))`.

A committee therefore cannot change within an incarnation; replacing members means closing the
lane and creating a new incarnation. A pinned member that loses its key stalls only this
lane, and §6.4 closes a stalled lane.

## 3. Lane blocks

### 3.1 Payload

A lane block's payload is the canonical Norito encoding of

```text
LaneBatch { anchor_height: u64, anchor_hash: Hash, transactions: Vec<SignedTransaction> }
```

It is never empty (work-driven, `sumeragi.md` §6.10): the lane's payload builder answers `EMPTY`
when the lane queue partition (§5) has nothing admissible.

### 3.2 Admission execution (execute-before-vote for lanes)

A lane member executes a lane block `B` at lane height `x` as follows; `Valid(R)` or `Invalid`:

1. The anchor is a committed `G` block the node has applied: `anchor_hash` is the hash of `G`
   height `anchor_height`. If the node has not applied that height yet, execution is **pending**
   (the executor answers when it has; the core allows execution to take time, §6.3). If the node
   applied that height with another hash, `Invalid`.
2. Monotone anchors: `anchor_height ≥` the anchor of lane block `x − 1` (from its `R` preimage).
3. The lane is active at the anchor: `active_from ≤ anchor_height` and not
   (`closing = Some(c)` with `anchor_height ≥ c`).
4. Every transaction: canonical encoding, size and signature limits, chain id, not expired at the
   anchor's block time, and **routes to this lane** under the committed routing state of the
   anchor (§5.1).
5. No transaction hash appears in lane blocks `x − W .. x − 1` or earlier in `B`
   (`W = profile.dedup_window`, the preimages carry the hashes).

`R_x = H("iroha/lane/result/v1" ‖ norito(LaneResult))` with
`LaneResult { anchor_height, anchor_hash, tx_hashes, payload_bytes, next_committee_digest, next_params }`;
`next_*` are the pinned constants (§2.3), so the core's lag-2 rule holds trivially.

Admission is a pure function of the lane block and committed `G` state; it never reads or
writes world state beyond the anchor's committed routing and catalog, so a lane cannot change
state and every honest lane member computes the same `R_x`.

## 4. Merging lanes into `G`

### 4.1 Nodes follow every lane

Every `G` node runs every active public lane's instance: as a member when its validator key is in
the pinned committee, otherwise as an observer (the core commits from CommitQCs and syncs bodies
as an observer, `sumeragi.md` §6.9). Lanes of restricted dataspaces are followed by the nodes the
dataspace policy admits; `G` merges only lanes its validators follow (public lanes in the first
release). A node starts a lane instance when it applies `G` height `active_from` and stops it after
applying the retirement height.

### 4.2 The `G` payload

A `G` block payload carries, besides its ordinary transactions (lane 0):

```text
LaneMerge { lane: LaneId, incarnation: [u8; 32], from: u64, to: u64, tip_hash: Hash, tip_result: Hash }
```

for each lane with new certified blocks: the next contiguous lane heights `from = merged + 1 ..= to`
that the leader's node has committed. Lanes appear in ascending `LaneId`. Limits: at most
`max_lane_blocks_per_merge` lane blocks per lane and `max_merge_bytes` of lane payload per `G`
block (chain parameters), so a `G` block's execution stays within the execution budget.

### 4.3 Merge execution

`G`'s executor, for a `G` block at height `h`, after its ordinary preamble:

1. For each `LaneMerge` in order, the node's lane instance must have committed heights
   `from ..= to` with `block_hash(to) == tip_hash` and `R_to == tip_result`; otherwise the
   execution is pending until it has (lane sync fetches them; a lane CommitQC for them exists, so
   bodies are available from the lane committee). A `LaneMerge` whose lane is not active, whose
   `from` is not `merged + 1`, or which violates a limit makes the `G` block `Invalid`.
2. **Freshness:** every merged lane block's anchor satisfies `anchor_height ≥ h − A`, and, if the
   lane is closing with `c`, `anchor_height < c`. A violating block makes the `G` block `Invalid`.
3. The transactions of the merged lane blocks are executed in order — lanes ascending, lane heights
   ascending, transactions in batch order — after lane 0's transactions, exactly as if they were
   one ordered list in the `G` block. Each transaction's outcome (accepted / rejected with reason)
   is committed in `R_h` like any other. Admission at the lane does not guarantee acceptance: a
   transaction may fail at merge (state changed since the anchor), and is then rejected with a
   fee as usual.
4. The lane records' `merged` frontiers advance to `to`, and the load samples of §6.1 are updated.

**Parallel execution.** Step 3 fixes the *result*: the canonical serial order above. The executor
may run merged lane batches concurrently with the deterministic parallel scheduler, which commits
a transaction only in canonical order and re-executes it on a read/write conflict with an earlier
one, so the result equals the serial order on every node and every machine. Lane key prefixes
(`LaneConfig::key_prefix`) and authority-based routing (§5.1) keep most lane batches disjoint, so
adding lanes adds execution parallelism as well as dissemination and certification capacity;
transactions that touch several lanes' state serialize on the conflict, never reorder.

A certified lane block that `G` never merges (stale anchor, closed lane) has no effect; its
transactions stay in the nodes' queues (§5.3) and are proposed again.

### 4.4 Why this is safe

- `G`'s CommitQC fixes the merge set and order; every honest `G` node executes the same
  transactions in the same order (the lane blocks are fixed by their lane CommitQCs, and the tip
  hash and result bind them). World state therefore stays a deterministic function of `G`'s
  committed chain.
- A lane's safety (one block per lane height) comes from its own core instance; `G` validators
  check nothing about lane signatures beyond their own nodes' lane commits, which are the core's
  commits under the pinned committee.
- A Byzantine `G` leader can only choose *how much* of each lane to merge (not reorder or alter
  it); honest leaders merge everything available, so every certified, fresh lane block is merged
  within one honest `G` leader's turn.

### 4.5 Storage and sync

The node's Kura stores `G` blocks as today and, per lane incarnation, a lane block store with one
certified frame per lane height (the same `KuraBlockStore` shape, under the lane instance's
storage identity). A `G` frame is committed only after the lane frames it merges are durable
(append order), so replay after restart re-executes `G` blocks from local lane frames. A syncing
node syncs each lane instance alongside `G` and executes a `G` block once its lanes reach the
merged tips; lane frames of retired incarnations are kept while any retained `G` block references
them and pruned with those `G` blocks.

## 5. Routing and the queue

### 5.1 Routing function

`route(tx, state) -> LaneId` uses only committed state: explicit routing rules (fixed lanes) first;
otherwise the default route is sharded over lane 0 and the **active, non-closing** elastic lanes by
`H(tx.authority) mod k` (authority-based sharding keeps one account's transactions in one lane,
preserving their nonce order). A node routes new transactions with its latest applied `G` state;
lanes validate routing at their anchor (§3.2 step 4). A transaction routed with a newer state than
the lane's anchor may be refused and is re-routed.

### 5.2 Queue partitions

The transaction queue keeps one FIFO partition per lane (lane 0 is `G`'s). Gossip, admission
limits and expiry are per node as today; the partition is derived by `route` when a transaction
is admitted and recomputed when the catalog changes (after a lane opens or closes).

### 5.3 Retirement from the queue

A transaction leaves the queue when a `G` block that executed it (merged or directly) is applied.
It is not removed when a lane block containing it commits: until `G` merges that block the
transaction is still pending, and if the block is never merged it is proposed again. The lane
payload builder skips transactions already in committed-but-unmerged lane blocks of that lane.

## 6. Autoscale

All rules run in `G`'s execution with committed data only; every node computes the same decision.
Parameters are the committed `nexus.autoscale` values (range `[min_lane_id, max_lane_id_exclusive)`,
windows, ratios, cooldown, per-lane target load); at most one lifecycle transition per `G` block.

### 6.1 Samples

Each `G` block appends a sample `{h, block_time_ms, merged_tx_count, lane0_tx_count,
per-lane merged bytes}` to a bounded window in World. Utilization of the lane set is
`(merged + lane-0 transactions in the window) / (window blocks × active lanes × per_lane_target)`;
latency is the p95 of `block_time_ms` in the window (the `G` block time, which rises when `G`'s
execution is saturated).

### 6.2 Scale out

When the scale-out window shows `p95 ≥ scale_out_latency_ratio × target_block_ms` or
`utilization ≥ scale_out_utilization_ratio`, the cooldown has expired and a free elastic id exists,
execution creates a lane record: the lowest free id, the default dataspace and public profile, a
fresh incarnation, and the committee `select_committee(dataspace, h)` — the dataspace's validator
pool (stake-elected for public dataspaces, the manifest cohort otherwise), exactly `n` members
(`n` = the dataspace's committee size), ordered canonically, each with its registered BLS-normal
key and PoP. If the pool cannot fill `n`, no lane is created.

### 6.3 Scale in and retirement

When the scale-in window shows both ratios below their scale-in thresholds and the cooldown has
expired, execution sets `closing = Some(h + 1)` on the highest active elastic lane. From `c`:
routing excludes the lane (§5.1), lane members refuse blocks anchored at or after `c` (§3.2
step 3), and `G` merges only its blocks anchored before `c` (§4.3). At `G` height `c + A` every
still-unmerged block of the lane is stale, so its frontier is final; at `c + A + 1` execution
removes the record and the lane retires. No drain vote or certificate exists: `G` alone decides
the frontier.

### 6.4 Stalled lanes

A lane that is active for at least `stall_window` `G` blocks, has routed load (transactions
routed to it were observed in `G` blocks — i.e. its queue share is non-zero in committed samples)
and has merged nothing within the window is closed exactly like a scale-in (fixed lanes too), so
a lane whose pinned committee lost its quorum cannot hold transactions forever.

## 7. Fees and settlement

Lane transactions pay fees at merge execution like any transaction (the fee model of
`nexus_fee_model.md`); per-lane settlement buffers and fee schedules are profile fields applied by
the executor. There are no lane relay envelopes, settlement receipts or FASTPQ relay proofs: `G`
executes the transactions itself.

## 8. Status and telemetry

`/v1/sumeragi/status` gains per-lane entries (lane, incarnation, instance status DTO, merged
frontier, closing height). Lane instances report through the same observer as `G`, labelled by
instance id.

## 9. What lanes do not have

There is no lane-level vote on world state, no second (merge) quorum certificate, no merge leader,
no side-channel custody of lane bodies, no drain vote or certificate, no relay envelope or relay
settlement proof, and no per-transaction queue reservation protocol: `G`'s CommitQC orders the
merge, the lane instances' own commits make lane bodies available to every node, and the closing
height plus the freshness bound fix a closing lane's frontier. Cross-lane atomicity is that of the
`G` block; atomicity across dataspaces with separate state is §11's two-phase commit. The lane and
dataspace catalogs, `LaneConfig` geometry (key prefixes, visibility, storage profile), routing
rules, the `nexus.autoscale` parameters, lane validator pools and committee selection, and the
Kura per-instance storage identity are the inputs this design builds on.

## 10. Implementation plan

1. **Multi-instance node (S5 core):** the node runs `G` plus one driver instance per active lane
   (per-instance records dir, bodies store, block store, observer labels); `net.rs` routes frames
   by instance id; O9 isolation tests.
2. **Lifecycle state:** `LaneLifecycleRecord` in World, genesis/governance creation of fixed lanes,
   lane `Init`/configuration from the record, start/stop of instances on `G` apply.
3. **Lane executor and payload builder:** `LaneBatch`, admission execution and `R`, queue
   partitions and routing.
4. **`G` merge:** `LaneMerge` payload section, pending-until-committed execution, freshness,
   canonical execution order, frontier update; Kura append order and replay.
5. **Autoscale:** samples, scale-out committee selection, scale-in/close/retire, stall close.
6. **Status, telemetry, Torii and SDK DTOs; kagami/localnet lane catalogs.**
7. **Tests:** multi-instance node tests (two lanes, merge order, restart), autoscale unit tests
   (deterministic decisions from sample windows), a four-peer network test with elastic scale-out
   and scale-in under load, and simulator coverage of a lane instance next to `G`.
