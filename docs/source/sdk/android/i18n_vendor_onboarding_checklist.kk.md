---
lang: kk
direction: ltr
source: docs/source/sdk/android/i18n_vendor_onboarding_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a4ae9208f26a25c6c6cee934ad5f3fcf7f4166b0431bf8e63ae1436f8e5e373
source_last_modified: "2025-12-29T18:16:36.044128+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK Localization Vendor Onboarding & QA Checklist (AND5)

**Prepared:** 2026-04-08  
**Owners:** Docs/DevRel Manager (Aiko Tanaka), Android DX TL (Sachiko Watanabe),
Kotodama Language Partners PM (Yuki Sato)

This checklist captures the artefacts promised in the AND5 staffing plan for
bringing the Japanese localization vendor online and pairing them with the
Docs/DevRel + Android DX review rota. It serves as the evidence bundle for the
staffing milestone and should be refreshed whenever onboarding assumptions
change.

## 1. Onboarding Tasks

| Task | Owner | Due | Status | Evidence |
|------|-------|-----|--------|----------|
| Execute PO `DOCS-L10N-4901` & NDA, create vendor billing profile | Docs/DevRel Manager | 2026-04-04 | ✅ Completed | `docs/source/docs_devrel/minutes/2026-03.md` (Procurement section) |
| Kickoff call: review AND5 scope, glossary, screenshot workflow | Android DX TL + Vendor PM | 2026-04-08 | ✅ Completed | Video recording + minutes (`artifacts/android/i18n/vendor_kickoff_20260408.md`) |
| Share glossary + style guide (`docs/source/sdk/android/i18n_plan.md` Section 2) | Android DX TL | 2026-04-08 | ✅ Completed | Attached in kickoff minutes |
| Walkthrough of tooling commands (`scripts/sync_docs_i18n.py`, `ci/check_android_docs_i18n.sh`) | Android DX TL (demo) | 2026-04-08 | ✅ Completed | Terminal capture `artifacts/android/i18n/tooling_demo_20260408.log` |
| Screenshot capture SOP review (quickstart/operator/retail samples) | Docs/DevRel Manager | 2026-04-09 | ✅ Completed | Updated Figma links stored in `docs/assets/android/samples/README.md` |
| QA shadow assignment (Thu session) + reviewer rota confirmation | Docs/DevRel Manager + Vendor PM | 2026-04-09 | ✅ Completed | Rota documented in `docs/source/sdk/android/i18n_plan.md` Section 4.3 |
| First delivery pilot: Android quickstart refresh | Vendor PM + Android DX TL | 2026-04-12 | 🕒 In Flight | Request `docs/source/sdk/android/i18n_requests/quickstart_20260408.md`, nightly `ci/check_android_docs_i18n.sh` output archived |

## 2. QA Shadow Playbook

1. **Thursday (vendor QA shadow)** – Vendor joins the Android DX engineer to
   review glossary diffs, screenshot updates, and sample manifests. Meeting
   notes recorded under `docs/source/sdk/android/i18n_requests/minutes/`.
2. **Friday (Docs/DevRel review block)** – Shira Kaplan reviews JP delivery,
   opens HE translation PR (or reviewer notes), updates `docs/i18n/manifest.json`,
   and records the CI job URL.
3. **Backup/contingency** – If Shira is OOO, Eitan Levi assumes the Friday block
   and updates the rota log. Any missed SLA triggers the Saturday contingency
   slot (Android DX on-call) to prep the next Tuesday digest.

## 3. Evidence Bundle Requirements

- Attach kickoff minutes, tooling demo log, and screenshot checklist to
  `artifacts/android/i18n/onboarding_20260408/`.
- Store each request’s CI output using:
  ```bash
  ci/check_android_docs_i18n.sh \
    --json-out artifacts/android/i18n/status-$(date -u +%Y%m%dT%H%M%SZ).json
  ```
- For every translation delivery, run:
  ```bash
  scripts/sync_docs_i18n.py \
    --locale ja --locale he \
    --status-out docs/source/sdk/android/i18n_requests/<name>.md
  ```
  and archive the generated status file inside the same request directory.
- Update `status.md` with a weekly bullet summarizing delivered locales,
  outstanding requests, and any escalations.

## 4. Contacts

| Role | Primary | Backup |
|------|---------|--------|
| Docs/DevRel contact | Aiko Tanaka (`docs-devrel@iroha.tech`) | Shira Kaplan |
| Android DX contact | Sachiko Watanabe (`android-dx@iroha.tech`) | Kenji Ito |
| Vendor PM | Yuki Sato (`kotodama-l10n@vendor.example`) | Haruka Tsuji |

Escalations route through the Docs/DevRel weekly sync (see
`docs/source/docs_devrel/monthly_sync_agenda.md`). Update this checklist when
contacts or tooling change so governance reviews always point at current data.
