---
lang: uz
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e992cea8d0c835b30bd9e91860f6b6f87bed79a2c25bd6d0544639685834f80c
source_last_modified: "2025-12-29T18:16:35.146583+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-settlement-faq
title: Settlement FAQ
description: Operator-facing answers covering settlement routing, XOR conversion, telemetry, and audit evidence.
---

This page mirrors the internal settlement FAQ (`docs/source/nexus_settlement_faq.md`)
so portal readers can review the same guidance without digging through the
mono-repo. It explains how the Settlement Router processes payouts, what metrics
to monitor, and how SDKs should integrate the Norito payloads.

## Highlights

1. **Lane mapping** — each data space declares a `settlement_handle`
   (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody`, or
   `xor_dual_fund`). Consult the latest lane catalog under
   `docs/source/project_tracker/nexus_config_deltas/`.
2. **Deterministic conversion** — the router converts all settlements to XOR via
   the governance-approved liquidity sources. Private lanes pre-fund XOR buffers;
   haircuts apply only when buffers drift outside policy.
3. **Telemetry** — watch `nexus_settlement_latency_seconds`, conversion counters,
   and haircut gauges. Dashboards live in `dashboards/grafana/nexus_settlement.json`
   and alerts in `dashboards/alerts/nexus_audit_rules.yml`.
4. **Evidence** — archive configs, router logs, telemetry exports, and
   reconciliation reports for audits.
5. **SDK responsibilities** — every SDK must expose settlement helpers, lane IDs,
   and Norito payload encoders to keep parity with the router.

## Example flows

| Lane type | Evidence to capture | What it proves |
|-----------|--------------------|----------------|
| Private `xor_hosted_custody` | Router log + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC buffers debit deterministic XOR and haircuts stay within policy. |
| Public `xor_global` | Router log + DEX/TWAP reference + latency/conversion metrics | Shared liquidity path priced the transfer at the published TWAP with zero haircut. |
| Hybrid `xor_dual_fund` | Router log showing public vs shielded split + telemetry counters | Shielded/public mix respected governance ratios and recorded the haircut applied to each leg. |

## Need more detail?

- Full FAQ: `docs/source/nexus_settlement_faq.md`
- Settlement router spec: `docs/source/settlement_router.md`
- CBDC policy playbook: `docs/source/cbdc_lane_playbook.md`
- Operations runbook: [Nexus operations](./nexus-operations)
