---
lang: uz
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6120b882618e0f9b6113948d3b12d97e0152a5fc5d4350681ba30aaf114e99d3
source_last_modified: "2026-01-22T14:45:01.354580+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-default-lane-quickstart
title: Default lane quickstart (NX-5)
sidebar_label: Default Lane Quickstart
description: Configure and verify the Nexus default lane fallback so Torii and SDKs can omit lane_id in public lanes.
---

:::note Canonical Source
This page mirrors `docs/source/quickstart/default_lane.md`. Keep both copies
aligned until the localization sweep lands in the portal.
:::

# Default Lane Quickstart (NX-5)

> **Roadmap context:** NX-5 — default public lane integration. The runtime now
> exposes a `nexus.routing_policy.default_lane` fallback so Torii REST/gRPC
> endpoints and every SDK can safely omit a `lane_id` when the traffic belongs
> on the canonical public lane. This guide walks operators through configuring
> the catalog, verifying the fallback in `/status`, and exercising the client
> behaviour end to end.

## Prerequisites

- A Sora/Nexus build of `irohad` (run with `irohad --sora --config ...`).
- Access to the configuration repository so you can edit `nexus.*` sections.
- `iroha_cli` configured to talk to the target cluster.
- `curl`/`jq` (or equivalent) to inspect the Torii `/status` payload.

## 1. Describe the lane and dataspace catalog

Declare the lanes and dataspaces that should exist on the network. The snippet
below (trimmed from `defaults/nexus/config.toml`) registers three public lanes
plus matching dataspace aliases:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Each `index` must be unique and contiguous. Dataspace ids are 64-bit values;
the examples above use the same numeric values as the lane indexes for clarity.

## 2. Set routing defaults and optional overrides

The `nexus.routing_policy` section controls the fallback lane and lets you
override routing for specific instructions or account prefixes. If no rule
matches, the scheduler routes the transaction to the configured `default_lane`
and `default_dataspace`. The router logic lives in
`crates/iroha_core/src/queue/router.rs` and applies the policy transparently to
Torii REST/gRPC surfaces.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

When you later add new lanes, update the catalog first, then extend the routing
rules. The fallback lane should continue to point at the public lane that holds

## 3. Boot a node with the policy applied

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

The node logs the derived routing policy during startup. Any validation errors
(missing indexes, duplicated aliases, invalid dataspace ids) are surfaced before
gossip begins.

## 4. Confirm lane governance state

Once the node is online, use the CLI helper to verify that the default lane is
sealed (manifest loaded) and ready for traffic. The summary view prints one row
per lane:

```bash
iroha_cli app nexus lane-report --summary
```

Example output:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

If the default lane shows `sealed`, follow the lane governance runbook before
allowing external traffic. The `--fail-on-sealed` flag is handy for CI.

## 5. Inspect Torii status payloads

The `/status` response exposes both the routing policy and the per-lane scheduler
snapshot. Use `curl`/`jq` to confirm the configured defaults and to check that
the fallback lane is producing telemetry:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Sample output:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

To inspect the live scheduler counters for lane `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

This confirms that the TEU snapshot, alias metadata, and manifest flags align
with the configuration. The same payload is what Grafana panels use for the
lane-ingest dashboard.

## 6. Exercise client defaults

- **Rust/CLI.** `iroha_cli` and the Rust client crate omit the `lane_id` field
  when you do not pass `--lane-id` / `LaneSelector`. The queue router therefore
  falls back to `default_lane`. Use explicit `--lane-id`/`--dataspace-id` flags
  only when targeting a non-default lane.
- **JS/Swift/Android.** Latest SDK releases treat `laneId`/`lane_id` as optional
  and fall back to the value advertised by `/status`. Keep the routing policy in
  sync across staging and production so mobile apps do not need emergency
  reconfigurations.
- **Pipeline/SSE tests.** The transaction event filters accept
  `tx_lane_id == <u32>` predicates (see `docs/source/pipeline.md`). Subscribe to
  `/v1/pipeline/events/transactions` with that filter to prove that writes sent
  without an explicit lane arrive under the fallback lane id.

## 7. Observability and governance hooks

- `/status` also publishes `nexus_lane_governance_sealed_total` and
  `nexus_lane_governance_sealed_aliases` so Alertmanager can warn whenever a
  lane loses its manifest. Keep those alerts enabled even for devnets.
- The scheduler telemetry map and the lane governance dashboard
  (`dashboards/grafana/nexus_lanes.json`) expect the alias/slug fields from the
  catalog. If you rename an alias, relabel the corresponding Kura directories so
  auditors keep deterministic paths (tracked under NX-1).
- Parliament approvals for default lanes should include a rollback plan. Record
  the manifest hash and governance evidence alongside this quickstart in your
  operator runbook so future rotations do not guess the required state.

Once these checks pass you can treat `nexus.routing_policy.default_lane` as the
code paths on the network.
