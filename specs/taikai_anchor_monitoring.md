<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Anchor Observability & Runbook

_Status: Draft update 2026-04-05 (SN13-C Manifests & SoraNS anchors)_ — Owners:
Media Platform WG / DA Program / Networking TL

This runbook explains how to monitor the Taikai routing-manifest (TRM)
anchoring path once `/v1/da/ingest` begins persisting `taikai-trm-state-*`
ledgers and emitting alias rotation telemetry. It complements the envelope
spec in `specs/taikai_segment_envelope.md` and the evidence template in
`fixtures/documentation/taikai_anchor_lineage_packet.md` by focusing on the operational
signals exported by Torii and the SoraFS orchestrator.

## Quickstart (Sev 1 / Sev 2)

1. **Capture the active spool artefacts.** Copy the most recent
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json`, and
   `taikai-lineage-*.json` entries, verified `taikai-anchor-*.ok` receipts, and
   any quarantined `taikai-anchor-*.ok.invalid-*` markers from the ingest spool
   before rotating any worker pods. These files live under
   `config.da_ingest.manifest_store_dir/taikai/` and are referenced by the
   governance evidence bundle.
2. **Check alias rotation telemetry.** Hit the Torii status endpoint and record
   the `taikai_alias_rotations` array to prove which TRM window is active:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
   Compare the reported `manifest_digest_hex` and `window_*` fields against the
   spool artefacts. Divergence is treated as Sev 2.
3. **Verify ingest health dashboards.** Load the Taikai Viewer board
   (`dashboards/grafana/taikai_viewer.json`) filtered to the affected cluster
   and confirm `Taikai Live Edge Drift`, `Segment Latency`, and
   `Segment Errors` panels are within SLO. Capture screenshots for the
   incident record.
4. **Inspect Prometheus directly when needed.** Use the PromQL snippets in
   §4 to check histogram quantiles and the alias rotation counter. If
   `taikai_ingest_errors_total{reason!="none"}` increases or
   `taikai_trm_alias_rotations_total` stalls for more than two windows,
   escalate to the DA program.
5. **Reference alerting state.** Confirm whether any rules in
   `dashboards/alerts/taikai_viewer_rules.yml` fired (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, or SoraFS proof-health
   alerts). Attach Alertmanager IDs to the incident doc.

## Metric inventory

| Metric | Source | Purpose / Threshold |
| --- | --- | --- |
| `taikai_ingest_segment_latency_ms` (histogram) | `crates/iroha_telemetry/src/metrics.rs:9065` | Measures CMAF ingest latency per cluster/stream. Keep p95 < 750 ms, p99 < 900 ms. Drives the "Segment Latency" panel and `TaikaiLiveEdgeDrift` alert indirectly. |
| `taikai_ingest_live_edge_drift_ms` (histogram) | `crates/iroha_telemetry/src/metrics.rs:9076` | Tracks live-edge drift between encoder and anchor workers. `TaikaiLiveEdgeDrift` pages when p99 > 1.5 s for 10 min. |
| `taikai_ingest_errors_total{reason}` | `crates/iroha_telemetry/src/metrics.rs` | Counts failed segments by reason (`decode`, `manifest_mismatch`, `lineage_replay`, etc.). Any increase triggers the `TaikaiIngestFailure` warning. |
| `taikai_trm_alias_rotations_total` | `crates/iroha_torii/src/da/taikai.rs:1969`, `crates/iroha_telemetry/src/metrics.rs:11672` | Increments when `/v1/da/ingest` accepts a new TRM per alias. Use it to prove rotation cadence and detect stalled windows. |
| `telemetry.taikai_alias_rotations[]` snapshot | `crates/iroha_telemetry/src/metrics.rs:2047` | JSON payload surfaced via `/status` showing `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total`, and timestamps per alias. Required for governance evidence. |
| `taikai_viewer_*` metrics (`*_rebuffer_events_total`, `*_cek_rotation_seconds_ago`, `*_alerts_firing_total`) | `crates/iroha_telemetry/src/metrics.rs:5681-5693` | Viewer health telemetry surfacing CEK rotation age, PQ circuit percentages, and alert counts. Map to the viewer dashboard and CEK rotation warning. |

## Dashboard coverage

- **Grafana — `dashboards/grafana/taikai_viewer.json`:** ships cluster + stream
  selectors and panels for p95/p99 ingest latency, live-edge drift, segment
  error rates, CEK rotation age, and viewer alert rollups. Import into Grafana
  or reference the managed folder to review JSON edits.
- **Grafana — `dashboards/grafana/taikai_cache.json`:** monitors hot/warm/cold
  cache tiers. Use this when alias rotations trigger cache promotion spikes or
  QoS denials. Panels reference `sorafs_taikai_cache_*` and
  `sorafs_taikai_qos_denied_total` metrics.
- **Alertmanager rules — `dashboards/alerts/taikai_viewer_rules.yml`:** define
  the paging criteria for drift, ingest failures, CEK rotation lag, and SoraFS
  proof-health penalties. Confirm every production cluster wires these rules
  into Alertmanager before enabling automatic alias rotation.

## Runbook procedures

### 1. Inspect spool and lineage state

- List TRM state files under
  `config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-*.json`. Each
  file pairs an alias namespace/name with the last accepted window. Example:
  ```bash
  jq '{alias_namespace,alias_name,window_start_sequence,window_end_sequence,manifest_digest_hex}' \
    "$MANIFEST_DIR/taikai/taikai-trm-state-docs.sora.json"
  ```
- Review the most recent `taikai-anchor-request-<slug>.json` payload to
  confirm `lineage_hint.previous_*` matches the TRM state file.
- Verify each `taikai-anchor-<slug>.ok` against its exact request capture and
  the configured `torii.da_ingest.taikai_anchor.receipt_public_key`; an HTTP
  success or a marker file by itself is not proof of delivery.
- Treat `taikai-anchor-<slug>.ok.invalid-<blake3>` as evidence of a rejected
  legacy or invalid acknowledgement. Torii quarantines these markers without
  retiring the source upload, then continues processing other batch items.
- If overlap or stale digests are detected, block further ingest for that
  alias and open an SN13-C incident.

### 2. Query Prometheus for ingest health

- Latency / drift p99 checks:
  ```promql
  histogram_quantile(
    0.99,
    sum by (le) (
      rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
    )
  )
  ```
  and
  ```promql
  histogram_quantile(
    0.99,
    sum by (le) (
      rate(taikai_ingest_live_edge_drift_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
    )
  )
  ```
- Error breakdown per reason:
  ```promql
  sum by (reason) (
    rate(taikai_ingest_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
  ```
- Alias rotation counter for a specific alias:
  ```promql
  rate(
    taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
  )
  ```
  Non-zero rate without matching spool updates indicates telemetry/export
  skew—capture evidence and page the ingest owner.

### 3. Validate `/status` snapshots

- The Torii status payload exposes the `telemetry.taikai_alias_rotations`
  array. Use `jq` to compare the JSON snapshot with the spool contents and
  the lineage packet that will be attached to the governance evidence bundle:
  ```bash
  curl -sSf "$TORII/status" \
    | jq '.telemetry.taikai_alias_rotations[] | select(.alias_name=="docs")'
  ```
- Record the JSON object (or attach it verbatim) whenever an alias is
  rotated. Governance reviewers expect the `rotations_total` counter to match
  the Prometheus counter value.

### 4. Confirm alert and dashboard wiring

- Make sure Alertmanager targets exist for every rule defined in
  `dashboards/alerts/taikai_viewer_rules.yml`. Missing receivers block SN13-C
  promotion.
- When testing new clusters, run `test_sorafs_fetch_alerts.sh` and the Taikai
  viewer smoke harness to trigger synthetic drift/error spikes. Verify alerts
  fire and auto-close once the condition clears.
- Update the portal or runbook changelog with screenshots whenever new panels
  or thresholds are added so downstream teams inherit the latest guidance.

## Evidence bundle checklist

- Spool artefacts (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`, verified `taikai-anchor-*.ok`, and quarantined
  `taikai-anchor-*.ok.invalid-*`) copied to the incident bundle directory.
- Run `cargo xtask taikai-anchor-bundle` with the spool and copy options shown
  below, the anchor key passed through `--receipt-public-key`, and the optional
  evidence-bundle signer passed through `--signing-key`. This emits a signed
  JSON inventory and copies request/SSM/TRM/lineage files into the drill bundle.
  The receipt key is the same public key configured in Torii. The default spool
  path is `storage/da_manifests/taikai` from `torii.toml`.
- `/status` snapshot covering `telemetry.taikai_alias_rotations`.
- Prometheus exports (raw JSON or CSV) for the metrics listed above during the
  affected window.
- Grafana screenshots (latency, drift, errors, CEK rotation panels) with the
  cluster/stream filters visible.
- Alertmanager IDs (if any) referencing the relevant rule from
  `taikai_viewer_rules.yml`.
- Reference to the Taikai lineage packet template:
  `fixtures/documentation/taikai_anchor_lineage_packet.md`.

### Bundle automation

Use the xtask helper to gather spool artefacts, emit a JSON summary, and
optionally sign it with an Ed25519 key before uploading the drill packet:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --receipt-public-key <anchor-ed25519-multihash> \
  --signing-key <ed25519-key-path-optional>
```

The summary records one of four statuses for each anchor:

- `pending`: no `.ok` receipt exists.
- `receipt_unverified`: a receipt exists, but `--receipt-public-key` was not
  supplied. This status must never be interpreted as delivered.
- `invalid_receipt`: the receipt cannot be matched to the canonical `base_id`,
  exact request capture, schema/version, or pinned signature. The entry includes
  `receipt_validation_error`.
- `delivered`: the receipt binds the canonical `base_id`, the BLAKE3 digest of
  the exact `taikai-anchor-request-*` bytes, and verifies under the supplied
  Ed25519 key.

The summary also records hashes for every `taikai-anchor-request-*`,
`taikai-trm-state-*`, `taikai-lineage-*`, envelope, and receipt file so
governance reviewers can diff the packet quickly. Omitting the receipt key is
useful for inventory only; it cannot produce a `delivered` classification.

## Dashboard mirroring & drill cadence

The SN13-C readiness gate requires two recurring actions: keeping public Taikai
operator guidance current in `iroha-docs` and exercising the evidence drill on
a predictable cadence.

1. **Public documentation.** After each dashboard JSON update
   (`dashboards/grafana/taikai_viewer.json` and
   `dashboards/grafana/taikai_cache.json`), coordinate the public highlights in
   the sibling `iroha-docs` repository. Include the SHA-256 digest of the JSON
   bundle in that PR and call out any new panels or thresholds so governance
   reviewers can diff the managed Grafana folder against the repository version.
2. **Monthly drill cadence.**
   - Schedule the drill for the first Tuesday of every month at 15:00 UTC so it
     lands before the SN13 governance sync.
   - Capture spool artefacts, `/status` snapshots, and Grafana screenshots into
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - Append the run to the drill ledger via
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
   - File the evidence bundle link in `specs/sorafs/runbooks-index.md`
     (Taikai section) once uploads land in the governance bucket.
3. **Follow-up review.** Within 48 hours, review the drill outcomes with the DA
   Program and NetOps. Note false positives, alert latency, and remediation
   follow-ups in the drill log entry so the next iteration can adjust thresholds.

These steps keep the roadmap requirement (“viewer/cache dashboards mirrored in
the portal and rehearsed evidence drills”) continuously satisfied even as the
dashboards evolve.

## Useful commands

```bash
# Dump alias rotation status to a dated artefact
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# List spool directories that belong to the current event/alias
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' \
  | sort

# Inspect TRM mismatch reasons directly from the spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

Keep this document alongside the Taikai cache and viewer dashboards when
running SN13-C rehearsals so operators can capture deterministic telemetry
records for every anchor rotation.
