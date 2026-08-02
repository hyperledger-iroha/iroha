---
title: Nexus Lane Model
sidebar_label: Lane Model
description: Logical lane taxonomy, lane configuration geometry, and world-state merge rules for SORA Nexus.
---

# Nexus Lane Model & WSV Partitioning

> **Status:** NX-1 production architecture — lane geometry, lifecycle storage,
> autonomous execution, and canonical merge are implemented; multilane release
> evidence remains open.
> **Owners:** Nexus Core WG, Governance WG  
> **Related roadmap item:** NX-1

This document captures the production architecture for Nexus’ multilane
consensus layer. It produces one deterministic world state while allowing
individual data spaces (lanes) to run public or private validator sets with
isolated workloads.

> **Cross-lane proofs:** This note focuses on geometry and storage. The per-lane settlement commitments, relay pipeline, and merge-ledger proofs required for roadmap **NX-4** are spelled out in [nexus_cross_lane.md](nexus_cross_lane.md).

## Concepts

- **Lane:** Logical shard of the Nexus ledger with its own validator set and execution backlog. Identified by a stable `LaneId`.
- **Data Space:** Governance bucket grouping one or more lanes that share compliance, routing, and settlement policies. Each dataspace also declares `fault_tolerance (f)` used to size lane-relay committees (`3f+1`).
- **Lane Manifest:** Governance-controlled metadata describing validators, DA policy, gas token, settlement rules, and routing permissions.
- **Global Commitment:** Proof emitted by a lane summarising new state roots, settlement data, and optional cross-lane transfers. The global NPoS ring orders commitments.

## Lane Taxonomy

Lane types canonically describe their visibility, governance surface, and settlement hooks. The configuration geometry (`LaneConfig`) captures these attributes so nodes, SDKs, and tooling can reason about the layout without bespoke logic.

| Lane type | Visibility | Validator membership | WSV exposure | Default governance | Settlement policy | Typical use |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | public | Permissionless (global stake) | Full state replica | SORA Parliament | `xor_global` | Baseline public ledger |
| `public_custom` | public | Permissionless or stake-gated | Full state replica | Stake weighted module | `xor_lane_weighted` | High-throughput public applications |
| `private_permissioned` | restricted | Fixed validator set (governance approved) | Commitments & proofs | Federated council | `xor_hosted_custody` | CBDC, consortium workloads |
| `hybrid_confidential` | restricted | Mixed membership; wraps ZK proofs | Commitments + selective disclosure | Programmable money module | `xor_dual_fund` | Privacy-preserving programmable money |

All lane types must declare:

- Dataspace alias — human-readable grouping that binds compliance policies.
- Governance handle — identifier resolved through `Nexus.governance.modules`.
- Settlement handle — identifier consumed by the settlement router to debit XOR buffers.
- Optional telemetry metadata (description, contact, business domain) surfaced
  through `/v1/nexus/lifecycle`, metrics, and dashboards.

## Lane Configuration Geometry (`LaneConfig`)

`LaneConfig` is the runtime geometry derived from the validated lane catalog. It does **not** replace governance manifests; instead it provides deterministic storage identifiers and telemetry hints for every configured lane.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` recomputes the geometry whenever configuration is loaded (`State::set_nexus`).
- Aliases are sanitised into lowercase slugs; consecutive non-alphanumeric characters collapse into `_`. If the alias yields an empty slug we fall back to `lane{id}`.
- Key prefixes ensure the WSV keeps per-lane key ranges disjoint even when the same backend is shared.
- `shard_id` is derived from the catalog metadata key `da_shard_id` (defaulting to `lane_id`) and drives the persisted shard cursor journal to keep DA replay deterministic across restarts/resharding.
- Kura segment names are deterministic across hosts; auditors can cross-check segment directories and manifests without bespoke tooling.
- Merge segments (`lane_{id:03}_merge`) hold the latest merge-hint roots and global state commitments for that lane.
- When governance renames a lane alias, nodes automatically relabel the corresponding `blocks/lane_{id:03}_{slug}` directories (and tiered snapshots) so auditors always see the canonical slug without manual cleanup. If the target Kura segment already exists, the config/lifecycle transition fails before catalog or tiered-state changes are committed.

## World-State Partitioning

- The logical Nexus world state is the union of per-lane state spaces. Public lanes persist full state; private/confidential lanes export Merkle/commitment roots to the merge ledger.
- MV storage prefixes every key with the 4-byte lane prefix from `LaneConfigEntry::key_prefix`, yielding keys such as `[00 00 00 01] ++ PackedKey`.
- Shared tables (accounts, assets, triggers, governance records) therefore store entries grouped by lane prefix, keeping range scans deterministic.
- Merge-ledger metadata mirrors the same layout: each lane writes merge-hint roots and reduced global state roots to `lane_{id:03}_merge`, allowing targeted retention or eviction when a lane retires.
- Cross-lane indexes (account aliases, asset registries, governance manifests) store explicit `(LaneId, DataSpaceId)` pairs. These indexes live in shared column families but use the lane prefix and explicit dataspace ids to keep lookups deterministic.
- The merge workflow combines public data with private commitments using `(lane_id, dataspace_id, height, state_root, settlement_root, proof_root)` tuples derived from merge-ledger entries.

## Kura & WSV Partitioning

- **Kura segments**
  - `lane_{id:03}_{slug}` — primary block segment for the lane (blocks, indexes, receipts).
  - `lane_{id:03}_merge` — merge-ledger segment recording reduced state roots and settlement artefacts.
  - Global segments (consensus evidence, telemetry caches) remain shared because they are lane-neutral; their keys do not include lane prefixes.
- Runtime watches lane catalog updates: newly added lanes have their block and merge-ledger directories provisioned automatically under `kura/blocks/` and `kura/merge_ledger/`, while retired lanes are archived under `kura/retired/{blocks,merge_ledger}/lane_{id:03}_*`.
- Tiered-state snapshots mirror the same lifecycle; each lane writes under `<cold_root>/lanes/lane_{id:03}_{slug}` where `<cold_root>` is `cold_store_root` (or `da_store_root` when `cold_store_root` is unset), and retirements migrate the directory tree to `<cold_root>/retired/lanes/`.
- **Key prefixes** — the 4-byte prefix computed from `LaneId` is always prepended to MV encoded keys. No host-specific hashing is used, so ordering is identical across nodes.
- **Block log layout** — block data, index, hashes, and the durable count marker (`blocks.count.norito`) are nested under `kura/blocks/lane_{id:03}_{slug}/`. Merge-ledger journals reuse the same slug (`kura/merge/lane_{id:03}_{slug}.log`), keeping per-lane recovery flows isolated.
- **Retention policy** — public lanes retain full block bodies;
  commitment-only lanes may compact older bodies only after the checkpoint,
  finality, commit manifest, and every required Native application
  manifest/proof, receipt, and latest-index binding are durable. After body
  pruning, Native evidence revalidates against the QC-authenticated manifest
  root; a hash-only legacy marker is not authoritative by itself. Confidential
  lanes keep ciphertext journals in dedicated segments to avoid blocking other
  workloads.
- **Tooling** — `cargo xtask nexus-lane-maintenance --config <path> [--compact-retired]` inspects `<store>/blocks` and `<store>/merge_ledger` using the derived `LaneConfig`, reports active vs retired segments, and archives retired directories/logs under `<store>/retired/...` to keep evidence deterministic. Maintenance utilities (`kagami`, CLI admin commands) should reuse the slugged namespace when exposing metrics, Prometheus labels, or archiving Kura segments.

## Storage Budgets

- `nexus.storage.local_budget_bytes` is the sole operator-facing aggregate on-disk budget for Nexus nodes. It covers Kura, cold WSV snapshots, SoraFS storage, and SoraNet/SoraVPN spools; zero and the pre-release `max_disk_usage_bytes` alias are rejected.
- The actual configuration keeps the operator value separate from an actual-only `effective_local_budget_bytes`. An explicit operator value initializes both. When the operator value is omitted, `irohad` derives only the effective value at every startup and never rewrites the TOML file.
- Runtime derivation groups canonicalized managed roots by live filesystem identity and computes each filesystem cap as `(managed_bytes + available_bytes) - ceil(total_bytes * 20%)` with checked arithmetic. Underflow, zero, overflow, or a result below current managed usage aborts startup instead of activating an unenforceable cap.
- Every subsystem disk weight must be non-zero and the five weights must sum to `10_000` basis points. Operator budgets too small to give every component a non-zero cap are rejected because zero means unlimited to the component enforcers. Runtime application requires non-empty, strictly ordered, globally unique component sets, rejects zero component shares, and recomputes the aggregate with checked arithmetic from the per-filesystem budgets rather than accepting separate aggregate metadata. Proportional products use exact `u128` intermediates so `u64::MAX` budgets cannot saturate into incorrect shares.
- A symbolic link or Windows reparse point is allowed only strictly above a configured managed root. The exact root, any descendant, and dangling links/reparse points fail closed. Accepted ancestors are canonicalized before grouping and deduplication, and filesystem identity is rechecked while measuring every managed tree; crossing a mount boundary is fatal.
- Explicit budgets receive the same structural filesystem validation. Only a genuine low-space comparison is warning-only: the warning accounts for already-managed bytes and reports when additional growth required by the configured component caps exceeds available space.
- `nexus.storage.budget_enforce_interval_blocks` sets how often (in committed blocks) the storage budget scan runs; set to 0 to enforce every block.
- When the global disk budget is exceeded, eviction is deterministic: prune SoraNet provision spools in lexicographic path order, then SoraVPN spools, then tiered-state cold snapshots oldest-first (offloading to `da_store_root` when configured), then Kura retired segments, and finally evict active Kura block bodies into `da_blocks/` for DA-backed rehydration on read. Blocks that exceed the Kura budget on their own are persisted directly into `da_blocks/` and indexed as evicted.
- `nexus.storage.max_wsv_memory_bytes` caps the hot WSV tier by propagating deterministic in-memory WSV sizing into `tiered_state.hot_retained_bytes`; grace retention may temporarily exceed the budget, but the overflow is observable via telemetry (`state_tiered_hot_bytes`, `state_tiered_hot_grace_overflow_bytes`).
- `nexus.storage.disk_budget_weights` splits the disk budget across components using basis points (must sum to 10,000). Operator-explicit aggregate budgets use the existing global split. Auto-derived budgets first assign a budget per filesystem, then normalize the same weights across only the components present on that filesystem before deriving `kura.max_disk_usage_bytes`, `tiered_state.max_cold_bytes`, `sorafs.storage.max_capacity_bytes`, `streaming.soranet.provision_spool_max_bytes`, and `streaming.soravpn.provision_spool_max_bytes`.
- Kura's storage budget enforcement sums block-store bytes across active +
  retired lane segments, including Native participant receipt,
  application-manifest/proof, and latest-index files, and includes queued blocks
  not yet persisted to avoid overshoot during write lag.
- SoraVPN provisioning spools use `streaming.soravpn` settings and are capped independently from the SoraNet provision spool.
- Per-component limits still apply: when a component has an explicit non-zero cap, the smaller of the explicit cap and the derived Nexus budget is enforced.
- Budget telemetry uses `storage_budget_bytes_used{component=...}` and `storage_budget_bytes_limit{component=...}` to report usage/caps for `kura`, `wsv_hot`, `wsv_cold`, `soranet_spool`, and `soravpn_spool`; `storage_budget_exceeded_total{component=...}` increments when enforcement rejects new data and logs emit a warning for the operator.
- DA eviction telemetry adds `storage_da_cache_total{component=...,result=hit|miss}` and `storage_da_churn_bytes_total{component=...,direction=evicted|rehydrated}` to track cache activity and bytes moved for `kura` and `wsv_cold`.
- Kura reports the same accounting used during admission (on-disk bytes plus queued blocks, including merge-ledger entry payloads when present), so the budget gauges reflect effective pressure rather than just persisted bytes.

## Routing & APIs

- Torii REST/gRPC endpoints accept an optional `lane_id`; absence resolves via
  `nexus.routing_policy.default_lane` / `default_dataspace`.
- Lane relay admission fails closed when the lane id is absent from the
  authoritative lane catalog, when the derived runtime geometry is missing or
  disagrees with the catalog dataspace, when the relay dataspace does not match
  that active lane binding, or when the dataspace is not present in the active
  dataspace catalog. `State::record_lane_relay`, verified lane relay
  registration, and lane-relay emergency override registration all use this
  catalog + geometry + dataspace agreement before accepting lane-scoped state.
  Missing dataspaces are reported separately from invalid validator rosters so
  operators can distinguish catalog drift from quorum shortages.
- Verified lane relay registration additionally shares the runtime relay
  finality verifier: it derives the canonical `3f+1` committee from the exact
  transaction snapshot, requires commit quorum, and verifies the aggregate BLS
  signature and proofs of possession. The QC mode tag is the strict canonical
  `lane-finality:v1` tag and signs a domain-separated statement containing the
  header hash, lane/dataspace/incarnation/height, DA commitment, standalone lane
  descriptor, manifest root, complete settlement commitment and hash, and RBC
  byte total. QC and proof carriers are excluded from that statement. The
  FastPQ proof is validity evidence, not authority: its old/new roots must equal
  the QC state roots, its transaction-set hash must equal the signed finality
  statement hash, and its DA and manifest commitments must equal the envelope.
  The submitting account is only a transporter; a missing or forged QC, a
  re-proved substituted settlement, or metadata-only FastPQ claim is rejected
  regardless of transaction authority and before contract state is mutated.
- `FindLaneRelayEnvelopeByRef` and runtime relay-cache hydration read verified
  relay records through the canonical `LaneRelayEnvelopeRef::relay_state_key()`
  and reject decoded records whose embedded `relay_ref` does not exactly match
  the requested or scanned key, so malformed contract state cannot spoof a
  different lane, dataspace, incarnation, or lane-local height under a valid
  key. The durable identity is exactly `(lane, dataspace, incarnation, height)`;
  a second finality effect at those coordinates is a conflict rather than a
  second settlement-hash-derived record.
- Merge-ledger commit validation and Space Directory-derived AXT policy snapshot
  derivation use the same active-lane agreement. A stale derived geometry entry
  cannot make a retired or removed lane merge-active, cannot select a target
  lane for AXT handle policy, and cannot populate cached AXT policy state when
  the authoritative catalog no longer contains that lane.
- DA commitment admission, DA pin-intent admission, Torii DA ingest proof-scheme
  resolution, Torii DA proof-policy endpoints, and block proof-policy hash
  validation are likewise derived from active catalog-backed Nexus lanes. Stale
  runtime geometry entries, removed catalog lanes, missing dataspaces, or
  proof/confidential-policy drift cannot make a retired lane acceptable for DA
  commitments, pin intents, ingest receipts, or block header proof-policy
  hashes.
- Torii DA commitment and pin-intent list/prove endpoints apply the same
  active-lane filter before exposing query results. List responses omit records
  whose lane is no longer present in the authoritative lane catalog, and
  targeted prove requests for stale runtime-only lanes return no proof even if
  old DA indexes still contain matching rows.
- Authoritative lane validator resolution also requires the lane to be present
  in the active lane catalog, to have matching derived runtime geometry, and
  to bind a dataspace present in the active dataspace catalog. Stale manifest
  bindings or `Active` public-lane validator records for retired, rebound,
  unknown, geometry-only, or malformed lanes are ignored by lane relay, block
  sync, Torii proxy authority checks, and the global NPoS epoch stake snapshot.
  When Nexus is enabled, live NPoS active-topology and roster-unavailability
  recovery selection also intersect validator-derived lane scopes with the
  active lane/dataspace catalogs. State-backed commit stake snapshot
  construction and roster-validation caches apply the same active-lane filter
  to stake weights, so a larger stale stake record from a retired, geometry-only,
  or unknown lane cannot override the weight from a valid active lane.
  State-backed QC and block-sync validation fallback paths use that filtered
  snapshot recomputation too, so a missing cached snapshot cannot revive stale
  unknown-lane stake during quorum checks. Live NPoS commit quorum status,
  local quorum-completion checks, commit-root signer selection, NEW_VIEW
  aggregation, and repair fanout/coverage telemetry now also pass the active
  lane set into world-backed stake quorum math.
- When autoscale adds managed elastic lanes for the default dataspace, ordinary
  no-target default traffic is sharded deterministically across the configured
  default lane plus valid elastic lanes using the transaction hash. The default
  lane remains part of the candidate set, and explicit rules, dataspace-targeted
  routes, settlements, and permission-scope routes keep their existing
  precedence. Valid elastic lanes must be public, autoscale-managed, correctly
  aliased, created at a positive height, and bound to the default dataspace.
  Their Torii route-authority set is inherited from the live commit topology
  after filtering for active consensus keys unless an explicit lane manifest
  binding exists, so consensus-created elastic lanes are immediately routable
  without giving ordinary public or restricted lanes a commit-topology fallback.
  Manifest validators and explicit bindings are exposed only when the declared
  validator list is duplicate-free, each binding names a declared validator, and
  the binding list has no duplicate validators or peers, so undeclared,
  duplicate validator, or duplicate peer entries cannot inflate route authority
  or public validator listings.
  The configured default lane itself must stay outside the autoscale-owned
  elastic id range so it remains a stable base anchor. Live-state routing also
  requires `nexus.enabled = true` and `autoscale.enabled = true` and filters
  managed elastic candidates to the configured
  `autoscale.min_lanes..autoscale.max_lanes` id range, so disabled Nexus,
  disabled autoscale, or corrupted out-of-range managed lanes cannot receive
  default traffic. If that active elastic range contains a manual lane,
  malformed autoscale-managed lane, or managed lane outside the default
  dataspace, live routing fails closed to the configured base default lane
  until the catalog is repaired. The integration router harness pins the same
  behavior at the public `ConfigLaneRouter::route_with_view` boundary, so
  in-range catalog corruption falls back to the base lane, stale managed lanes
  left in the catalog are ignored when either gate is disabled, and enabled
  autoscale still shards over valid elastic lanes.
  Block autoscale application also requires both enabled Nexus and enabled
  autoscale, so corrupted actual state with either gate disabled cannot create
  or retire elastic lanes. Autoscale catalog changes are staged inside the
  `StateBlock` and published to committed Nexus state and lane storage geometry
  only during `StateBlock::commit()` after transaction-height validation, so a
  transaction-height validation failure cannot publish a lane addition,
  retirement, runtime reset, or cooldown marker. Autoscale commit is serialized
  across transaction-height validation, lane storage reconciliation, and final
  publication: it first performs side-effect-free validation, then reconciles
  lane storage geometry outside the state writer lock, and only then publishes
  the committed Nexus catalog/runtime state under the writer lock. Manual lane
  lifecycle uses the same commit-serialization lock and lock order for geometry
  reconciliation and writer-locked catalog publication. World-backed cleanup for
  reset lanes runs after the block world overlay is committed, while state commit
  serialization is still active, so cleanup opens fresh committed-world
  transactions instead of re-entering through an uncommitted block overlay. DA
  commitment, shard/receipt cursor, confidential-compute receipt, pin-intent
  indexes, block-hash log entries, latest-header/query-index markers,
  commit-topology cells, and verified lane relay hydration prepared while
  applying a block are published only after a registered transaction block
  commits successfully, so those runtime, topology, and world indexes cannot
  leak from a block whose transaction commit later fails. Staged autoscale, DA,
  or verified-relay side effects with a missing inserted transaction block abort
  before geometry reconciliation, WSV cleanup, or relay-cache hydration.
  WSV-only commits without an inserted transaction block still persist their
  world mutations, but do not advance block metadata caches or query-index
  status.
  Commit also
  completes fallible autoscale geometry reconciliation before the transaction
  commit and publishes the autoscale catalog/runtime reset before staged DA
  indexes, so a storage failure while reconciling elastic-lane geometry cannot
  partially publish DA runtime, query state, or block-local WSV cleanup for an
  uncommitted block. Lane lifecycle and config-swap retirements use the same
  storage preflight barrier: a failed Kura or tiered retire preflight preserves
  the committed WSV rows that would otherwise be reset for the retiring lane.
  Lane-geometry reconciliation dry-runs both Kura block/merge storage and
  tiered-state snapshot geometry before either backend is mutated. Kura then
  fsyncs a Norito intent containing the complete before/after geometry and
  incarnation bindings before it renames, archives, or provisions any path;
  tiered-state reconciliation runs behind that durable barrier. A failure before
  catalog publication rolls Kura back to the prior fingerprint, while restart
  after publication rolls it forward. Archive collisions, path traversal,
  symlinks, non-regular files, stale incarnation markers, and partial block/log
  pairs fail closed. The same commit boundary applies to deterministic autoscale
  scale-out and scale-in, including staged DA indexes in the block: a Kura or
  tiered conflict for a new or retiring elastic lane aborts before tiered
  artifacts or DA runtime/query indexes are published.
  Production startup authenticates the process-configured catalog in the
  canonical V5 geometry journal before opening any lane-derived Kura path, then
  binds the physical primary lane to its exact incarnation and storage paths.
  Replay restores the cursor before the first transition at genesis height, or
  to the snapshot's exact committed height, so multiple transitions at one
  height retain their durable sequence and cannot be skipped or replayed out of
  order. Checkpoints retain the last transition height and sequence; the first
  post-checkpoint transition uses checked height/sequence advancement. The
  published journal file is the commit point: only exact, regular, direct-child
  temporary files owned by an unfinished publication are consumed or removed.
  Configured-primary and journal-derived trees are traversed within fixed depth
  and entry bounds, every opened path is revalidated by filesystem identity,
  and paired geometry moves use atomic no-clobber renames. Before the first
  rename, the V2 lane-incarnation marker seals both intended target paths and a
  digest of the paired merge log. Mutable live pairs clear that move seal only
  after both halves arrive; retained archive pairs keep it, and GC validates
  the journal-owned logical target even after moving the archive into its
  quarantine name. Restart completes a block-before-merge partial move in
  either direction without overwriting an operator-created collision or
  adopting an unauthenticated path. Only a newly appended journal `Intent` may
  finish provisioning its journal-owned empty staging pair. Replay keeps
  `CatalogPublished` or `RolledBack` provenance until the inverse or forward
  move completes, and requires authenticated durable storage on every retry;
  losing that evidence can never be reinterpreted as authority to create an
  empty lane.
  Catalog-only routing without a live Nexus state view does not shard over
  elastic lanes; it keeps ordinary no-target traffic on the configured base
  default lane until live autoscale enablement and bounds are available.
  State-free router fast paths also defer unmatched no-target default traffic,
  including no-target IVM/proved-VM traffic, to live-state routing even when
  unrelated policy rules exist, so an unmatched rule cannot accidentally pin
  autoscaled default traffic to the base lane. State-free query routing,
  non-fallible state-free hints, and live non-fallible routing fallbacks reject
  autoscale-owned default or explicit-rule lanes instead of treating elastic
  lanes as operator-configured anchors.
  Fallible default-route resolution also rejects a corrupted policy whose
  configured default lane claims autoscale ownership, so elastic lanes cannot
  become the default anchor even when in-memory policy construction bypasses
  config validation.
  Canonical dataspace anchors used by dataspace-targeted writes, settlements,
  and permission-scope routes ignore every lane that claims autoscale ownership,
  even when the claim is malformed, so elastic capacity cannot become the
  canonical lane for a dataspace. A dataspace with only autoscale-owned lanes
  fails closed with `no_lane_for_dataspace`.
  Disabled Nexus, corrupted runtime autoscale bounds, or a default lane at or
  above `autoscale.min_lanes` disable elastic sharding for routing and keep
  no-target default traffic on the configured default lane.
  Scale-out also requires a free id in the configured
  `autoscale.min_lanes..autoscale.max_lanes` elastic range; hot windows fail
  closed without recording a transition once that range is exhausted, when the
  active elastic range is occupied by a manual or malformed managed lane, when
  the configured default route no longer resolves to the default dataspace, or
  when Kura cannot provide the complete historical sample window.
- When autoscale retires a managed elastic lane, the block-local lifecycle path
  prunes lane relay caches, lane-emergency-validator overrides, and
  verified relay contract-state records, merge-history checkpoints, DA
  commitment, confidential-compute, pin-intent, DA receipt cursor, and DA shard
  cursor indexes owned only by the retired lane in the same committed
  transition. Verified relay cleanup removes the canonical relay state key and
  its exact contract-map key for decoded records owned by the reset lane; if a
  canonical relay state row is undecodable, the lowercase exact key format is
  parsed and rows whose key lane is reset-owned are pruned as stale. Arbitrary
  prefixed siblings, uppercase digest variants, and opaque malformed
  contract-map rows remain inert state because they cannot be safely
  reverse-mapped to a lane. Verified-relay hydration likewise scans only
  lowercase exact canonical relay keys before decode, so arbitrary prefixed
  state cannot drive relay-cache admission attempts. Block-local resets also drop
  verified relay records staged earlier in the block for reset lanes before
  commit-time hydration, treating either the public relay reference lane or the
  embedded envelope lane as reset ownership. The same reset prunes AXT replay
  ledger entries keyed by a retired handle target lane while preserving
  cross-lane replay guards whose handles target surviving lanes. Public-lane
  stake-share rows and reward records keyed by or carrying the reset lane, plus
  reward-claim cursors keyed by the reset lane, are removed as live economic
  indices so a fresh incarnation can start reward epochs and claim accounting
  from its own state. Operator staking status snapshots for reset lanes are
  cleared at the same time, so status surfaces cannot continue reporting stale
  bonded or slash totals after a lane id is reused. It also
  refreshes the block-local AXT policy cache after the catalog change,
  retargeting Space Directory-derived entries when directory data is present and
  pruning explicit cache entries whose target lane no longer exists. Scale-in
  uses the same resolved
  default-route capacity and complete historical sample-window preconditions as
  scale-out, and only retires when router-owned capacity is strictly above the
  configured default-route lane, so stale routing state or unrelated manual
  lanes cannot retire an elastic lane. The `autoscale.min_lanes` and
  `autoscale.max_lanes` values bound the autoscaler-owned elastic id range;
  they do not count unrelated public-profile base lanes as default-route
  capacity. Despite their legacy names, these fields are not minimum and
  maximum active-lane counts: `min_lanes` is the inclusive lane-id lower bound
  and `max_lanes` is the exclusive upper bound. The range may contain vacant
  ids, and it may start at the next id above the initial catalog namespace;
  lifecycle creation expands that namespace deterministically. Live
  default-route capacity is the configured base anchor plus valid managed
  elastic entries that currently exist inside the half-open range.
- WSV snapshots persist a versioned `nexus_runtime` section containing the
  effective lane catalog and `autoscale.last_transition_height`. On restart,
  those stateful values remain authoritative while static Nexus policy is
  refreshed from node configuration; this prevents a tip snapshot from
  silently recreating a retired lane, dropping an elastic lane, or clearing the
  cooldown. Snapshot decoding rejects unknown layout versions, invalid or
  duplicate catalogs, missing lane 0, malformed autoscale ownership metadata,
  future lane-creation markers, and transition heights beyond the snapshot
  height. Kura then reconciles the retained geometry journal to the snapshot
  catalog and atomically rebinds its in-memory lane map without deleting inactive
  rollback storage. It never fabricates an empty segment for a missing dynamic
  lane: the exact lane/incarnation/activation marker and both block and merge
  paths must be recoverable, otherwise startup fails closed.
  Legacy snapshots without `nexus_runtime` retain the historical
  configuration-sourced startup behavior.
- Autoscale utilization samples count committed fragments, not just external
  transaction envelopes. Current-block decisions use the in-flight execution
  counter, and historical window samples read the persisted committed-fragment
  total from block results, with external transactions kept as a legacy floor.
  Block validation rejects non-zero committed-fragment totals that do not match
  re-execution, so peers cannot forge autoscale load history. Latency ratios
  and utilization are computed with widened deterministic integer intermediates
  and saturate only the final permille value, so extreme counters or timestamps
  cannot wrap or make an overloaded sample look cold. Utilization is divided by
  the same default-route capacity that the router can actually use:
  the configured default lane plus valid managed elastic lanes in the default
  dataspace and configured autoscale id range, not unrelated base/governance/zk,
  manually managed, malformed, or out-of-range lanes. The scale-out capacity
  bound uses the same default-route count, so unrelated manual catalog lanes and
  corrupted out-of-range managed lanes outside the autoscaler-managed candidate
  set do not inflate capacity or receive default traffic. Runtime autoscale
  prechecks also reject autoscale-owned out-of-range catalog corruption before
  plan construction, requiring an explicit repair retire instead of creating or
  retiring lanes around the corrupted entry. Missing historical Kura blocks,
  including gaps inside longer decision windows, and equal or backward block
  timestamps collapse the candidate window to no samples instead of
  extrapolating load or clamping bad timing evidence into synthetic hot/cold
  samples.
- Autoscale configuration fails closed before runtime: lane bounds, block
  targets, decision windows, cooldown, and per-lane TPS must be positive;
  `min_lanes < max_lanes` so the elastic id range is non-empty; and scale-in
  thresholds must stay below scale-out thresholds so hysteresis cannot collapse
  into repeated lane churn. The block
  transition path repeats the lane-bound safety-cap and ratio sanity checks
  against the effective runtime state, so programmatic config swaps or
  corrupted actual state cannot raise `max_lanes` above the compiled cap or
  reinterpret non-finite, zero, sub-permille, or collapsed thresholds as
  permissive scale-out/scale-in triggers. Raw scale-in ratios must remain
  strictly below scale-out ratios after conversion to the permille thresholds
  used by block application. A future `last_transition_height` is treated as an
  active cooldown and suppresses create/retire transitions without overwriting
  the marker. If a hot/cold decision reaches the internal lifecycle helper but
  that helper rejects the add/retire plan, the catalog remains unchanged and
  `last_transition_height` is not advanced, so corrupted runtime state cannot
  pin cooldown after a failed scaling attempt. When configured windows
  conflict, a hot longer scale-out window takes precedence over a cold shorter
  scale-in window so capacity is added rather than retired in the same block.
  Scale-out treats either sustained p95 latency above the scale-out latency
  threshold or sustained p95 utilization above the scale-out utilization
  threshold as enough pressure to add managed capacity. Scale-in stays
  conjunctive and retires capacity only when both p95 latency and p95
  utilization are at or below their scale-in thresholds for the full cold
  window.
- Native AMX participant votes are prefiltered before they enter proposer
  session caches. The received vote message variant must match the signed
  attestation phase, remote sender `PeerId` must match the vote signer, the
  signer must use a BLS-normal consensus identity, and the individual signature
  must verify before the state-dependent live-PoP check and exact-body cache
  insertion run. QC assembly repeats the BLS-normal and individual-signature
  checks before aggregating, so polluted vote inputs fail closed even outside
  the normal ingress path. Vote caches are bounded both by session count and by
  exact attestation-body buckets inside each session, using deterministic FIFO
  eviction so one source/plan cannot retain unbounded retried or adversarial
  bodies. Operators tune these guards with
  `sumeragi.advanced.native_amx.session_cache_max` and
  `sumeragi.advanced.native_amx.session_body_bucket_max` (defaults: `1024` and
  `256` respectively).
- Any Torii ingress node may accept transactions and route them using the
  active routing policy, even if the target dataspace is not validated locally
  by that ingress node.
- SDKs surface lane selectors and map user-friendly aliases to `LaneId` using the lane catalog.
- Routing rules operate on the validated catalog and may pick both lane and dataspace. `LaneConfig` provides telemetry-friendly aliases for dashboards and logs.
- Enabled Nexus config swaps and lane lifecycle plans are validated before
  mutation: the configured default route and explicit rule targets must resolve
  against the candidate lane/dataspace catalogs. A rule that omits `dataspace`
  is validated against `nexus.routing_policy.default_dataspace`, and explicit
  rules cannot target autoscale-owned lanes. The router's fallible `try_route*`
  APIs enforce the same explicit-rule ownership boundary even if a corrupted or
  manually constructed policy bypasses config validation. Plans that would
  retire a policy lane, remove the default dataspace, rebind a lane without a
  matching routing-policy update, repeat an addition id or alias, or list the
  same retired lane more than once fail atomically.
- External Nexus config swaps and manual lane lifecycle plans must not claim
  autoscale-managed lanes or retire valid autoscale-managed lanes. The reserved
  `autoscale.managed` metadata key is written only by the consensus autoscaler,
  and valid managed elastic lanes are destroyed only through the internal
  autoscale lifecycle path. Invalid autoscale-owned lanes may be removed by an
  explicit lifecycle retire so operators can repair corrupted ownership state.
  While
  `autoscale.enabled = true`, manual lanes also cannot be added inside the
  configured `autoscale.min_lanes..autoscale.max_lanes` elastic id range; base,
  governance, zk, or other operator-managed lanes must sit outside that range
  so they cannot consume deterministic scale-out capacity. The routing
  `default_lane` must also remain below `autoscale.min_lanes`, giving the
  default route a base anchor below the autoscale-owned elastic range. Config
  swaps may preserve an active autoscale-managed lane unchanged, but swaps that
  add, mutate, omit, or replace one fail atomically instead of silently converting
  it into a manual lane change. Preserved autoscale-managed lanes must stay inside
  the configured `autoscale.min_lanes..autoscale.max_lanes` id range and remain
  bound to `nexus.routing_policy.default_dataspace` so ownership cannot be
  stranded outside the autoscaler's create/retire range or default dataspace.
  Config swaps also cannot disable `autoscale.enabled` while owned elastic lanes
  exist; valid owned lanes must remain under the autoscaler, and invalid owned
  lanes must be explicitly retired before the owner is disabled. Static TOML
  parsing rejects `nexus.autoscale.enabled = true` unless
  `nexus.enabled = true`; it also rejects both the reserved
  `autoscale.managed` lane metadata key and manual lanes in the enabled
  elastic id range before runtime for the same ownership boundary. The internal
  autoscale lifecycle path must create
  deterministic public elastic lanes in the configured default dataspace
  (`autoscale.managed = true`, `autoscale.created_height` equal to the current
  transition block height, and `elastic-lane-{id}` alias) and cannot add or
  retire unmanaged/manual lanes, malformed autoscale-owned lanes, or managed
  lanes outside the configured elastic id range or default dataspace. Internal
  transition metadata must also match the plan shape exactly: scale-out stages
  one addition for the logged lane, while scale-in stages one retirement for the
  logged lane. Its `active_lanes` and `autoscale_capacity_lanes` fields must
  also match the current default-route autoscale capacity before the plan is
  staged, and the same pending transition metadata is revalidated at block
  commit before storage geometry is published. Runtime
  lifecycle validation also checks every lane in the resulting catalog, so
  unrelated plans cannot preserve a pre-existing manual lane inside the active
  elastic range or an autoscale-owned lane with malformed metadata, a disabled
  autoscale owner, an out-of-range id, or a non-default dataspace binding.
  Operators may still repair manual elastic-range corruption by explicitly
  retiring the manual lane. Invalid autoscale-owned lanes can likewise be
  repaired by an explicit lifecycle retire, while valid autoscale-owned lanes
  remain protected from manual retirement and corrupted owned lanes cannot be
  hidden behind an unrelated lifecycle plan.
  Runtime
  `State::set_nexus` also rejects disabled Nexus configs that carry lane,
  dataspace, or routing overrides, enable autoscale, enable lane-relay
  emergency overrides, or enable the relay worker,
  matching the user-config parser's single-lane disabled profile. Relay worker
  configs must also use lane-relay-burn fee settlement at the state boundary.
  Authority-paid Nexus fees are rejected in this mode until an authenticated
  authority spend-lease protocol exists; sponsor receipts require a verified
  source allocation for the exact program revision and asset. Emergency relay
  multisig thresholds cannot exceed member count. Per-dataspace defaults name
  one exact `fee_sponsor_program_id` and require enabled Nexus plus dataspace
  keys present in the active catalog; there is no sponsorship toggle or account
  fallback. Runtime config swaps also enforce the parser's fee-shape contract:
  the fee asset selector must be the canonical XOR asset definition id or
  `xor#universal`/`xor#universal.universal` after genesis binds the alias to a
  canonical Base58 asset definition, and is trimmed to the parser-normalized
  selector, while the fee sink literal must be non-empty. Operation allow/deny
  selectors and asset budgets live on immutable on-chain sponsor-program
  revisions rather than in runtime configuration.
- Unresolved routing is deterministic: if a rule resolves to an unknown lane,
  unknown dataspace, or lane/dataspace mismatch, admission is rejected with an
  unresolved-route error (no fallback-to-default rewrite for ambiguous inputs).
- State-aware Torii, gossip, admission, consensus requeue, and block-requeue
  routing first synchronize the queue router, routing policy, and cached
  catalogs from current Nexus state, then validate computed plans against those
  current lane/dataspace catalogs. Caller-provided or cached routing plans are
  accepted only when every coordinator and participant leg resolves against
  current catalogs and the full plan exactly matches a freshly recomputed full
  plan for the same transaction, so stale plans cannot survive a policy change
  merely because the old lane still exists. Fresh lanes are usable before an
  external cache refresh, and stale queue-local policies or lanes cannot remain
  authoritative after retirement or policy changes.
  Queue plan-journal replay applies the same committed-state synchronization and
  full-plan comparison, tombstoning stale Native AMX participant legs even when
  the old participant lane still resolves against the active catalog.
  Queue reconfiguration after committed Nexus changes also refreshes cached
  full Native AMX routing plans for pending transactions through both state-
  and view-backed reconfiguration entry points, so participant legs cannot
  remain stale behind an unchanged coordinator route.
  Block requeue discards a stale process-global routing-ledger plan after a
  failed ledger-sourced reinsertion, so the next recovery pass recomputes
  Native AMX participant legs from current committed state instead of replaying
  the same stale hint. Commit event production consumes the full routing plan
  before any legacy coordinator-only hint, so partial ledger cleanup or a stale
  single-route shadow cannot override the digest-checked plan metadata.
  Queue-side expiry and unresolved-route rejection events apply the same
  full-plan-first cleanup before terminal events are emitted, so stale
  coordinator-only shadows cannot mislabel removed transactions. Shared
  routing-ledger plan discard also clears same-hash legacy shadows once the
  expected full plan is removed.
  Lane TEU deferral also returns the full routing plan for consensus requeue, so
  deferred Native AMX transactions keep participant legs instead of requeueing
  as coordinator-only work.
  Transaction gossip route hints also resolve against the active dataspace
  catalog before broadcast or reinsertion, so dangling lane bindings left after
  dataspace removal are rejected alongside missing lanes and lane/dataspace
  mismatches. Gossip batch partitioning carries the full routing plan alongside
  the coordinator route and uses actual Norito length as a fallback for
  variable-size plans, so Native AMX participant legs are not requeued
  indefinitely or reduced to coordinator-only metadata. Outgoing gossip batch
  assembly also refreshes cached full routing plans from current Nexus state
  before emitting route hints, so Native AMX participant drift is corrected
  before serialization.
  Torii submit-transaction proxy receivers apply the same full-plan comparison
  to ingress hints, so Native AMX participant drift is rejected even when the
  coordinator route is unchanged. The receiver also validates canonical
  coordinator/participant roles and checks that a Native AMX hint's advertised
  `plan_digest` matches the digest recomputed from its route legs before queue
  admission, preventing forged proxy hints from being silently normalized.
- Proposal assembly refreshes stale routing vectors with the same live Nexus
  snapshot, including the active autoscale elastic range. Proposal sidecars and
  execution-context routes therefore cannot collapse autoscaled default-route
  assignments back to the base lane while the queue and committed state still
  route them to an elastic lane. The refresh compares the full routing plan, so
  Native AMX proposal vectors also replace stale participant legs even when the
  coordinator route is unchanged. Proposal size-cap trimming preserves full
  routing plans for removed transactions too, so overflow requeue keeps Native
  AMX participant metadata. Pending queue reconfiguration uses the same
  height-aware autoscale range, so queued default-route transactions and local
  routing-ledger hints stay on the active default route until an elastic lane's
  creation height is committed. Proposal lookahead is enabled only when the
  committed Nexus snapshot has more than one policy-reachable active lane at the
  candidate block height. Valid default-route autoscale candidates, canonical
  dataspace routes, and valid explicit rule lanes count; autoscale-owned default
  anchors, off-default autoscale-owned rule targets, unrouted same-dataspace
  sidecar lanes, and future-created autoscale lanes cannot trigger scan-budget
  overfetch before routing policy can select them.
- Block validation and block execution use that same live Nexus autoscale range
  when recomputing execution-context routing and per-lane transaction
  summaries. Validators therefore accept matching elastic execution contexts
  and reject stale base-lane contexts for transactions that the committed Nexus
  state routes to an elastic default lane. If Nexus is disabled, or if the
  active elastic range contains a manual, malformed managed, or off-default
  managed lane, validators likewise reject stale elastic execution contexts
  because live routing falls back to the base default lane. Native AMX execution
  contexts also compare every committed coordinator and participant leg with the
  recomputed full plan, so a stale participant route cannot survive merely
  because the
  coordinator and plan digest still look current. Per-lane committed TEU
  telemetry is attributed from the same validated block routing vector, not from
  the process-global routing hint ledger, so stale cached hints cannot move slot
  load metrics onto the wrong lane.
- Global Torii pipeline-status reads treat routing-plan hints as probes only.
  A hinted route may short-circuit the fanout path only after returning a
  terminal status (`Applied`, `Rejected`, or `Expired`); non-terminal hinted
  successes (`Queued`, `Approved`, or `Committed`) and malformed hinted success
  bodies fall through to full fanout, so stale retired-lane caches cannot hide a
  newer terminal status on the active lane.
- Incoming Torii read and verified-query proxy requests still execute on the
  ingress-selected receiver when the route is active, avoiding multi-hop
  cascades during transient authority-view skew, but receivers first validate
  the hinted lane/dataspace against their current Nexus catalogs. Missing
  retired lanes and lane/dataspace mismatches return `route_unavailable` with
  `stale_route` diagnostics before any local read handler executes.
- Stateful transaction validation without a caller-supplied routing context also
  resolves the active Nexus full plan before taking the coordinator lane. Direct
  validation entrypoints therefore cannot fall back to catalog-only base-lane
  routing while autoscale is distributing default-route traffic across elastic
  lanes.
- Startup replay of the pending queue-plan journal uses the same current-state
  synchronization before comparing persisted plans. Journal records whose
  routed lane/dataspace no longer matches committed Nexus policy are tombstoned
  instead of being replayed under stale queue-local routing. A stale elastic
  default-route plan is tombstoned the same way if active elastic-range
  corruption makes live routing fall back to the base lane after restart.
- Runtime catalog updates apply the same fail-closed rule to already queued
  transactions: autoscale scale-in re-shards pending default-route traffic onto
  the surviving elastic/default candidates, and if any lifecycle change makes a
  pending transaction unrouteable, the queue rejects it and clears stale routing
  caches plus TEU backlog accounting instead of proposing it with retired-lane
  metadata.
- Merge-candidate synthesis requires the exact cached envelope to have a
  committed, revalidated FastPQ effect record and also rechecks it against the
  active Nexus lane catalog. Structural gossip metadata alone is progress data,
  never merge authority. Relays for lanes that no longer exist, or for
  lane/dataspace bindings that no longer match or no longer have a dataspace
  catalog entry, are ignored even if stale cache state survived outside the
  normal lifecycle pruning path.
  `State::commit_merge_entry` applies the same active-catalog check before it
  trusts a QC-backed merge snapshot, updates merge metadata, or settles Nexus
  fee receipts.
- Merge commits also require every lane snapshot to advance beyond the latest
  committed height remembered for the same `(lane_id, dataspace_id)`, even when
  that lane was omitted from the newest active-only merge entry because it was
  unchanged. A higher-epoch entry cannot replay the same lane height or regress
  to an older cached relay, even when the relay payload and merge QC are
  otherwise valid. Lane retirement, lane/dataspace rebinding (including
  rebinds that keep the same runtime DA shard id), and fresh lane-id additions
  reset the remembered merge height, verified relay contract-state records, and
  lane-scoped DA receipt cursors plus any DA shard cursor and public-lane
  economic index or operator staking status owned only by that lane. Verified
  relay contract-state pruning is exact-keyed to the canonical
  relay key and matching contract-map key, so spoofed prefixed siblings cannot
  expand reset scope or rehydrate relay evidence. A deliberately destroyed and
  recreated lane is not blocked by the previous
  incarnation's merge height or DA sequence cursors, including after startup
  rehydrates historical merge entries from Kura or persisted DA shard cursor
  journals. A lifecycle plan that retires and adds the same lane id in one
  transaction is treated as the same kind of fresh incarnation, even when the
  replacement keeps the same dataspace and runtime geometry.

## Settlement & Fees

- Every lane pays XOR fees to the global validator set. Lanes may collect native gas tokens but must escrow XOR equivalents alongside commitments.
- Settlement proofs include amount, conversion metadata, and proof of escrow (e.g., transfer to global fee vault).
- The unified settlement router (NX-3) debits buffers using the same lane prefixes, so settlement telemetry lines up with storage geometry.

## Governance

- Lanes declare their governance module via the catalog. `LaneConfigEntry` carries the original alias and slug to keep telemetry and audit trails readable.
- The Nexus registry distributes signed lane manifests that include the `LaneId`, dataspace binding, governance handle, settlement handle, and metadata.
- Runtime-upgrade hooks continue to enforce governance policies (`gov_upgrade_id` by default) and log diffs via the telemetry bridge (`nexus.config.diff` events).
- Lane manifests define the dataspace validator pool for admin-managed lanes
  using explicit `{ validator, peer_id }` bindings; stake-elected lanes derive
  their validator pool from public-lane staking records. In both modes,
  authoritative routing and roster selection use the stored `peer_id` rather
  than deriving peers from validator account signatories. Explicit manifest
  validators and bindings are exposed only when the declared validator list is
  duplicate-free, each binding names a declared validator, and the binding list
  has no duplicate validators or peers before public or routing authority reads
  consume them, so undeclared or duplicate manifest rows cannot multiply
  validator or peer weight. Protected governance admission and transaction
  state validation canonicalize the same duplicate-free validator set before
  authority or quorum checks, so duplicate validator rows fail closed instead
  of being silently collapsed at one boundary. Governance quorum metadata
  (`gov_manifest_approvers`) is duplicate-free as well; duplicate approver
  claims reject the transaction instead of being collapsed into one approval.
  Manifest loading likewise rejects duplicate protected namespaces and duplicate
  runtime-upgrade allowlist ids after trimming. Duplicate manifest filenames
  that resolve to the same lane alias in one source directory invalidate that
  alias so the governed lane remains locked until the duplicate source is
  removed.
- Public-lane staking mutations bind the transaction authority to the economic
  owner they mutate. Validator registration and exit must be submitted by the
  validator account itself, initial registration stake must come from that same
  validator account, and bond/schedule-unbond/finalize-unbond operations must
  be submitted by the staker account whose balance or pending withdrawal is
  being changed.
- Live NPoS lane-scope inference uses only `Active` public-lane validator
  records. Jailed, exiting, exited, pending, or slashed historical records stay
  available for audit and staking lifecycle queries but cannot pin recovery
  elections or active topology selection to a stale lane after retirement,
  rebinding, or autoscale scale-in. Live topology, stake-snapshot, election,
  due-activation, released-exit sweeping, penalty-locator,
  staking-admission, direct staking mutations, reward bookkeeping, slash
  handling, peer/account cleanup guards, multisig account-rekey rewrites,
  Soracloud runtime-authority, and host-finance stake-accounting derivations
  also ignore or reject public-lane validator rows unless the
  storage key `(lane_id, validator)` exactly matches the embedded
  `PublicLaneValidatorRecord`, so malformed or stale rows cannot auto-promote,
  exit-finalize, mutate bonded stake, receive reward epochs, enter a roster,
  reserve capacity or peer bindings, block account cleanup, get force-exited by
  peer cleanup, get repaired into a live row during account rekey, grant
  runtime authority, or redirect penalties through a mismatched peer binding.
  Torii's public-lane validator app API applies the same exact-key filter before
  serializing staking rows, falling back to manifest validator bindings only
  when no valid staking rows exist for the requested lane. Public-lane
  reward-claim, commit election profile, multisig account-rekey, and Torii
  stake-share/reward app API paths likewise consume or serialize only rows whose
  storage keys exactly match the embedded `(lane_id, validator, staker)` or
  `(lane_id, epoch)` economic record fields.
  When a lane reset retires or rebinds a lane, `set_nexus`, lifecycle plans,
  and autoscale scale-in terminalize
  revivable public-validator records (`PendingActivation`, `Active`, or
  `Jailed`) for that lane as `Exited`, treating either the storage key lane or
  the embedded record lane as reset ownership before later epoch promotion or
  roster derivation can reuse them. World-backed NPoS quorum, coverage, and
  commit-root stake selection paths use the same active-lane filter whenever
  Nexus is enabled. Public-lane stake-share rows, reward records, and
  reward-claim cursors are not audit records; lane reset paths delete
  reset-owned rows (by storage key or embedded lane where present) and clear
  operator staking status for reset lanes while preserving unchanged-lane
  economic state and status. Lane-relay emergency overrides are pruned for
  reset lanes and for lanes absent from the updated catalog; an override
  pre-staged for a newly created lane id is treated as stale prior-incarnation
  state and must be reinstalled after the lifecycle update. Failed
  lane-retirement preflight
  keeps those live rows, emergency overrides, AXT replay entries, verified
  relay state, and validator activity in the committed WSV until the storage
  transition can complete.

## Telemetry & Status

- `/v1/nexus/lifecycle` exposes the lifecycle/catalog snapshot used to verify
  lane aliases, dataspace bindings, incarnations, activation/close heights, and
  autoscale transitions.
- `/v1/sumeragi/status` exposes only authoritative `SumeragiV2Status` reducer
  state; it does not carry lane evidence.
- `/v1/sumeragi/diagnostics` exposes non-authoritative operational evidence
  including lane commitments/sessions. Its bounded Native
  participant-application records and bounded restart-stable autonomous
  execution stages are derived from durable State/Kura evidence (plus durable
  queue ownership for finalization). A Native same-height identity disagreement
  is `conflict`, never a silently selected winner.
- Scheduler metrics (`nexus_scheduler_lane_teu_*`) render lane aliases/slugs so operators can map backlog and TEU pressure quickly.
- `nexus_lane_configured_total` counts the number of derived lane entries and is recomputed when configuration changes. Telemetry emits signed diffs whenever lane geometry changes.
- Dataspace backlog gauges include the alias/description metadata to help operators associate queue pressure with business domains.

## Configuration & Norito Types

- `LaneCatalog`, `LaneConfig`, and `DataSpaceCatalog` live in `iroha_data_model::nexus` and provide Norito-format structures for manifests and SDKs.
- `LaneConfig` lives in `iroha_config::parameters::actual::Nexus` and is derived automatically from the catalog; it does not require Norito encoding because it is an internal runtime helper.
- The user-facing configuration (`iroha_config::parameters::user::Nexus`) continues to accept declarative lane and dataspace descriptors; parsing now derives the geometry and rejects invalid aliases or duplicate lane ids.
- `DataSpaceMetadata.fault_tolerance` controls lane-relay committee sizing; committee membership is sampled deterministically per epoch from the dataspace validator pool using the VRF epoch seed bound with `(dataspace_id, lane_id)`.

## Outstanding Work

- Integrate settlement router updates (NX-3) with the new geometry so XOR buffer debits and receipts are tagged by lane slug.
- Close the still-open multilane release-evidence gates: fresh unskipped
  four-peer suites; 10/10 fresh twelve-peer corridor seeds; the two-hour fault
  soak; five pinned-hardware one-versus-four-lane pairs meeting the throughput,
  latency, and resource bounds; SDK/formal evidence; and the prescribed
  locked/offline full-workspace build, test, strict Clippy, formatting, and
  legacy-codec checks. This document does not claim those runs have passed.
- Add compliance hooks for whitelists/blacklists and programmable-money policies (tracked under NX-12).

---

*This document will evolve as NX-2 through NX-18 tasks progress. Please capture open questions in the roadmap or governance tracker.*
