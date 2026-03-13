---
lang: hy
direction: ltr
source: docs/source/sdk/android/i18n_requests/quickstart_20260408.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 38b85bcfb9787961d4087cc4870682c1a150f0078d56c05a9941a6f8156dea00
source_last_modified: "2025-12-29T18:16:36.043202+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Localization Request — Android Quickstart Refresh (2026-04-08)

**Source doc:** `docs/source/sdk/android/index.md`  
**Locales required:** Japanese (`index.ja.md`), Hebrew (`index.he.md`)  
**Owner:** Android DX TL (content) + Docs/DevRel Manager (staffing)  
**Due dates:** JP by 2026-04-12; HE review window 2026-04-19 (per rota)  
**Roadmap linkage:** AND5 — Developer Experience & Samples

## Scope Summary

- Translate the refreshed quickstart sections covering the Norito sample
  walkthrough, `/v2/pipeline` submission flow, and telemetry toggle explainer.
- Update localized screenshot captions + callouts; English sources live under
  `docs/assets/android/samples/quickstart/`. Annotated PNG exports are attached
  to the Docs tracker issue `DOCS-L10N-4930`.
- Keep terminology aligned with `docs/source/sdk/android/key_management.md`
  (StrongBox alias lifecycle), `docs/source/sdk/android/offline_signing.md`
  (envelope/hand-off), and the telemetry runbook (`android_runbook.md`).
- Confirm that the localized files retain the metadata block:
  `source_hash`, `source_last_modified`, and
  `translation_last_reviewed` must match the English source after review.

## Deliverables

1. Updated `docs/source/sdk/android/index.ja.md` with AND5 content.
2. Updated `docs/source/sdk/android/index.he.md` mirroring the JP changes.
3. `docs/i18n/manifest.json` entries refreshed with the new hashes/timestamps.
4. `metadata.localized_screenshots` entries added to the quickstart
   `sample_manifest.json` (handled automatically by `scripts/check_android_samples.sh`).
5. Status update in `status.md` referencing this request and the CI job that
   validated freshness.

## Evidence & Tooling

- Run `scripts/sync_docs_i18n.py --locale ja --locale he --status-out docs/source/sdk/android/i18n_requests/quickstart_20260408.md`
  before submitting the translation PR so the manifest stays in sync.
- Attach the CI artefact from `ci/check_android_docs_i18n.sh --json-out artifacts/android/i18n/status-$(date -u +%Y%m%dT%H%M%SZ).json`
  to the Docs tracker ticket.
- Reference the kickoff notes in `docs/source/sdk/android/i18n_plan.md`
  (Section 4) when logging delivery/approval.

## Notes

- JP vendor will deliver via `android-dx/l10n` branch; use squash merge with
  translator attribution (`Co-authored-by` trailer).
- HE reviewer relies on the Tuesday diff digest plus Thursday QA shadow to
  verify terminology; raise blockers in the Docs/DevRel weekly sync.
- If screenshots change after this request, regenerate the PNG bundle and
  update this file with the new hash/timestamp before re-running CI.
