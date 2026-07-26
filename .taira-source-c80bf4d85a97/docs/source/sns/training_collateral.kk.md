---
lang: kk
direction: ltr
source: docs/source/sns/training_collateral.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2971b30c1bbdbd9f060cd6168ca9f9617696f02a3c48cda5ecf725ae86ca1ad5
source_last_modified: "2025-12-29T18:16:36.101863+00:00"
translation_last_reviewed: 2026-02-07
title: SNS Training Collateral (SN-8)
summary: Instructor scripts, localization hooks, and annex evidence capture for the SNS suffix program.
---

# Sora Name Service Training Collateral

**Roadmap reference:** SN-8 “Metrics & Onboarding” (see `roadmap.md:432`).  
**Audience:** registrar operators, DNS/gateway engineers, guardians, and finance reviewers preparing `.sora`, `.nexus`, or `.dao` launches.  
**Artifacts:** `dashboards/grafana/sns_suffix_analytics.json`, `docs/portal/docs/sns/kpi-dashboard.md`, `docs/source/sns/onboarding_kit.md`, `docs/examples/sorafs_direct_mode_policy.json`.

This guide consolidates the training curriculum, localization workflow, and
evidence capture steps that governance expects before approving a suffix launch.
It complements the KPI dashboard and onboarding kit by describing how to run the
briefings, which labs to exercise, and how to thread the outputs into the
regulatory annex automation.

## 1. Curriculum overview

### 1.1 Audience tracks

| Track | Objectives | Required pre-reads |
|-------|------------|--------------------|
| Registrar operations | Encode/submit manifests, monitor SLA dashboards, escalate errors. | `docs/source/sns/onboarding_kit.md`, `docs/portal/docs/sns/kpi-dashboard.md`. |
| DNS & gateway | Apply resolver skeletons, propagate freezes, and rehearse rollback. | `docs/source/sorafs_gateway_dns_owner_runbook.md`, `docs/examples/sorafs_gateway_direct_mode.toml`. |
| Guardians & council delegates | Review governance addenda, dispute tooling, and KPI annex evidence. | `docs/source/sns/governance_playbook.md`, `docs/source/sns/reports/steward_scorecard_2026q1.md`. |
| Finance & analytics | Reconcile ARPU/bulk-release metrics and publish annex snapshots. | `docs/portal/docs/finance/settlement-iso-mapping.md`, `dashboards/grafana/sns_suffix_analytics.json`. |

### 1.2 Module sequence

| Module | Duration | Inputs | Exercises | Exit criteria |
|--------|----------|--------|-----------|---------------|
| M1 — KPI orientation | 30 min | KPI dashboard JSON + portal embed. | Walk through suffix filters, export PDF/CSV snapshots. | Trainees can locate registrar throughput, freezes, and ARPU deltas. |
| M2 — Manifest lifecycle | 45 min | `docs/source/sns/registry_schema.md`, zonefile helper JSON. | Build one registrar manifest per language and validate with `scripts/sns_zonefile_skeleton.py`. | Validated manifest + resolver skeleton committed to training branch. |
| M3 — Incident & dispute drills | 40 min | `docs/source/sns/governance_playbook.md`, guardian CLI. | Simulate freeze + dispute appeals, capture audit log excerpt. | Signed dispute transcript stored under `artifacts/sns/training/<suffix>/<cycle>/`. |
| M4 — KPI annex capture | 25 min | Grafana export, annex helper. | Run `cargo xtask sns-annex` with training cycle, update regulatory memo + portal copy. | Annex Markdown + regulatory memo reflect latest digest. |

### 1.3 Lab prerequisites

1. Import `dashboards/grafana/sns_suffix_analytics.json` into the staging Grafana
   instance and verify the portal mirror (`docs/portal/docs/sns/kpi-dashboard.md`)
   shows the same UID/tags.
2. Pre-populate `artifacts/sns/training/<suffix>/<cycle>/` with:
   - `manifests/` — anonymized manifests for lab submissions.
   - `logs/` — Torii + guardian telemetry samples.
   - `slides/` — per-language deck (PDF and editable source).
3. Ensure Torii staging exposes the SNS APIs, and provide trainees with
   temporary credentials scoped to the training namespaces.

## 2. Localization workflow

### 2.1 Languages & collateral

Training handouts must exist for Arabic, Spanish, French, Japanese, Portuguese,
Russian, and Urdu audiences. Each translation lives next to this file as
`training_collateral.<lang>.md` so Git history captures review dates. When you
update the English source, run `python3 scripts/sync_docs_i18n.py --lang <code>`
or manually refresh the stub so translators see the new hash.

| Language | Classroom assets | Contact |
|----------|------------------|---------|
| Arabic (`ar`) | `artifacts/sns/training/.sora/ar/slides/`, localized KPI screenshots, annex template. | `suffix-onboarding-ar@sora.org` |
| Spanish (`es`) | `artifacts/sns/training/.nexus/es/workbooks/`, `docs/examples/sns_training_eval_template.md`. | `nexus-regops@sora.org` |
| French (`fr`) | Shared deck in `artifacts/sns/training/.dao/fr/slides/`, interpreter notes. | `steward-fra@sora.org` |
| Japanese (`ja`) | Interpreter-led workshop; use `docs/source/sns/onboarding_kit.ja.md` + localized CLI output. | `suffix-jp@sora.org` |
| Portuguese (`pt`) | Finance breakout referencing `docs/portal/docs/finance/settlement-iso-mapping.md`. | `suffix-pt@sora.org` |
| Russian (`ru`) | Additional dispute examples stored under `artifacts/sns/training/.sora/ru/logs/`. | `guardian-ru@sora.org` |
| Urdu (`ur`) | Remote delivery kit + translated facilitator script under `artifacts/sns/training/.dao/ur/`. | `suffix-ur@sora.org` |

### 2.2 Delivery checklist

1. Update the language-specific stub (set `status: complete`, note the
   `translation_last_reviewed` timestamp).
2. Export the localized slide deck to PDF and drop it in
   `artifacts/sns/training/<suffix>/<lang>/<cycle>/slides/`.
3. Record a short walkthrough (≤10 min) that demonstrates KPI navigation in the
   target language; link it from the stub.
4. File the assets in the governance tracker with the `sns-training` label so
   reviewers can diff collateral between cycles.

## 3. Delivery assets

### 3.1 Slide deck & workbook

- Deck template: `docs/examples/sns_training_template.md` (export to PDF per
  language before delivery).
- Workbook template (`docs/examples/sns_training_workbook.md`) links directly to
  the KPI dashboard, registrar API docs, and dispute tooling so attendees never
  leave the curated set of references.
- Pre-session email template lives under
  `docs/examples/sns_training_invite_email.md`.

### 3.2 Labs & evaluations

- Lab 1 (KPI export): export registrar throughput, attach digest, and record
  metrics in the shared spreadsheet.
- Lab 2 (Manifest drill): run `scripts/sns_zonefile_skeleton.py` with the
  language-specific descriptor and capture `git diff` for the report.
- Lab 3 (Dispute run): use guardian CLI to stage a freeze and capture the
  resulting entries for the dispute annex.
- Evaluations: `docs/examples/sns_training_eval_template.md` contains the
  survey delivered at the end of every session; drop completed forms in
  `artifacts/sns/training/<suffix>/<cycle>/feedback/`.

## 4. KPI review & annex handoff

After the final lab, capture evidence so governance can reference the exact KPI
state that trainees used:

1. Export the Grafana dashboard (`sns_suffix_analytics.json`) for the current
   cycle and copy it to
   `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`.
2. Run `cargo xtask sns-annex --suffix <suffix> --cycle <cycle> --dashboard <export> \
   --dashboard-artifact <artifacts path> --output docs/source/sns/reports/<suffix>/<cycle>.md \
   --regulatory-entry docs/source/sns/regulatory/<memo>.md \
   --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md`.
   The `--portal-entry` flag keeps the portal memo in sync with the source annex.
3. Attach the workbook answers, slide deck, and recording links to the same
   governance ticket so reviewers see the whole bundle.

## 5. Scheduling & feedback loops

| Cycle | Training window | Feedback channels | Metrics |
|-------|-----------------|-------------------|---------|
| 2026‑03 | First week after KPI review | `#sns-training` Matrix room, annex comments. | Attendance %, satisfaction score ≥4/5, annex export digest logged. |
| 2026‑06 | Prior to `.dao` GA | Survey link + finance office hours. | Finance readiness score, manifest drill pass rate. |
| 2026‑09 | Post multi-suffix expansion | Guardian Q&A, portal feedback form. | Dispute drill completion time, annex SLA (≤2 days). |

Capture anonymous feedback in `docs/source/sns/reports/sns_training_feedback.md`
so the next cohort can iterate on the exercises and localized scripts.
