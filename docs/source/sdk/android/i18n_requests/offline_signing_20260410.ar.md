---
lang: ar
direction: rtl
source: docs/source/sdk/android/i18n_requests/offline_signing_20260410.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f621c87ae477969d261e9c7afe75d9dd24ffca6c35c41016051815d6cad8ba3d
source_last_modified: "2026-01-03T18:08:01.003774+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Localization Request — Android Offline Signing Guide (2026-04-10)

**Source doc:** `docs/source/sdk/android/offline_signing.md`  
**Locales required:** Japanese (`offline_signing.ja.md`), Hebrew (`offline_signing.he.md`)  
**Owner:** Android DX TL (Sachiko Watanabe) + Docs/DevRel Manager  
**Due dates:** JP delivery by 2026-04-16 (synchronized with AND5 Sprint 17);
HE review window 2026-04-23 (after reviewer rota slot).  
**Roadmap linkage:** AND5 / OA program (offline allowances)

## Scope Summary

- Translate the refreshed offline-signing walkthrough (envelope creation,
  StrongBox alias lifecycle, Norito manifest signing).
- Include the new troubleshooting + telemetry sections plus the OA audit
  checklist callouts.
- Update localized manifests/screenshots found under
  `docs/assets/android/offline_signing/20260410/`; each carries callouts that
  must be mirrored in JA/HE.
- Keep the JSON/YAML snippets in English but translate inline comments.

## Deliverables

1. Updated `docs/source/sdk/android/offline_signing.ja.md` and `.he.md`.
2. Manifest entries in `docs/i18n/manifest.json`.
3. Evidence attachments: OA checklist references, screenshot manifest output
   from `scripts/check_android_samples.sh --samples offline_signing`.
4. Status note referencing this request once translations merge.

## Evidence & Tooling

- Localization status / diff snippet generation:
  ```bash
  scripts/sync_docs_i18n.py \
    --locale ja --locale he \
    --status-out docs/source/sdk/android/i18n_requests/offline_signing_20260410.md
  ```
- CI freshness capture:
  ```bash
  ci/check_android_docs_i18n.sh \
    --json-out artifacts/android/i18n/status-$(date -u +%Y%m%dT%H%M%SZ).json
  ```
- OA checklist references live in
  `docs/source/sdk/android/offline_signing.md#oa-checklist`; ensure translations
  keep anchors so cross-links from status.md continue working.

## Notes

- HE reviewer should reuse the StrongBox glossary from `key_management.md`; any
  alias terminology drift must be logged in the Docs sync notes.
- If OA specs change mid-translation, reopen this request with the updated diff
  summary before accepting new drops.
