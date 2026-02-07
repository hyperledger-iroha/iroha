---
lang: my
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-12-29T18:16:35.080069+00:00"
translation_last_reviewed: 2026-02-07
---

# SNS Training Workbook Template

Use this workbook as the canonical handout for each training cohort. Replace
placeholders (`<...>`) before distributing to attendees.

## Session details
- Suffix: `<.sora | .nexus | .dao>`
- Cycle: `<YYYY-MM>`
- Language: `<ar/es/fr/ja/pt/ru/ur>`
- Facilitator: `<name>`

## Lab 1 — KPI export
1. Open the portal KPI dashboard (`docs/portal/docs/sns/kpi-dashboard.md`).
2. Filter by suffix `<suffix>` and time range `<window>`.
3. Export PDF + CSV snapshots.
4. Record SHA-256 of the exported JSON/PDF here: `______________________`.

## Lab 2 — Manifest drill
1. Fetch the sample manifest from `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json`.
2. Validate with `cargo run --bin sns_manifest_check -- --input <file>`.
3. Generate resolver skeleton with `scripts/sns_zonefile_skeleton.py`.
4. Paste the diff summary:
   ```
   <git diff output>
   ```

## Lab 3 — Dispute simulation
1. Use guardian CLI to start a freeze (case id `<case-id>`).
2. Record the dispute hash: `______________________`.
3. Upload the evidence log to `artifacts/sns/training/<suffix>/<cycle>/logs/`.

## Lab 4 — Annex automation
1. Export the Grafana dashboard JSON and copy it into `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`.
2. Run:
   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. Paste the annex path + SHA-256 output: `________________________________`.

## Feedback notes
- What was unclear?
- Which labs ran over time?
- Tooling bugs observed?

Return completed workbooks to the facilitator; they belong under
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.
