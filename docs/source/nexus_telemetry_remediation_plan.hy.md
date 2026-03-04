---
lang: hy
direction: ltr
source: docs/source/nexus_telemetry_remediation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19d46f99e2ba79c56cbc3af65b47f5fb6997fa66f8ee951806b21696418a1d7b
source_last_modified: "2025-12-29T18:16:35.995522+00:00"
translation_last_reviewed: 2026-02-07
---

% Nexus Telemetry Remediation Plan (Phase B2)

# Overview

Roadmap item **B2 — telemetry gap ownership** requires a published plan that
ties every outstanding Nexus telemetry gap to a signal, alert guardrail, owner,
deadline, and verification artifact before the Q1 2026 audit windows begin.
This document centralises that matrix so release engineering, telemetry ops,
and SDK owners can confirm coverage ahead of the routed-trace and
`TRACE-TELEMETRY-BRIDGE` rehearsals.

# Gap Matrix

| Gap ID | Signal & Alert Guardrail | Owner / Escalation | Due (UTC) | Evidence & Verification |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | `torii_lane_admission_latency_seconds{lane_id,endpoint}` histogram with alert **`SoranetLaneAdmissionLatencyDegraded`** firing when `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` for 5 minutes (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (signal) + `@telemetry-ops` (alert) — escalate via Nexus routed-trace on-call. | 2026‑02‑23 | Alert tests under `dashboards/alerts/tests/soranet_lane_rules.test.yml` and the `TRACE-LANE-ROUTING` rehearsal capture showing the fired/recovered alert plus Torii `/metrics` scrape archived in `docs/source/nexus_transition_notes.md`. |
| `GAP-TELEM-002` | Counter `nexus_config_diff_total{knob,profile}` with guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` gating deploys (`docs/source/telemetry.md`). | `@nexus-core` (instrumentation) → `@telemetry-ops` (alert) — governance duty officer paged when counter increments unexpectedly. | 2026‑02‑26 | Governance dry-run outputs stored next to `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; the release checklist includes the Prometheus query screenshot plus log excerpt proving `StateTelemetry::record_nexus_config_diff` emitted the diff. |
| `GAP-TELEM-003` | Event `TelemetryEvent::AuditOutcome` (metric `nexus.audit.outcome`) with alert **`NexusAuditOutcomeFailure`** when failures or missing outcomes persist for >30 minutes (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) with escalation to `@sec-observability`. | 2026‑02‑27 | CI gate `scripts/telemetry/check_nexus_audit_outcome.py` archives NDJSON payloads and fails when a TRACE window lacks a success event; alert screenshots attached to the routed-trace report. |
| `GAP-TELEM-004` | Gauge `nexus_lane_configured_total` monitored with guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` (documented in `docs/source/telemetry.md`) feeding the SRE on-call checklist. | `@telemetry-ops` (gauge/export) with escalation to `@nexus-core` when nodes report inconsistent catalog sizes. | 2026‑02‑28 | Scheduler telemetry test `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` proves emission; operators attach Prometheus diff + `StateTelemetry::set_nexus_catalogs` log excerpt to the TRACE rehearsal package. |

# Export budget and OTLP limits

- **Decision (2026-02-11):** cap OTLP exporters to **5 MiB/min per node** or
  **25 000 spans/min**, whichever is lower, with a 256-span batch size and a
  10 second export timeout. Exports above 80 % of the cap trigger the
  `NexusOtelExporterSaturated` alert in `dashboards/alerts/nexus_telemetry_rules.yml`
  and emit a `telemetry_export_budget_saturation` event for audit logs.
- **Enforcement:** Prometheus rules read the `iroha.telemetry.export.bytes_total`
  and `iroha.telemetry.export.spans_total` counters; the OTLP collector profile
  ships the same limits as defaults, and on-node configs must not raise them
  without a governance waiver.
- **Evidence:** Alert test vectors and the approved limits are archived under
  `docs/source/nexus_transition_notes.md` alongside the routed-trace audit
  artefacts. B2 acceptance now treats the export budget as closed.

# Operational Workflow

1. **Weekly triage.** Owners report progress in the Nexus readiness call;
   blockers and alert-test artefacts are logged in `status.md`.
2. **Alert dry-runs.** Each alert rule ships alongside a `dashboards/alerts/tests/*.test.yml`
   entry so CI executes `promtool test rules` whenever the guardrail changes.
3. **Audit evidence.** During `TRACE-LANE-ROUTING` and `TRACE-TELEMETRY-BRIDGE`
   rehearsals the on-call captures the Prometheus query results, alert history,
   and the relevant script outputs (`scripts/telemetry/check_nexus_audit_outcome.py`,
   `scripts/telemetry/check_redaction_status.py` for correlated signals) and
   stores them with the routed-trace artefacts.
4. **Escalation.** If any guardrail fires outside a rehearsed window, the
   owning team files a Nexus incident ticket referencing this plan, including
   the metric snapshot and mitigation steps before resuming audits.

With this matrix published and referenced from both `roadmap.md` and
`status.md`, roadmap item **B2** now meets the “responsibility, deadline,
alert, verification” acceptance criteria.
