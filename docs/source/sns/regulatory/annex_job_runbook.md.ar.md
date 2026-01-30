---
lang: ar
direction: rtl
source: docs/source/sns/regulatory/annex_job_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2c064150d40663b89e4dd8630921247f359ea4acb85f599557a9f127bf68ee5a
source_last_modified: "2026-01-03T18:07:57.626407+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SNS KPI Annex Job Runbook
summary: Checklist for maintaining the annex job schedule referenced by roadmap item SN-8.
---

# SNS KPI Annex Job Runbook (SN-8)

Roadmap task **SN-8 – Metrics & Onboarding** requires every suffix to ship a
monthly KPI annex with signed dashboard evidence. This runbook explains how to
add a new cycle to `docs/source/sns/regulatory/annex_jobs.json`, how to execute
`scripts/run_sns_annex_jobs.py`, and which artefacts must be captured for
governance sign-off.

## 1. When to add a job

1. Confirm the upcoming review window with Governance (default cadence: monthly
   per suffix).
2. Use `scripts/add_sns_annex_cycle.py <cycle>` to append entries to
   `docs/source/sns/regulatory/annex_jobs.json`, create the annex/memo
   placeholders, and seed the localization stubs. The helper defaults to all
   three suffixes (`.sora`, `.nexus`, `.dao`) but accepts `--suffix` overrides
   when you need to stage a subset.
3. If you prefer a manual edit, append entries to
   `docs/source/sns/regulatory/annex_jobs.json` with the same fields described
   above (`suffix`, `cycle`, `jurisdiction`, optional overrides) and create the
   EU DSA memo skeleton (`docs/source/sns/regulatory/eu-dsa/<cycle>.md`) with
   placeholder annex markers (`<!-- sns-annex:<suffix>-<cycle>:start -->`). The
   batch helper updates the placeholders automatically once the dashboard
   export is ready.

## 2. Running the batch helper

The helper iterates every defined job, copies the dashboard export into the
artefact tree, and rewrites the annex blocks in the regulatory memo (and portal
mirror when it exists). Use verbose mode to capture the commands in the change
ticket:

```bash
python3 scripts/run_sns_annex_jobs.py \
  --jobs docs/source/sns/regulatory/annex_jobs.json \
  --verbose
```

- By default the helper invokes `cargo xtask sns-annex` using the dashboard
  path recorded in the job (or the toolkit default). Pass `--runner` to point
  at a different command when testing.
- Use `--dry-run` when you only need to verify the planned commands or confirm
  that the regulatory/portal entries exist.
- Use `--check-only` to validate previously recorded annex blocks without
  regenerating the dashboard export—handy for CI jobs that lint documentation.

## 3. Evidence bundle

Each run should yield:

| Artefact | Path |
|----------|------|
| Dashboard export | `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json` |
| Annex markdown | `docs/source/sns/reports/<suffix>/<cycle>.md` |
| Regulatory memo block | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` |
| Portal memo block (when present) | `docs/portal/docs/sns/regulatory/<jurisdiction>-<cycle>.md` |

Before merging:

1. Attach the generated artefacts (`artifacts/sns/regulatory/...`) to the
   governance ticket.
2. Update `docs/source/sns/onboarding_kit.md` (Section 1.3) if the cadence or
   annex scope changes.
3. Record the run in `status.md` when the annex closes a roadmap checkpoint.

## 4. Failure handling

- Missing dashboard export ⇒ regenerate `dashboards/grafana/sns_suffix_analytics.json`
  or provide an explicit `dashboard` path in the job entry.
- Missing regulatory memo ⇒ create the file (plus localization stubs) before
  rerunning the helper.
- Digest mismatch ⇒ rerun the Grafana export and delete the stale artefact to
  avoid referencing an inconsistent snapshot.

## 5. Current schedule snapshot

The tracking JSON now includes the April **and** May 2026 KPI cycles for
`.sora`, `.nexus`, and `.dao`. Run this checklist whenever a new suffix/cycle
pair is added so SN-8 evidence stays reproducible and localized copies remain
in sync with the latest intake memos.
