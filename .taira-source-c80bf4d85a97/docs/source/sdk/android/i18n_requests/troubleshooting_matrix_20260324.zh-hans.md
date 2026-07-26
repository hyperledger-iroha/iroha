---
lang: zh-hans
direction: ltr
source: docs/source/sdk/android/i18n_requests/troubleshooting_matrix_20260324.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a952b4a46554bcd120819adcb6e8fa321241006b05730f33b5a249a7e857f4e
source_last_modified: "2025-12-29T18:16:36.043666+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Localization Request — Android Troubleshooting Matrix (2026-03-24)

**Source doc:** `docs/source/sdk/android/troubleshooting.md`  
**Locales required:** Japanese (`troubleshooting.ja.md`), Hebrew (`troubleshooting.he.md`)  
**Owner:** Docs/DevRel Manager (Aiko Tanaka)  
**Due dates:** JP by 2026-04-12 (aligned with vendor kickoff); HE review window 2026-04-26 (per rota)  
**Roadmap linkage:** AND5 — Developer Experience & Samples

## Scope Summary

- Translate the entire troubleshooting matrix: severity/SLA tables, scenario
  matrix, tooling reference, evidence checklist, and localization guidance.
- Keep CLI snippets and log extracts in English while translating narrative
  context, table descriptions, and callouts.
- Maintain anchors pointing to `docs/source/android_runbook.md`,
  `docs/source/sdk/android/telemetry_redaction.md`, and labs references.
- Screenshots are not required for this request; focus on table accuracy.

## Deliverables

1. Updated `docs/source/sdk/android/troubleshooting.ja.md` and `.he.md`.
2. Refreshed `docs/i18n/manifest.json` entries (hash + timestamps).
3. Status note referencing this request and CI freshness job.
4. Evidence attachments for the severity table diff and SLA appendix.

## Evidence & Tooling

- Generate localization status files:
  ```bash
  scripts/sync_docs_i18n.py \
    --locale ja --locale he \
    --status-out docs/source/sdk/android/i18n_requests/troubleshooting_matrix_20260324.md
  ```
- Capture CI freshness snapshot:
  ```bash
  ci/check_android_docs_i18n.sh \
    --json-out artifacts/android/i18n/status-$(date -u +%Y%m%dT%H%M%SZ).json
  ```
- Attach the rendered HTML of the severity + scenario tables to the Docs tracker
  ticket (`DOCS-L10N-4920`) for reviewer reference.

## Notes

- Reference glossary: `docs/source/sdk/android/i18n_plan.md` (Section 2).
- Keep “Sev 1/2/3” labels untranslated for consistency with runbooks.
- Include translator/reviewer names in commit message trailers
  (`Co-authored-by`) to satisfy provenance requirements.
