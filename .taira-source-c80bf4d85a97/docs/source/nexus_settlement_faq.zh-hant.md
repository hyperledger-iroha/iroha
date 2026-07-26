---
lang: zh-hant
direction: ltr
source: docs/source/nexus_settlement_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9ccbc52b2d34a410c5b724b6421fc91bd403cd40d0a03360315a2ae2e3e504ec
source_last_modified: "2025-12-29T18:16:35.994994+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus Settlement FAQ

**Roadmap link:** NX-14 — Nexus documentation & operator runbooks  
**Status:** Drafted 2026-03-24 (mirrors settlement router & CBDC playbook specs)  
**Audience:** Operators, SDK authors, and governance reviewers preparing for the
Nexus (Iroha 3) launch.

This FAQ answers the questions that surfaced during the NX-14 review about
settlement routing, XOR conversion, telemetry, and audit evidence. Refer to
`docs/source/settlement_router.md` for the full specification and
`docs/source/cbdc_lane_playbook.md` for CBDC-specific policy knobs.

> **TL;DR:** All settlement flows clear through the Settlement Router, which
> debits XOR buffers on public lanes and applies lane-specific fees. Operators
> must keep routing config (`config/config.toml`), telemetry dashboards, and
> audit logs in sync with the published manifests.

## Frequently asked questions

### Which lanes handle settlement, and how do I know where my DS fits?

- Every data space declares a `settlement_handle` in its manifest. Default
  handles map as follows:
  - `xor_global` for default public lanes.
  - `xor_lane_weighted` for public custom lanes that source liquidity elsewhere.
  - `xor_hosted_custody` for private/CBDC lanes (escrowed XOR buffer).
  - `xor_dual_fund` for hybrid/confidential lanes that mix shielded + public
    flows.
- Inspect `docs/source/nexus_lanes.md` for lane classes and
  `docs/source/project_tracker/nexus_config_deltas/*.md` for the latest catalog
  approvals. `irohad --sora --config … --trace-config` prints the effective
  catalog at runtime for audits.

### How does the Settlement Router determine conversion rates?

- The router enforces a single path with deterministic pricing:
  - For public lanes we use the on-chain XOR liquidity pool (public DEX). Price
    oracles fall back to the governance-approved TWAP when liquidity is thin.
  - Private lanes pre-fund XOR buffers. When they debit a settlement, the router
    logs the conversion tuple `{lane_id, source_token, xor_amount, haircut}` and
    applies governance-approved haircuts (`haircut.rs`) if buffers drift.
- Configuration lives under `[settlement]` in `config/config.toml`. Avoid custom
  edits unless instructed by governance. Reference
  `docs/source/settlement_router.md` for field descriptions.

### How are fees and rebates applied?

- Fees are expressed per lane in the manifest:
  - `base_fee_bps` — applies to every settlement debit.
  - `liquidity_haircut_bps` — compensates shared liquidity providers.
  - `rebate_policy` — optional (e.g., CBDC promotional rebates).
- The router emits `SettlementApplied` events (Norito format) containing fee
  breakdowns so SDKs and auditors can reconcile ledger entries.

### What telemetry proves that settlements are healthy?

- Prometheus metrics (exported via `iroha_telemetry` and settlement router):
  - `nexus_settlement_latency_seconds{lane_id}` — P99 must stay below 900 ms for
    public lanes / 1200 ms for private lanes.
  - `settlement_router_conversion_total{source_token}` — confirms conversion
    volumes per token.
  - `settlement_router_haircut_total{lane_id}` — alert when non-zero without an
    associated governance note.
  - `iroha_settlement_buffer_xor{lane_id,dataspace_id}` — shows live per-lane XOR debits (micro units). Alert when <25 %/10 % of Bmin.
  - `iroha_settlement_pnl_xor{lane_id,dataspace_id}` — realised haircut variance to reconcile with treasury P&L.
  - `iroha_settlement_haircut_bp{lane_id,dataspace_id}` — effective epsilon applied in the latest block; auditors compare against router policy.
  - `iroha_settlement_swapline_utilisation{lane_id,dataspace_id,profile}` — sponsor/MM credit line usage; alert above 80 %.
- `nexus_lane_block_height{lane,dataspace}` — latest block height observed for the lane/dataspace pair; keep neighboring peers within a few slots of one another.
- `nexus_lane_finality_lag_slots{lane,dataspace}` — slots separating the global head and the most recent block for that lane; alert when >12 outside drills.
- `nexus_lane_settlement_backlog_xor{lane,dataspace}` — backlog waiting to settle in XOR; gate CBDC/private workloads before exceeding regulator thresholds.

`settlement_router_conversion_total` carries the `lane_id`, `dataspace_id`, and
`source_token` labels so you can prove which gas asset drove each conversion.
`settlement_router_haircut_total` accumulates XOR units (not raw micro
amounts), letting Treasury reconcile the haircut ledger directly from
Prometheus.
- `lane_settlement_commitments[*].swap_metadata.volatility_class` shows whether
  the router applied the `stable`, `elevated`, or `dislocated` margin bucket.
  Elevated/dislocated entries must link to the incident log or governance note.
- Dashboards: `dashboards/grafana/nexus_settlement.json` plus the
  `nexus_lanes.json` overview. Tie alerts to `dashboards/alerts/nexus_audit_rules.yml`.
- When settlement telemetry degrades, log the incident per the runbook in
  `docs/source/nexus_operations.md`.

### How do I export lane telemetry for regulators?

Run the helper below whenever a regulator requests the lane table:

```
cargo xtask nexus-lane-audit \
  --status artifacts/status.json \
  --json-out artifacts/nexus_lane_audit.json \
  --parquet-out artifacts/nexus_lane_audit.parquet \
  --markdown-out artifacts/nexus_lane_audit.md \
  --captured-at 2026-02-12T09:00:00Z \
  --lane-compliance artifacts/lane_compliance_evidence.json
```

* `--status` accepts the JSON blob returned by `iroha status --format json`.
* `--json-out` captures a canonical JSON array per lane (aliases, dataspace,
  block height, finality lag, TEU capacity/utilization, scheduler trigger +
  utilisation counters, RBC throughput, backlog, governance metadata, etc.).
* `--parquet-out` writes the same payload as a Parquet file (Arrow schema),
  ready for regulators that require columnar evidence.
* `--markdown-out` emits a human-readable summary that flags lagging lanes,
  non-zero backlog, missing compliance evidence, and pending manifests; defaults
  to `artifacts/nexus_lane_audit.md`.
* `--lane-compliance` is optional; when supplied it must point at the JSON
  manifest described in the compliance doc so the exported rows embed the
  matching lane policy, reviewer signatures, metrics snapshot, and audit log
  excerpts.

Archive both outputs under `artifacts/` with the routed-trace evidence (screens
from `nexus_lanes.json`, Alertmanager state, and `nexus_lane_rules.yml`).

### What evidence do auditors expect?

1. **Config snapshot** — capture `config/config.toml` with `[settlement]` section
   and the lane catalog referenced by the current manifest.
2. **Router logs** — archive `settlement_router.log` daily; it includes hashed
   settlement IDs, XOR debits, and where haircuts were applied.
3. **Telemetry exports** — weekly snapshot of the metrics mentioned above.
4. **Reconciliation report** — optional but recommended: export
   `SettlementRecordV1` entries (see `docs/source/cbdc_lane_playbook.md`) and
   compare against treasury ledger.

### Do SDKs need special handling for settlement?

- SDKs must:
  - Provide helpers for querying settlement events (`/v1/settlement/records`)
    and interpreting `SettlementApplied` logs.
  - Surface lane IDs + settlement handles in client configuration so operators
    can route transactions correctly.
  - Mirror the Norito payloads defined in `docs/source/settlement_router.md`
    (e.g., `SettlementInstructionV1`) with end-to-end tests.
- The Nexus SDK quickstart (next section) details per-language snippets for
  onboarding to the public network.

### How do settlements interact with governance or emergency brakes?

- Governance can pause specific settlement handles via manifest updates. The
  router respects the `paused` flag and rejects new settlements with a
  deterministic error (`ERR_SETTLEMENT_PAUSED`).
- Emergency “haircut clamps” limit maximum XOR debits per block to prevent
  draining shared buffers.
- Operators must monitor `governance.settlement_pause_total` and follow the
  incident template in `docs/source/nexus_operations.md`.

### Where do I report bugs or request changes?

- Feature gaps → open an issue tagged `NX-14` and cross-link to the roadmap.
- Urgent settlement incidents → page the Nexus primary (see
  `docs/source/nexus_operations.md`) and attach router logs.
- Documentation corrections → file PRs against this file and the portal
  counterparts (`docs/portal/docs/nexus/overview.md`,
  `docs/portal/docs/nexus/operations.md`).

### Can you show example settlement flows?

The following snippets show what auditors expect for the most common lane
types. Capture the router log, ledger hashes, and the matching telemetry export
for each scenario so reviewers can replay the evidence.

#### Private CBDC lane (`xor_hosted_custody`)

Below is a trimmed router log for a private CBDC lane using the hosted custody
handle. The log proves deterministic XOR debits, fee composition, and telemetry
IDs:

```text
2026-03-24T11:42:07Z settlement_router lane=3 dataspace=ds::cbdc::jp
    handle=xor_hosted_custody settlement_id=0x9c2f...a413
    source_token=JPYCBDC amount=125000.00
    xor_debited=312.500000 xor_rate=400.000000 haircut_bps=25 base_fee_bps=15
    fee_breakdown={base=0.046875, haircut=0.078125}
    ledger_tx=0x7ab1...ff11 telemetry_trace=nexus-settle-20260324T1142Z-lane3
```

In Prometheus you should see the matching metrics:

```text
nexus_settlement_latency_seconds{lane_id="3"} 0.842
settlement_router_conversion_total{lane_id="3",source_token="JPYCBDC"} += 1
settlement_router_haircut_total{lane_id="3"} += 0.078125
```

Archive the log snippet, ledger transaction hash, and metrics export together so
auditors can reconstruct the flow. The next examples show how to record evidence
for public and hybrid/confidential lanes.

#### Public lane (`xor_global`)

Public data spaces route through `xor_global`, so the router debits the shared
DEX buffer and records the live TWAP that priced the transfer. Attach the TWAP
hash or governance note whenever the oracle falls back to a cached value.

```text
2026-03-25T08:11:04Z settlement_router lane=0 dataspace=ds::public::creator
    handle=xor_global settlement_id=0x81cc...991c
    source_token=XOR amount=42.000000
    xor_debited=42.000000 xor_rate=1.000000 haircut_bps=0 base_fee_bps=10
    fee_breakdown={base=0.004200, liquidity=0.000000}
    dex_twap_id=twap-20260325T0810Z ledger_tx=0x319e...dd72 telemetry_trace=nexus-settle-20260325T0811Z-lane0
```

Metrics prove the same flow:

```text
nexus_settlement_latency_seconds{lane_id="0"} 0.224
settlement_router_conversion_total{lane_id="0",source_token="XOR"} += 1
settlement_router_haircut_total{lane_id="0"} += 0
```

Save the TWAP record, router log, telemetry snapshot, and ledger hash in the
same evidence bundle. When alerts fire for lane 0 latency or TWAP freshness,
link the incident ticket to this bundle.

#### Hybrid/confidential lane (`xor_dual_fund`)

Hybrid lanes mix shielded buffers with public XOR reserves. Every settlement
must show which bucket sourced the XOR and how the haircut policy split the
fees. The router log exposes those details via the dual-fund metadata block:

```text
2026-03-26T19:54:31Z settlement_router lane=9 dataspace=ds::hybrid::art
    handle=xor_dual_fund settlement_id=0x55d2...c0ab
    source_token=ARTCREDIT amount=9800.00
    xor_debited_public=12.450000 xor_debited_shielded=11.300000
    xor_rate_public=780.000000 xor_rate_shielded=820.000000
    haircut_bps=35 base_fee_bps=20 dual_fund_ratio=0.52
    fee_breakdown={base=0.239000, haircut=0.418750}
    ledger_tx=0xa924...1104 telemetry_trace=nexus-settle-20260326T1954Z-lane9
```

```text
nexus_settlement_latency_seconds{lane_id="9"} 0.973
settlement_router_conversion_total{lane_id="9",source_token="ARTCREDIT"} += 1
settlement_router_haircut_total{lane_id="9"} += 0.418750
```

Archive the router log alongside the dual-fund policy (governance catalog
extract), `SettlementRecordV1` export for the lane, and the telemetry snippet so
auditors can confirm the shielded/public split respected the governance limits.

Keep this FAQ updated whenever settlement router behaviour changes or when
governance introduces new lane classes/fee policies.
