---
title: Nexus Lane Model
sidebar_label: Lane Model
description: Logical lane taxonomy, lane configuration geometry, and world-state merge rules for SORA Nexus.
---

# Nexus Lane Model & WSV Partitioning

> **Status:** NX-1 deliverable — lane taxonomy, configuration geometry, and storage layout are ready for implementation.  
> **Owners:** Nexus Core WG, Governance WG  
> **Related roadmap item:** NX-1

This document captures the target architecture for Nexus’ multilane consensus layer. The goal is to produce a single deterministic world state while allowing individual data spaces (lanes) to run public or private validator sets with isolated workloads.

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
- Optional telemetry metadata (description, contact, business domain) surfaced through `/status` and dashboards.

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
- When governance renames a lane alias, nodes automatically relabel the corresponding `blocks/lane_{id:03}_{slug}` directories (and tiered snapshots) so auditors always see the canonical slug without manual cleanup.

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
- **Retention policy** — public lanes retain full block bodies; commitment-only lanes may compact older bodies after checkpoints because commitments are authoritative. Confidential lanes keep ciphertext journals in dedicated segments to avoid blocking other workloads.
- **Tooling** — `cargo xtask nexus-lane-maintenance --config <path> [--compact-retired]` inspects `<store>/blocks` and `<store>/merge_ledger` using the derived `LaneConfig`, reports active vs retired segments, and archives retired directories/logs under `<store>/retired/...` to keep evidence deterministic. Maintenance utilities (`kagami`, CLI admin commands) should reuse the slugged namespace when exposing metrics, Prometheus labels, or archiving Kura segments.

## Storage Budgets

- `nexus.storage.local_budget_bytes` defines the total node-local on-disk budget that Nexus nodes should consume across Kura, cold WSV snapshots, SoraFS storage, and streaming spools (SoraNet/SoraVPN). The legacy `nexus.storage.max_disk_usage_bytes` alias remains accepted for compatibility.
- `nexus.storage.budget_enforce_interval_blocks` sets how often (in committed blocks) the storage budget scan runs; set to 0 to enforce every block.
- When the global disk budget is exceeded, eviction is deterministic: prune SoraNet provision spools in lexicographic path order, then SoraVPN spools, then tiered-state cold snapshots oldest-first (offloading to `da_store_root` when configured), then Kura retired segments, and finally evict active Kura block bodies into `da_blocks/` for DA-backed rehydration on read. Blocks that exceed the Kura budget on their own are persisted directly into `da_blocks/` and indexed as evicted.
- `nexus.storage.max_wsv_memory_bytes` caps the hot WSV tier by propagating deterministic in-memory WSV sizing into `tiered_state.hot_retained_bytes`; grace retention may temporarily exceed the budget, but the overflow is observable via telemetry (`state_tiered_hot_bytes`, `state_tiered_hot_grace_overflow_bytes`).
- `nexus.storage.disk_budget_weights` splits the disk budget across components using basis points (must sum to 10,000). The derived caps are applied to `kura.max_disk_usage_bytes`, `tiered_state.max_cold_bytes`, `sorafs.storage.max_capacity_bytes`, `streaming.soranet.provision_spool_max_bytes`, and `streaming.soravpn.provision_spool_max_bytes`.
- Kura's storage budget enforcement sums block-store bytes across active + retired lane segments and includes queued blocks not yet persisted to avoid overshoot during write lag.
- SoraVPN provisioning spools use `streaming.soravpn` settings and are capped independently from the SoraNet provision spool.
- Per-component limits still apply: when a component has an explicit non-zero cap, the smaller of the explicit cap and the derived Nexus budget is enforced.
- Budget telemetry uses `storage_budget_bytes_used{component=...}` and `storage_budget_bytes_limit{component=...}` to report usage/caps for `kura`, `wsv_hot`, `wsv_cold`, `soranet_spool`, and `soravpn_spool`; `storage_budget_exceeded_total{component=...}` increments when enforcement rejects new data and logs emit a warning for the operator.
- DA eviction telemetry adds `storage_da_cache_total{component=...,result=hit|miss}` and `storage_da_churn_bytes_total{component=...,direction=evicted|rehydrated}` to track cache activity and bytes moved for `kura` and `wsv_cold`.
- Kura reports the same accounting used during admission (on-disk bytes plus queued blocks, including merge-ledger entry payloads when present), so the budget gauges reflect effective pressure rather than just persisted bytes.

## Routing & APIs

- Torii REST/gRPC endpoints accept an optional `lane_id`; absence resolves via
  `nexus.routing_policy.default_lane` / `default_dataspace`.
- Any Torii ingress node may accept transactions and route them using the
  active routing policy, even if the target dataspace is not validated locally
  by that ingress node.
- SDKs surface lane selectors and map user-friendly aliases to `LaneId` using the lane catalog.
- Routing rules operate on the validated catalog and may pick both lane and dataspace. `LaneConfig` provides telemetry-friendly aliases for dashboards and logs.
- Unresolved routing is deterministic: if a rule resolves to an unknown lane,
  unknown dataspace, or lane/dataspace mismatch, admission is rejected with an
  unresolved-route error (no fallback-to-default rewrite for ambiguous inputs).

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
  than deriving peers from validator account signatories.

## Telemetry & Status

- `/status` exposes lane aliases, dataspace bindings, governance handles, and settlement profiles, derived from the catalog and `LaneConfig`.
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
- Finalise the merge algorithm (ordering, pruning, conflict detection) and attach regression fixtures for cross-lane replay.
- Add compliance hooks for whitelists/blacklists and programmable-money policies (tracked under NX-12).

---

*This document will evolve as NX-2 through NX-18 tasks progress. Please capture open questions in the roadmap or governance tracker.*
