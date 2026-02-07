---
id: nexus-telemetry-remediation
lang: kk
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus telemetry remediation plan (B2)
description: Mirror of `docs/source/nexus_telemetry_remediation_plan.md`, documenting the telemetry gap matrix and operational workflow.
---

# Overview

Roadmap item **B2 — telemetry gap ownership** requires a published plan tying
every outstanding Nexus telemetry gap to a signal, alert guardrail, owner,
deadline, and verification artifact before the Q1 2026 audit windows begin.
This page mirrors `docs/source/nexus_telemetry_remediation_plan.md` so release
engineering, telemetry ops, and SDK owners can confirm coverage ahead of the
routed-trace and `TRACE-TELEMETRY-BRIDGE` rehearsals.

# Gap matrix

| Gap ID | Signal & alert guardrail | Owner / escalation | Due (UTC) | Evidence & verification |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Histogram `torii_lane_admission_latency_seconds{lane_id,endpoint}` with alert **`SoranetLaneAdmissionLatencyDegraded`** firing when `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` for 5 minutes (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (signal) + `@telemetry-ops` (alert); escalate via Nexus routed-trace on-call. | 2026‑02‑23 | Alert tests under `dashboards/alerts/tests/soranet_lane_rules.test.yml` plus the `TRACE-LANE-ROUTING` rehearsal capture showing fired/recovered alert and Torii `/metrics` scrape archived in [Nexus transition notes](./nexus-transition-notes). |
| `GAP-TELEM-002` | Counter `nexus_config_diff_total{knob,profile}` with guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` gating deploys (`docs/source/telemetry.md`). | `@nexus-core` (instrumentation) → `@telemetry-ops` (alert); governance duty officer paged when counter increments unexpectedly. | 2026‑02‑26 | Governance dry-run outputs stored next to `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; release checklist includes the Prometheus query screenshot plus log excerpt proving `StateTelemetry::record_nexus_config_diff` emitted the diff. |
| `GAP-TELEM-003` | Event `TelemetryEvent::AuditOutcome` (metric `nexus.audit.outcome`) with alert **`NexusAuditOutcomeFailure`** when failures or missing outcomes persist for >30 minutes (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) escalating to `@sec-observability`. | 2026‑02‑27 | CI gate `scripts/telemetry/check_nexus_audit_outcome.py` archives NDJSON payloads and fails when a TRACE window lacks a success event; alert screenshots attached to the routed-trace report. |
| `GAP-TELEM-004` | Gauge `nexus_lane_configured_total` with guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` feeding the SRE on-call checklist. | `@telemetry-ops` (gauge/export) escalating to `@nexus-core` when nodes report inconsistent catalog sizes. | 2026‑02‑28 | Scheduler telemetry test `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` proves emission; operators attach Prometheus diff + `StateTelemetry::set_nexus_catalogs` log excerpt to the TRACE rehearsal package. |

# Operational workflow

1. **Weekly triage.** Owners report progress in the Nexus readiness call;
   blockers and alert-test artefacts are logged in `status.md`.
2. **Alert dry-runs.** Each alert rule ships alongside a
   `dashboards/alerts/tests/*.test.yml` entry so CI executes `promtool test
   rules` whenever the guardrail changes.
3. **Audit evidence.** During `TRACE-LANE-ROUTING` and
   `TRACE-TELEMETRY-BRIDGE` rehearsals the on-call captures the Prometheus query
   results, alert history, and relevant script outputs
   (`scripts/telemetry/check_nexus_audit_outcome.py`,
   `scripts/telemetry/check_redaction_status.py` for correlated signals) and
   stores them with the routed-trace artefacts.
4. **Escalation.** If any guardrail fires outside a rehearsed window, the owning
   team files a Nexus incident ticket referencing this plan, including the
   metric snapshot and mitigation steps before resuming audits.

With this matrix published — and referenced from both `roadmap.md` and
`status.md` — roadmap item **B2** now meets the “responsibility, deadline,
alert, verification” acceptance criteria.
