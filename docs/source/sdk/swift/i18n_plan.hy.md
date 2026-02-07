---
lang: hy
direction: ltr
source: docs/source/sdk/swift/i18n_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 905e58c3c93cb2b4c7566d704b7ff3abf5f023952a02c42f2922a31b9f21076a
source_last_modified: "2025-12-29T18:16:36.071329+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
title: Swift SDK Localization & I18N Plan (IOS5)
summary: Scope, staffing, and SLA for translating Swift SDK docs and samples alongside Android.
---

# Swift SDK Localization & I18N Plan (IOS5)

**Status:** Live (aligned with Docs/DevRel April sync)  
**Scope:** Swift SDK developer docs and sample walkthroughs  
**Owners:** Docs/DevRel Manager (staffing), Swift PM/Lead (source updates), Localization vendor/contractors (execution), QA Guild (linguistic review)

## Objectives

1. Localize high-traffic Swift SDK docs in Japanese and Hebrew within five
   business days of English updates.
2. Keep Swift + Android localization requests on the same vendor cadence so
   terminology and screenshots stay consistent across SDKs.
3. Enforce deterministic freshness by hashing the English sources and storing
   per-locale metadata alongside the translations; merges fail when locales
   drift past the SLA.

## Scope of Work

| Artifact | English Path | JP Target | HE Target | Status/Notes |
|----------|--------------|-----------|-----------|--------------|
| Quickstart | `docs/source/sdk/swift/index.md` | `index.ja.md` | `index.he.md` | In scope for IOS5 beta refresh; reuse Android glossary. |
| Pipeline adoption | `docs/source/sdk/swift/pipeline_adoption_guide.md` | `pipeline_adoption_guide.ja.md` | `pipeline_adoption_guide.he.md` | Translate after Torii staging doc lock; include retry/idempotency terms. |
| Fixture cadence brief | `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` | `ios2_fixture_cadence_brief.ja.md` | `ios2_fixture_cadence_brief.he.md` | Share cadence table with Android fixture brief to keep terminology aligned. |
| Telemetry redaction | `docs/source/sdk/swift/telemetry_redaction.md` | `telemetry_redaction.ja.md` | `telemetry_redaction.he.md` | Add OTLP attribute glossary; align with AND7 redaction plan. |
| Support playbook | `docs/source/sdk/swift/support_playbook.md` | `support_playbook.ja.md` | `support_playbook.he.md` | Include SLA appendix once IOS8 support policy lands. |

Translations should land in locale-suffixed files next to the English source.
When a translated file is present, populate the `source_hash` and
`translation_last_reviewed` metadata as in the Android plan.

## Workflow & SLA

1. **Request:** Swift PM opens a localization request using the Docs tracker
   template (diff summary, commit hash, locales). Link to this plan in the
   request.
2. **Execution:** Vendor/contractors translate using the Android glossary and
   screenshot bundle; QA Guild reviews within two business days.
3. **Publishing:** translators run `scripts/sync_docs_i18n.py --locale <lang>`
   to stamp hashes/dates, then land the translated files with the English
   change.
4. **Enforcement:** `ci/check_swift_docs.sh` validates i18n metadata; merges fail
   when a required locale is missing or stale.

## Staffing & Coordination

- **Vendor:** shares the same JP contractor pool as Android (PO `DOCS-L10N-4901`
  coverage confirmed for Swift). The HE reviewer rota mirrors the Android plan
  (Shira Kaplan primary, Eitan Levi backup).
- **Cadence review:** added to the monthly Docs/DevRel sync agenda; decisions
  and escalations must be logged in the minutes and summarized in `status.md`.
- **Evidence:** store request files and CI status snapshots under
  `docs/source/sdk/swift/readiness/archive/i18n/` for auditability.

## Reporting

- Weekly `status.md` bullet summarising delivered locales and outstanding gaps.
- CI freshness snapshots attached to release readiness bundles.
- Escalations (missed SLA or staffing risk) must be recorded in the Docs
  monthly minutes and reflected in the Swift roadmap/status entries.
