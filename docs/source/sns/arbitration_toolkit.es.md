---
lang: es
direction: ltr
source: docs/source/sns/arbitration_toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b5c935e2e7100bc1b47fc27a879f7a29eb00cebbf3a087bbe6ca8fad983c579d
source_last_modified: "2026-01-03T18:07:57.515957+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
title: SNS Arbitration Toolkit (SN-6a)
summary: Case schema, SLA targets, dashboards, and transparency templates for dispute resolution.
---

# Sora Name Service Arbitration Toolkit (SN-6a)

**Status:** Drafted 2026-04-26 — fulfils the “define dispute case schema, SLAs, and transparency reports” portion of SN-6a.  
**Roadmap links:** SN-6 “Compliance & Dispute Resolution” (roadmap.md), SN-7 resolver/gateway sync, SNS metrics roadmap (SN-8).  
**Artifacts:** JSON schema (`docs/examples/sns/arbitration_case_schema.json`) and transparency template (`docs/examples/sns/arbitration_transparency_report_template.md`).

This toolkit gives governance councils, legal reviewers, and registrar support
teams a deterministic package for SNS arbitration. It specifies the machine-
readable case schema, response SLAs, evidence capture workflow, and the
reporting template required for monthly transparency releases. Implementations
can load the schema to validate CLI submissions, surface SLA breaches in
Grafana, and feed reports directly into the public governance portal.

## 1. Arbitration Case Schema

The canonical schema lives in
`docs/examples/sns/arbitration_case_schema.json`. It follows JSON Schema
Draft 7 and mirrors the registrar/Norito selector data model, so automation can
attach disputes to the same i105 selectors and suffix identifiers used
elsewhere in SNS.

```bash
jq '.properties' docs/examples/sns/arbitration_case_schema.json
```

### 1.1 Required fields

| Field | Type | Description |
|-------|------|-------------|
| `case_id` | string (`SNS-YYYY-NNNNN` or UUID) | Immutable identifier referenced by governance votes and RCA logs. |
| `selector` | object `{suffix_id,label,global_form}` | Targeted name. `global_form` must match the canonical Katakana i105 output documented in `address_display_guidelines.md`. |
| `dispute_type` | enum (`ownership`,`policy_violation`,`abuse`,`billing`,`other`) | Tracks the policy clause under review. |
| `priority` | enum (`urgent`,`high`,`standard`,`info`) | Drives the SLA matrix below and alert routing. |
| `reported_at` | RFC3339 timestamp | Time the complaint entered the queue (pre-notary). |
| `status` | enum (`open`,`triage`,`hearing`,`decision`,`remediation`,`closed`,`suspended`) | Mirrors the workflow in §2. |
| `reporter` | object `{role,contact,reference_ticket}` | Who filed the complaint (registrar, steward, guardian, public). |
| `respondents` | array of `{role,account_id,contact}` | Parties expected to answer the complaint. |
| `allegations` | array of `{code,summary,policy_reference}` | Enumerates the norms allegedly violated. |
| `evidence` | array of `{id,kind,uri,hash,description,sealed}` | Items attached at intake; `sealed=true` keeps artefacts in confidential storage. |
| `sla` | object `{acknowledge_by,resolution_by,extensions[]}` | Deadlines derived from §2 plus any granted extensions. |
| `actions` | timeline array `{timestamp,actor,action,notes}` | Mandatory for every state transition so SLAs and dashboards stay reproducible. |
| `decision` | object `{finding,remedies,effective_at,publication_state}` | Populated once the council renders a verdict, even when the outcome is “dismissed”. |

### 1.2 Optional fields

- `acknowledged_at`, `triage_started_at`, `hearing_scheduled_at`,
  `resolution_issued_at` — recorded automatically by the workflow engine to
  support SLA analytics.
- `respondent_submissions` — documents and statements the respondent provided.
- `guardian_overrides` — when the guardian board freezes a selector mid-case.
- `billing_adjustments` — ledger instructions when the outcome affects rent,
  penalties, or refunds.

## 2. Workflow & SLA Matrix

Arbitration follows the same five-phase lifecycle across all suffixes. The SLA
matrix keeps response times enforceable even as suffix volume grows.

| Priority | Incident Definition | Acknowledge | Hearing scheduled | Decision issued | Owner |
|----------|--------------------|-------------|-------------------|-----------------|-------|
| Urgent | Active abuse / security impact | ≤ 2 h | ≤ 24 h | ≤ 72 h | Guardian on-call + council liaison |
| High | Ownership / policy disputes impacting launches | ≤ 8 h | ≤ 48 h | ≤ 10 d | Council case manager |
| Standard | Routine billing, renewal, or content disputes | ≤ 24 h | ≤ 5 d | ≤ 21 d | Registrar steward |
| Info | FYI / requests for clarification | ≤ 3 d | n/a | ≤ 30 d | Support queue |

Workflow phases:

1. **Intake:** Support CLI (`sns governance case create`) or portal form
   populates the schema, verifies the selector, stores attachments in
   Norito-backed object storage, and enqueues the case with calculated SLA
   deadlines.
2. **Triage:** Case manager validates jurisdiction, confirms payment proofs, and
   either escalates to guardians (urgent) or schedules the hearing window.
3. **Hearing:** Respondents upload statements via `sns governance case file`,
   optional synchronous calls are recorded, and `actions[]` receives the
   transcript hash plus attendance metadata.
4. **Decision:** Council votes recorded as `actions[]` entries referencing the
   governance vote id; verdict populates `decision`.
5. **Remediation & Closure:** Resolver freeze/unfreeze (`sns governance
   freeze/unfreeze`), billing adjustments, and transparency artefacts attached.

Automation must emit `sns_arbitration_sla_breach_total` labels whenever
`acknowledged_at > sla.acknowledge_by` or `resolution_issued_at >
sla.resolution_by` even if extensions exist—extensions just add metadata for
the transparency report.

## 3. Evidence & Attachments

- **Hashing:** Every attachment stores `hash` (SHA-256) in the schema and
  optionally a Norito manifest path or content-addressed CAR file for Taikai.
- **Confidential evidence:** set `sealed=true` for material restricted to
  guardians/legal; the transparency report only publishes hashed pointers.
- **Linked manifests:** For auctions or registrar misconfigurations, copy the
  registrar manifest hash and Torii request id into `actions[]` so auditors can
  pull the payload later.

## 4. Transparency Reports

Governance releases monthly reports per suffix using
`docs/examples/sns/arbitration_transparency_report_template.md`. Each report
captures:

- Case counts by type, severity, and disposition.
- SLA compliance metrics (`sns_arbitration_cases_open_total`,
  `sns_arbitration_sla_breach_total`, `sns_arbitration_cases_closed_total`).
- Summaries of remedial actions (freezes, transfers, refunds).
- Guardian overrides and appeal outcomes.

Populate the template by exporting NDJSON via
`sns governance case export --since=<ISO8601>` and piping through `jq`
aggregations:

```bash
sns governance case export --since 2026-04-01 > artifacts/sns/cases.ndjson
jq -s 'group_by(.dispute_type) | map({type: .[0].dispute_type, count: length})' \
  artifacts/sns/cases.ndjson > artifacts/sns/case_summary.json
```

Attach both the rendered report and raw NDJSON to the transparency ticket.

## 5. CLI Automation

CLI helpers live under `sns governance case ...`:

- `sns governance case create --case-json cases/alpha.json` validates the payload
  against `docs/examples/sns/arbitration_case_schema.json` before POSTing it to
  Torii. Use `--dry-run` to lint cases without submitting, or `--schema` to
  supply a staged schema file during reviews.
- `sns governance case export --since 2026-04-01T00:00:00Z --limit 200` returns
  the NDJSON feed used by dashboards and transparency reports. Filters for
  `--status` and `--since` match the toolkit workflow in §4.

Both commands rely on the validation helpers implemented in
`crates/iroha_cli/src/commands/sns.rs` so automation pipelines, docs portal
scripts, and governance workstations all enforce the exact same structure.

## 6. Dashboard & Telemetry Hooks

The arbitration dashboard (Grafana export pending) must track:

- `sns_arbitration_cases_open_total{suffix_id,priority}` and
  `sns_arbitration_cases_closed_total`.
- `sns_arbitration_sla_breach_total{phase}` broken out by acknowledge vs.
  resolution deadlines.
- `sns_arbitration_hearing_lead_time_seconds` histogram to visualise backlog.
- `sns_arbitration_decision_age_seconds` histogram to ensure resolved cases are
  published quickly.
- `sns_arbitration_guardian_override_total` for emergency freezes or forced
  transfers.

Reference implementation: `dashboards/grafana/sns_arbitration_observability.json`
plus the matching Alertmanager pack (`dashboards/alerts/sns_arbitration_rules.yml`).
The dashboard gates SN-6a readiness by feeding the same queries referenced in
the Grafana descriptions into CI evidence bundles.

Alert rules:

- Urgent cases unacknowledged for >1 h.
- >2 simultaneous urgent/high cases in the same suffix.
- SLA breach ratio >5 % in a trailing 30 d window.

## 6. Implementation Checklist

- [x] Publish schema + docs (this file + JSON schema).
- [x] Publish transparency template.
- [ ] Wire schema validation into `sns governance case create`.
- [ ] Publish Grafana/Alertmanager bundle referenced above.
- [ ] Automate monthly report generation in CI and publish to the portal.

As the remaining automation lands, update this file with dashboard/CLI paths
and mark SN-6a complete in `roadmap.md`.
