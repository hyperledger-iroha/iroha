---
lang: pt
direction: ltr
source: docs/source/sdk/android/i18n_requests/networking_20260410.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b39b8469bdd3fde7568c3f9253ce43d48c7ac884d71bdf9d60cb8f9b03e3a51
source_last_modified: "2026-01-03T18:08:01.012388+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Localization Request — Android Networking Guide Refresh (2026-04-10)

**Source doc:** `docs/source/sdk/android/networking.md`  
**Locales required:** Japanese (`networking.ja.md`), Hebrew (`networking.he.md`)  
**Owner:** Android Networking TL (Elena Morita) + Docs/DevRel Manager  
**Due dates:** JP by 2026-04-16 (post-NRPC diff digest); HE review window opens 2026-04-26 once Torii `/v2/pipeline` parity is signed off.  
**Roadmap linkage:** AND5 & AND4 (shared `/v2/pipeline` guidance + NRPC readiness)

## Scope Summary

- Translate the updated networking guide sections covering `/v2/pipeline`
  retries, Norito RPC parity, telemetry toggles, and the mock Torii harness.
- Include the new “Telemetry + Chaos Hooks” appendix and ensure the Norito RPC
  glossary stays aligned with `docs/source/torii/nrpc_spec.md`.
- Update localized screenshots/diagrams from
  `docs/assets/android/networking/20260410/`; annotate directional text in the
  translated copy (e.g., callouts inside diagrams).
- Keep CLI snippets in English but translate surrounding context and table
  descriptions.

## Deliverables

1. Updated `docs/source/sdk/android/networking.ja.md` & `networking.he.md`.
2. Refreshed entries in `docs/i18n/manifest.json` with new hashes/timestamps.
3. Screenshot manifest updates recorded by running
   `scripts/check_android_samples.sh --samples networking`.
4. A `status.md` bullet referencing this request and the CI job that enforced
   freshness.

## Evidence & Tooling

- Generate localization status + archive diff snippets for reviewers:
  ```bash
  scripts/sync_docs_i18n.py \
    --locale ja --locale he \
    --status-out docs/source/sdk/android/i18n_requests/networking_20260410.md
  ```
- Capture CI freshness output:
  ```bash
  ci/check_android_docs_i18n.sh \
    --json-out artifacts/android/i18n/status-$(date -u +%Y%m%dT%H%M%SZ).json
  ```
- Attach Torii mock harness digest + NRPC diff digest to the Docs tracker ticket
  `DOCS-L10N-4931`.

## Notes

- HE translation should wait for the `/v2/pipeline` parity “green” signal from
  Torii (tracked in the AND4 council notes).
- If networking snippets change after translation starts, regenerate the
  screenshot bundle and rerun the commands above so artefacts stay consistent.
