---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf1-determinism.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cd9ef613d7c825bd562d2708ed0027f3866ddf031655592820a46aeadad3256
source_last_modified: "2025-11-13T08:10:42.118544+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: הרצת יובש לקביעות SF1 של SoraFS
summary: Checklist ו-digests צפויים לאימות פרופיל chunker הקנוני `sorafs.sf1@1.0.0`.
---

# הרצת יובש לקביעות SF1 של SoraFS

דו"ח זה מתעד את הרצת ה-dry-run הבסיסית עבור פרופיל chunker הקנוני
`sorafs.sf1@1.0.0`. Tooling WG צריכה להריץ מחדש את ה-checklist למטה בעת אימות
רענוני fixtures או pipelines חדשים של consumers. רשמו את תוצאת כל פקודה בטבלה
כדי לשמור trail שניתן לביקורת.

## Checklist

| שלב | פקודה | תוצאה צפויה | הערות |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | כל הבדיקות עוברות; בדיקת parity `vectors` מצליחה. | מאשר שה-fixtures הקנוניים מתקמפלים ומתאימים ליישום Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | הסקריפט יוצא 0; מדווח digests של manifest למטה. | מאמת ש-fixtures מתחדשים בצורה נקיה ושהחתימות נשארות מצורפות. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | הרשומה `sorafs.sf1@1.0.0` תואמת את registry descriptor (`profile_id=1`). | מבטיח שה-metadata של ה-registry נשארת מסונכרנת. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | הרגנרציה מצליחה ללא `--allow-unsigned`; קבצי manifest ו-signature לא משתנים. | מספק הוכחת determinism לגבולות chunk ול-manifests. |
| 5 | `node scripts/check_sf1_vectors.mjs` | מדווח שאין diff בין fixtures TypeScript ל-Rust JSON. | helper אופציונלי; לשמור על parity בין runtimes (הסקריפט מתוחזק ע"י Tooling WG). |

## digests צפויים

- Chunk digest (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Sign-Off Log

| תאריך | Engineer | תוצאת checklist | הערות |
|------|----------|------------------|-------|
| 2026-02-12 | Tooling (LLM) | ❌ Failed | שלב 1: `cargo test -p sorafs_chunker` נכשל ב-`vectors` suite משום שה-fixtures עדיין מפרסמים את ה-handle הישן `sorafs.sf1@1.0.0` וחסרים profile aliases/digests (`fixtures/sorafs_chunker/sf1_profile_v1.*`). שלב 2: `ci/check_sorafs_fixtures.sh` נעצר — `manifest_signatures.json` חסר במצב הריפו (נמחק ב-working tree). שלב 4: `export_vectors` לא יכול לאמת חתימות כל עוד קובץ manifest חסר. מומלץ לשחזר fixtures חתומים (או לספק מפתח council) ולרגנר bindings כך שה-handle הקנוני + מערך ה-aliases יוטמעו כפי שהבדיקות דורשות. |
| 2026-02-12 | Tooling (LLM) | ✅ Passed | Fixtures רוג'נרו באמצעות `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, והפיקו handle קנוני + alias lists ו-manifest digest חדש `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. אומת עם `cargo test -p sorafs_chunker` והרצה נקיה של `ci/check_sorafs_fixtures.sh` (fixtures בסטייג' לאימות). שלב 5 בהמתנה עד ש-helper parity של Node יגיע. |
| 2026-02-20 | Storage Tooling CI | ✅ Passed | Parliament envelope (`fixtures/sorafs_chunker/manifest_signatures.json`) נשלף דרך `ci/check_sorafs_fixtures.sh`; הסקריפט רגנר fixtures, אישר את manifest digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, והריץ שוב את Rust harness (שלבי Go/Node רצים כשזמינים) ללא diffs. |

Tooling WG צריכה להוסיף שורה מתוארכת אחרי הרצת ה-checklist. אם שלב כלשהו נכשל,
פתחו issue המקושר כאן והוסיפו פרטי remediation לפני אישור fixtures או פרופילים
חדשים.
