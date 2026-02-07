---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0e832438aa843ed1ce6d018055a67bd1688ccb4cead9f73b87033469256015d
source_last_modified: "2026-01-22T16:26:46.526983+00:00"
translation_last_reviewed: 2026-02-07
title: Reserve Ledger Digest & Dashboards
description: How to turn `sorafs reserve ledger` output into telemetry, dashboards, and alerts for the Reserve+Rent policy.
---

The Reserve+Rent policy (roadmap item **SFM‑6**) now ships the `sorafs reserve`
CLI helpers plus the `scripts/telemetry/reserve_ledger_digest.py` translator so
treasury runs can emit deterministic rent/reserve transfers. This page mirrors
the workflow defined in `docs/source/sorafs_reserve_rent_plan.md` and explains
how to wire the new transfer feed into Grafana + Alertmanager so economics and
governance reviewers can audit every billing cycle.

## End-to-end workflow

1. **Quote + ledger projection**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account ih58... \
    --treasury-account ih58... \
    --reserve-account ih58... \
    --asset-definition xor#sora \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   The ledger helper attaches a `ledger_projection` block (rent due, reserve
   shortfall, top-up delta, underwriting booleans) plus the Norito `Transfer`
   ISIs needed to move XOR between the treasury and reserve accounts.

2. **Generate the digest + Prometheus/NDJSON outputs**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   The digest helper normalises micro‑XOR totals into XOR, records whether the
   projection meets underwriting, and emits the **transfer feed** metrics
   `sorafs_reserve_ledger_transfer_xor` and
   `sorafs_reserve_ledger_instruction_total`. When multiple ledgers need to be
   processed (e.g., a batch of providers), repeat `--ledger`/`--label` pairs and
   the helper writes a single NDJSON/Prometheus file containing every digest so
   dashboards ingest the entire cycle without bespoke glue. The `--out-prom`
   file targets a node-exporter textfile collector—drop the `.prom` file into
   the exporter’s watched directory or upload it to the telemetry bucket
   consumed by the Reserve dashboard job—while `--ndjson-out` feeds the same
   payloads into data pipelines.

3. **Publish artefacts + evidence**
   - Store digests under `artifacts/sorafs_reserve/ledger/<provider>/` and link
     the Markdown summary from your weekly economics report.
   - Attach the JSON digest to the rent burn-down (so auditors can replay the
     math) and include the checksum inside the governance evidence packet.
   - If the digest signals a top-up or underwriting breach, reference the alert
     IDs (`SoraFSReserveLedgerTopUpRequired`,
     `SoraFSReserveLedgerUnderwritingBreach`) and note which transfer ISIs were
     applied.

## Metrics → dashboards → alerts

| Source metric | Grafana panel | Alert / policy hook | Notes |
|---------------|---------------|---------------------|-------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | “DA Rent Distribution (XOR/hour)” in `dashboards/grafana/sorafs_capacity_health.json` | Feed the weekly treasury digest; spikes in reserve flow propagate into `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`). |
| `torii_da_rent_gib_months_total` | “Capacity Usage (GiB-months)” (same dashboard) | Pair with the ledger digest to prove the invoiced storage matches the XOR transfers. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | “Reserve Snapshot (XOR)” + status cards in `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerTopUpRequired` fires when `requires_top_up=1`; `SoraFSReserveLedgerUnderwritingBreach` fires when `meets_underwriting=0`. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | “Transfers by Kind”, “Latest Transfer Breakdown”, and the coverage cards in `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing`, and `SoraFSReserveLedgerTopUpTransferMissing` warn when the transfer feed is absent or zeroed even though rent/top-up is required; the coverage cards fall to 0% in the same cases. |

When a rent cycle completes, refresh the Prometheus/NDJSON snapshots, confirm
that the Grafana panels pick up the new `label`, and attach screenshots +
Alertmanager IDs to the rent governance packet. This proves the CLI projection,
telemetry, and governance artefacts all stem from the **same** transfer feed and
keeps the roadmap’s economics dashboards aligned with the Reserve+Rent
automation. The coverage cards should read 100% (or 1.0) and the new alerts
should clear once rent and reserve top-up transfers are present in the digest.
