---
lang: he
direction: rtl
source: docs/source/project_tracker/sorafs_pin_registry_tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 54f96d65069b15af3c1f48cc22d56efedddcaa098c91df1382007c47a3a6329f
source_last_modified: "2026-01-06T15:14:01.036336+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/project_tracker/sorafs_pin_registry_tracker.md -->

# מעקב חוזה Pin Registry של SoraFS

מעקב זה מתאם את עבודת היישום עבור חוזה Pin Registry של SoraFS לפני SF-4.
הוא יורש את הדרישות שהוגדרו ב‑
[SoraFS Architecture RFC (SF-1)](../sorafs_architecture_rfc.md), כולל זרימת
digest הקנונית של manifest ומעטפות ממשל.

| מזהה | אבן דרך | בעלים | חלון יעד | סטטוס | הערות |
|----|-----------|--------|---------------|--------|-------|
| PR-001 | שלד חוזה (`PinRegistry::register`, `approve`, `retire`) | Storage Team; Nexus Core Infra TL | Q4 2025 | בתהליך | `iroha_data_model` חושף כעת טיפוסי נתונים לרישום pin של SoraFS יחד עם ISI stubs ל‑`RegisterPinManifest`, `ApprovePinManifest`, ו‑`RetirePinManifest`. |
| PR-002 | צנרת חתימות ממשל | Governance Secretariat; Tooling WG | Q1 2026 | בתהליך | Iroha core מאמתת כעת מעטפות מועצה (digest/manifest/profile/signatures) במהלך `ApprovePinManifest`; עבודה נותרת: חשיפה ב‑Torii/CLI ותמיכה רב‑אלגוריתמית. |
| PR-003 | אכיפת alias + מדיניות retention | Storage Team | Q1 2026 | בתהליך | אימות alias + ייחודיות נמצאים כעת ב‑`crates/iroha_core/src/smartcontracts/isi/sorafs.rs`, עם עדכוני DTO ב‑Torii ב‑`crates/iroha_torii/src/routing.rs`; חלונות retention ומספר רפליקות עדיין נדרשים. |
| PR-004 | התאמת CI + fixtures | Tooling WG | Q1 2026 | מתוכנן | להרחיב `ci/check_sorafs_fixtures.sh` ולהוסיף בדיקות golden ממוקדות חוזה. |
| PR-005 | תיעוד rollout ומדריך מפעיל | Docs Team | Q1 2026 | מתוכנן | לפרסם runbook מפעילים שמפנה למעטפות ממשל ותוכנית מיגרציה. |

## הפניות

- [`docs/source/sorafs_architecture_rfc.md`](../sorafs_architecture_rfc.md)
- [`fixtures/sorafs_chunker/manifest_signatures.json`](../../../fixtures/sorafs_chunker/manifest_signatures.json)
- [`ci/check_sorafs_fixtures.sh`](../../../ci/check_sorafs_fixtures.sh)

</div>
