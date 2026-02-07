---
lang: dz
direction: ltr
source: docs/source/quickstart/default_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 365c4e7bf4f1968d0dc8562e13be9a484c74b94f74b4117fe8dc4c548a62013e
source_last_modified: "2026-01-22T14:45:02.162646+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

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

Instruction matchers are case-insensitive. For boxed ISIs you can target the
family or a specific variant using `family::variant` (for example,
`register::domain` or `transfer::asset_definition`). The `smartcontract::deploy`
matcher targets smart contract code registration instructions emitted by
`/v1/contracts/deploy`.

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
iroha_cli app nexus lane-report --summary --only-missing --fail-on-sealed
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

After lane governance is ready, run the Nexus lane smoke helper (shipped under
`scripts/nexus_lane_smoke.py`) to validate `/v1/sumeragi/status` plus the
Prometheus exports. Lanes flagged as `manifest_required=true` now need a
non-empty `manifest_path` in the status payload—the helper fails if the manifest
reference is missing so release bundles always capture the signed artefact path:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.org/v1/sumeragi/status \
  --metrics-url https://torii.example.org/metrics \
  --lane-alias core \
  --expected-lane-count 3 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Set `--max-headroom-events 0` to ensure the scheduler’s headroom telemetry has not recorded any
load-shedding events, and use `--max-slot-p95/--max-slot-p99` (plus `--min-slot-samples`) to keep
the NX-18 slot-duration SLO enforced before you expose the default lane to production traffic.

For air-gapped rehearsals you can pass `--status-file` / `--metrics-file` and
reuse the recorded fixtures in `fixtures/nexus/lanes/`. The helper exits
non-zero when the scheduler gauges or lane governance snapshots don’t line up
with the catalog, making it ideal for gating CI.

Finally, if you renamed an existing lane, capture the corresponding telemetry
event so auditors can trace the relabel: `journalctl -u irohad -o json |
jq 'select(.msg=="nexus.lane.topology")'`. The payload includes the old/new
aliases and slugs, proving Kura and the tiered-state backend moved the storage
directories onto the canonical slug. Feed that NDJSON log into
`scripts/nexus_lane_smoke.py --from-telemetry telemetry.ndjson` (optionally with
`--require-alias-migration old:new`) to prove the rename event was recorded in
the automation-friendly evidence bundle.

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
