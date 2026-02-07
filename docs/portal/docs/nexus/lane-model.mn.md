---
lang: mn
direction: ltr
source: docs/portal/docs/nexus/lane-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 963c22085107a828fb801f1cdbcef3745e975431f7168531688f4c8d487cf2b3
source_last_modified: "2026-01-05T09:28:11.843792+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-lane-model
title: Nexus lane model
description: Logical lane taxonomy, configuration geometry, and world-state merge rules for Sora Nexus.
---

# Nexus Lane Model & WSV Partitioning

> **Status:** NX-1 deliverable — lane taxonomy, configuration geometry, and storage layout are ready for implementation.  
> **Owners:** Nexus Core WG, Governance WG  
> **Roadmap reference:** NX-1 in `roadmap.md`

This portal page mirrors the canonical `docs/source/nexus_lanes.md` brief so Sora
Nexus operators, SDK owners, and reviewers can read the lane guidance without
diving into the mono-repo tree. The target architecture keeps the world state
deterministic while allowing individual data spaces (lanes) to run public or
private validator sets with isolated workloads.

## Concepts

- **Lane:** Logical shard of the Nexus ledger with its own validator set and
  execution backlog. Identified by a stable `LaneId`.
- **Data Space:** Governance bucket grouping one or more lanes that share
  compliance, routing, and settlement policies.
- **Lane Manifest:** Governance-controlled metadata describing validators, DA
  policy, gas token, settlement rules, and routing permissions.
- **Global Commitment:** Proof emitted by a lane summarising new state roots,
  settlement data, and optional cross-lane transfers. The global NPoS ring
  orders commitments.

## Lane taxonomy

Lane types canonically describe their visibility, governance surface, and
settlement hooks. The configuration geometry (`LaneConfig`) captures these
attributes so nodes, SDKs, and tooling can reason about the layout without
bespoke logic.

| Lane type | Visibility | Validator membership | WSV exposure | Default governance | Settlement policy | Typical use |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | public | Permissionless (global stake) | Full state replica | SORA Parliament | `xor_global` | Baseline public ledger |
| `public_custom` | public | Permissionless or stake-gated | Full state replica | Stake weighted module | `xor_lane_weighted` | High-throughput public applications |
| `private_permissioned` | restricted | Fixed validator set (governance approved) | Commitments & proofs | Federated council | `xor_hosted_custody` | CBDC, consortium workloads |
| `hybrid_confidential` | restricted | Mixed membership; wraps ZK proofs | Commitments + selective disclosure | Programmable money module | `xor_dual_fund` | Privacy-preserving programmable money |

All lane types must declare:

- Dataspace alias — human-readable grouping that binds compliance policies.
- Governance handle — identifier resolved through `Nexus.governance.modules`.
- Settlement handle — identifier consumed by the settlement router to debit XOR
  buffers.
- Optional telemetry metadata (description, contact, business domain) surfaced
  through `/status` and dashboards.

## Lane configuration geometry (`LaneConfig`)

`LaneConfig` is the runtime geometry derived from the validated lane catalog. It
does **not** replace governance manifests; instead it provides deterministic
storage identifiers and telemetry hints for every configured lane.

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

- `LaneConfig::from_catalog` recomputes the geometry whenever configuration is
  loaded (`State::set_nexus`).
- Aliases are sanitised into lowercase slugs; consecutive non-alphanumeric
  characters collapse into `_`. If the alias yields an empty slug we fall back
  to `lane{id}`.
- `shard_id` is derived from the catalog metadata key `da_shard_id` (defaulting
  to `lane_id`) and drives the persisted shard cursor journal to keep DA replay
  deterministic across restarts/resharding.
- Key prefixes ensure the WSV keeps per-lane key ranges disjoint even when the
  same backend is shared.
- Kura segment names are deterministic across hosts; auditors can cross-check
  segment directories and manifests without bespoke tooling.
- Merge segments (`lane_{id:03}_merge`) hold the latest merge-hint roots and
  global state commitments for that lane.

## World-state partitioning

- The logical Nexus world state is the union of per-lane state spaces. Public
  lanes persist full state; private/confidential lanes export Merkle/commitment
  roots to the merge ledger.
- MV storage prefixes every key with the 4-byte lane prefix from
  `LaneConfigEntry::key_prefix`, yielding keys such as `[00 00 00 01] ++
  PackedKey`.
- Shared tables (accounts, assets, triggers, governance records) therefore store
  entries grouped by lane prefix, keeping range scans deterministic.
- Merge-ledger metadata mirrors the same layout: each lane writes merge-hint
  roots and reduced global state roots to `lane_{id:03}_merge`, allowing
  targeted retention or eviction when a lane retires.
- Cross-lane indexes (account aliases, asset registries, governance manifests)
  store explicit lane prefixes so operators can reconcile entries quickly.
- **Retention policy** — public lanes retain full block bodies; commitment-only
  lanes may compact older bodies after checkpoints because commitments are
  authoritative. Confidential lanes keep ciphertext journals in dedicated
  segments to avoid blocking other workloads.
- **Tooling** — maintenance utilities (`kagami`, CLI admin commands) should
  reference the slugged namespace when exposing metrics, Prometheus labels, or
  archiving Kura segments.

## Routing & APIs

- Torii REST/gRPC endpoints accept an optional `lane_id`; absence implies
  `lane_default`.
- SDKs surface lane selectors and map user-friendly aliases to `LaneId` using
  the lane catalog.
- Routing rules operate on the validated catalog and may pick both lane and
  dataspace. `LaneConfig` provides telemetry-friendly aliases for dashboards and
  logs.

## Settlement & fees

- Every lane pays XOR fees to the global validator set. Lanes may collect native
  gas tokens but must escrow XOR equivalents alongside commitments.
- Settlement proofs include amount, conversion metadata, and proof of escrow
  (for example, transfer to the global fee vault).
- The unified settlement router (NX-3) debits buffers using the same lane
  prefixes, so settlement telemetry lines up with storage geometry.

## Governance

- Lanes declare their governance module via the catalog. `LaneConfigEntry`
  carries the original alias and slug to keep telemetry and audit trails
  readable.
- The Nexus registry distributes signed lane manifests that include the
  `LaneId`, dataspace binding, governance handle, settlement handle, and
  metadata.
- Runtime-upgrade hooks continue to enforce governance policies
  (`gov_upgrade_id` by default) and log diffs via the telemetry bridge
  (`nexus.config.diff` events).

## Telemetry & status

- `/status` exposes lane aliases, dataspace bindings, governance handles, and
  settlement profiles, derived from the catalog and `LaneConfig`.
- Scheduler metrics (`nexus_scheduler_lane_teu_*`) render lane aliases/slugs so
  operators can map backlog and TEU pressure quickly.
- `nexus_lane_configured_total` counts the number of derived lane entries and is
  recomputed when configuration changes. Telemetry emits signed diffs whenever
  lane geometry changes.
- Dataspace backlog gauges include the alias/description metadata to help
  operators associate queue pressure with business domains.

## Configuration & Norito types

- `LaneCatalog`, `LaneConfig`, and `DataSpaceCatalog` live in
  `iroha_data_model::nexus` and provide Norito-format structures for
  manifests and SDKs.
- `LaneConfig` lives in `iroha_config::parameters::actual::Nexus` and is derived
  automatically from the catalog; it does not require Norito encoding because it
  is an internal runtime helper.
- The user-facing configuration (`iroha_config::parameters::user::Nexus`)
  continues to accept declarative lane and dataspace descriptors; parsing now
  derives the geometry and rejects invalid aliases or duplicate lane IDs.

## Outstanding work

- Integrate settlement router updates (NX-3) with the new geometry so XOR buffer
  debits and receipts are tagged by lane slug.
- Extend admin tooling to list column families, compact retired lanes, and
  inspect per-lane block logs using the slugged namespace.
- Finalise the merge algorithm (ordering, pruning, conflict detection) and
  attach regression fixtures for cross-lane replay.
- Add compliance hooks for whitelists/blacklists and programmable-money
  policies (tracked under NX-12).

---

*This page will continue to track NX-1 follow-ups as NX-2 through NX-18 land.
Please surface open questions in `roadmap.md` or the governance tracker so the
portal stays aligned with the canonical docs.*
