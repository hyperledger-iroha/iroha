---
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dea5098afdf15e92c78aba363b38f3ec8ce2018672a3d34bb1b505e2ee2f5869
source_last_modified: "2025-12-29T18:16:35.144858+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-operations
title: Nexus operations runbook
description: Field-ready summary of the Nexus operator workflow, mirroring `docs/source/nexus_operations.md`.
---

Use this page as the quick-reference sibling of
`docs/source/nexus_operations.md`. It distils the operational checklist, change
management hooks, and telemetry coverage requirements that Nexus operators must
follow.

## Lifecycle checklist

| Stage | Actions | Evidence |
|-------|--------|----------|
| Pre-flight | Verify release hashes/signatures, confirm `profile = "iroha3"`, and prepare config templates. | `scripts/select_release_profile.py` output, checksum log, signed manifest bundle. |
| Catalog alignment | Update `[nexus]` catalog, routing policy, and DA thresholds per council-issued manifest, then capture `--trace-config`. | `irohad --sora --config … --trace-config` output stored with onboarding ticket. |
| Smoke & cutover | Run `irohad --sora --config … --trace-config`, execute CLI smoke (`FindNetworkStatus`), validate telemetry exports, and request admission. | Smoke-test log + Alertmanager confirmation. |
| Steady state | Monitor dashboards/alerts, rotate keys per governance cadence, and sync configs/runbooks whenever manifests change. | Quarterly review minutes, dashboard screenshots, rotation ticket IDs. |

Detailed onboarding (key replacement, routing templates, release profile steps)
remain in `docs/source/sora_nexus_operator_onboarding.md`.

## Change management

1. **Release updates** – track announcements in `status.md`/`roadmap.md`; attach
   the onboarding checklist to every release PR.
2. **Lane manifest changes** – verify signed bundles from the Space Directory and
   archive them under `docs/source/project_tracker/nexus_config_deltas/`.
3. **Configuration deltas** – every `config/config.toml` change requires a ticket
   referencing the lane/data-space. Store a redacted copy of the effective config
   whenever nodes join or upgrade.
4. **Rollback drills** – quarterly rehearse stop/restore/smoke procedures; log
   outcomes under `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Compliance approvals** – private/CBDC lanes must secure compliance sign-off
   before modifying DA policy or telemetry redaction knobs (see
   `docs/source/cbdc_lane_playbook.md`).

## Telemetry & SLOs

- Dashboards: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, plus
  SDK-specific views (e.g., `android_operator_console.json`).
- Alerts: `dashboards/alerts/nexus_audit_rules.yml` and Torii/Norito transport
  rules (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Metrics to watch:
  - `nexus_lane_height{lane_id}` – alert on zero progress for three slots.
  - `nexus_da_backlog_chunks{lane_id}` – alert above lane-specific thresholds
    (default 64 public / 8 private).
  - `nexus_settlement_latency_seconds{lane_id}` – alert when P99 exceeds 900 ms
    (public) or 1200 ms (private).
  - `torii_request_failures_total{scheme="norito_rpc"}` – alert if 5-minute error
    ratio >2 %.
  - `telemetry_redaction_override_total` – Sev 2 immediately; ensure overrides
    have compliance tickets.
- Run the telemetry remediation checklist in
  [Nexus telemetry remediation plan](./nexus-telemetry-remediation) at least
  quarterly and attach the filled form to operations review notes.

## Incident matrix

| Severity | Definition | Response |
|----------|------------|----------|
| Sev 1 | Data-space isolation breach, settlement halt >15 min, or governance vote corruption. | Page Nexus Primary + Release Engineering + Compliance, freeze admission, collect artefacts, publish comms ≤60 min, RCA ≤5 business days. |
| Sev 2 | Lane backlog SLA breach, telemetry blind spot >30 min, failed manifest rollout. | Page Nexus Primary + SRE, mitigate ≤4 h, file follow-ups within 2 business days. |
| Sev 3 | Non-blocking drift (docs, alerts). | Log in tracker, schedule fix inside the sprint. |

Incident tickets must record affected lane/data-space IDs, manifest hashes,
timeline, supporting metrics/logs, and follow-up tasks/owners.

## Evidence archive

- Store bundles/manifests/telemetry exports under `artifacts/nexus/<lane>/<date>/`.
- Keep redacted configs + `--trace-config` output for each release.
- Attach council minutes + signed decisions when config or manifest changes land.
- Preserve weekly Prometheus snapshots relevant to Nexus metrics for 12 months.
- Record runbook edits in `docs/source/project_tracker/nexus_config_deltas/README.md`
  so auditors know when responsibilities changed.

## Related material

- Overview: [Nexus overview](./nexus-overview)
- Specification: [Nexus spec](./nexus-spec)
- Lane geometry: [Nexus lane model](./nexus-lane-model)
- Transition & routing shims: [Nexus transition notes](./nexus-transition-notes)
- Operator onboarding: [Sora Nexus operator onboarding](./nexus-operator-onboarding)
- Telemetry remediation: [Nexus telemetry remediation plan](./nexus-telemetry-remediation)
