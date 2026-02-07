---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Taikai Anchor Observability Runbook

This portal copy mirrors the canonical runbook in
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md).
Use it when rehearsing SN13-C routing-manifest (TRM) anchors so SoraFS/SoraNet
operators can correlate spool artefacts, Prometheus telemetry, and governance
evidence without leaving the portal preview build.

## Scope & Owners

- **Program:** SN13-C — Taikai manifests & SoraNS anchors.
- **Owners:** Media Platform WG, DA Program, Networking TL, Docs/DevRel.
- **Goal:** Provide a deterministic playbook for Sev 1/Sev 2 alerts, telemetry
  validation, and evidence capture while Taikai routing manifests roll forward
  across aliases.

## Quickstart (Sev 1/Sev 2)

1. **Capture spool artefacts** — copy the latest
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json`, and
   `taikai-lineage-*.json` files from
   `config.da_ingest.manifest_store_dir/taikai/` before restarting workers.
2. **Dump `/status` telemetry** — record the
   `telemetry.taikai_alias_rotations` array to prove which manifest window is
   active:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **Check dashboards and alerts** — load
   `dashboards/grafana/taikai_viewer.json` (cluster + stream filters) and note
   whether any rules in
   `dashboards/alerts/taikai_viewer_rules.yml` fired (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, SoraFS proof-health events).
4. **Inspect Prometheus** — run the queries in §“Metric reference” to confirm
   ingest latency/drift and alias-rotation counters behave as expected. Escalate
   if `taikai_trm_alias_rotations_total` stalls for multiple windows or if
   error counters increase.

## Metric reference

| Metric | Purpose |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | CMAF ingest latency histogram per cluster/stream (target: p95 < 750 ms, p99 < 900 ms). |
| `taikai_ingest_live_edge_drift_ms` | Live-edge drift between encoder and anchor workers (pages at p99 > 1.5 s for 10 min). |
| `taikai_ingest_segment_errors_total{reason}` | Error counters by reason (`decode`, `manifest_mismatch`, `lineage_replay`, …). Any increase triggers `TaikaiIngestFailure`. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | Increments whenever `/v1/da/ingest` accepts a new TRM for an alias; use `rate()` to validate rotation cadence. |
| `/status → telemetry.taikai_alias_rotations[]` | JSON snapshot with `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total`, and timestamps for evidence bundles. |
| `taikai_viewer_*` (rebuffer, CEK rotation age, PQ health, alerts) | Viewer-side KPIs to ensure CEK rotation + PQ circuits remain healthy during anchors. |

### PromQL snippets

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## Dashboards & alerts

- **Grafana viewer board:** `dashboards/grafana/taikai_viewer.json` — p95/p99
  latency, live-edge drift, segment errors, CEK rotation age, viewer alerts.
- **Grafana cache board:** `dashboards/grafana/taikai_cache.json` — hot/warm/cold
  promotions and QoS denials when alias windows rotate.
- **Alertmanager rules:** `dashboards/alerts/taikai_viewer_rules.yml` — drift
  paging, ingest failure warnings, CEK rotation lag, and SoraFS proof-health
  penalties/cooldowns. Ensure receivers exist for every production cluster.

## Evidence bundle checklist

- Spool artefacts (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- Run `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>` to emit a signed JSON inventory of pending/delivered envelopes and copy request/SSM/TRM/lineage files into the drill bundle. The default spool path is `storage/da_manifests/taikai` from `torii.toml`.
- `/status` snapshot covering `telemetry.taikai_alias_rotations`.
- Prometheus exports (JSON/CSV) for the metrics above over the incident window.
- Grafana screenshots with filters visible.
- Alertmanager IDs referencing the relevant rule fires.
- Link to `docs/examples/taikai_anchor_lineage_packet.md` describing the
  canonical evidence packet.

## Dashboard mirroring & drill cadence

Satisfying the SN13-C roadmap requirement means proving that the Taikai
viewer/cache dashboards are reflected inside the portal **and** that the anchor
evidence drill runs on a predictable cadence.

1. **Portal mirroring.** Whenever `dashboards/grafana/taikai_viewer.json` or
   `dashboards/grafana/taikai_cache.json` changes, summarise the deltas in
   `sorafs/taikai-monitoring-dashboards` (this portal) and note the JSON
   checksums in the portal PR description. Highlight new panels/thresholds so
   reviewers can correlate with the managed Grafana folder.
2. **Monthly drill.**
   - Run the drill on the first Tuesday of each month at 15:00 UTC so evidence
     lands before the SN13 governance sync.
   - Capture spool artefacts, `/status` telemetry, and Grafana screenshots inside
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - Log the execution with
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **Review & publish.** Within 48 hours, review alerts/false positives with the
   DA Program + NetOps, record follow-up items in the drill log, and link the
   governance bucket upload from `docs/source/sorafs/runbooks-index.md`.

If either dashboards or drills fall behind, SN13-C cannot exit 🈺; keep this
section up to date whenever cadence or evidence expectations change.

## Helpful commands

```bash
# Snapshot alias rotation telemetry to an artefact directory
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# List spool entries for a specific alias/event
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# Inspect TRM mismatch reasons from the spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

Keep this portal copy in sync with the canonical runbook whenever Taikai
anchoring telemetry, dashboards, or governance evidence requirements change.
