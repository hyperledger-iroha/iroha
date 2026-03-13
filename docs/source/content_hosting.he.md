---
lang: he
direction: rtl
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T17:57:58.226177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Content Hosting Lane
% Core Iroha

# נתיב אירוח תוכן

נתיב התוכן מאחסן חבילות סטטיות קטנות (ארכיון זפת) ברשת ומשרת
קבצים בודדים ישירות מ-Torii.

- **פרסום**: שלח `PublishContentBundle` עם ארכיון tar, תפוגה אופציונלית
  גובה, ומניפסט אופציונלי. מזהה החבילה הוא ה-hash blake2b של
  tarball. ערכי Tar חייבים להיות קבצים רגילים; שמות הם נתיבי UTF-8 מנורמלים.
  מכסי גודל/נתיב/ספירת קבצים מגיעים מתצורת `content` (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`).
  המניפסטים כוללים את ה-hash של אינדקס Norito, מרחב נתונים/נתיב, מדיניות מטמון
  (`max_age_seconds`, `immutable`), מצב אישור (`public` / `role:<role>` /
  `sponsor:<uaid>`), מציין מיקום של מדיניות שמירה ועקיפות MIME.
- **הסרת כפיות**: מטענים זפת מחולקים (ברירת מחדל 64KiB) ומאוחסנים פעם אחת לכל
  hash עם ספירות הפניות; לפרוש צרור ירידה וגזם חתיכות.
- **הגשה**: Torii חושף את `GET /v2/content/{bundle}/{path}`. זרם התגובות
  ישירות מחנות הנתחים עם `ETag` = hash של קובץ, `Accept-Ranges: bytes`,
  תמיכה בטווח, ו-Cache-Control שנגזר מהמניפסט. קורא כבוד את
  מצב אישור מניפסט: תגובות מוגנות תפקידים ותגובות חסות דורשות קנוניות
  כותרות בקשה (`X-Iroha-Account`, `X-Iroha-Signature`) עבור החתומים
  חשבון; חבילות חסרות/פג תוקפן מחזירות 404.
- **CLI**: `iroha content publish --bundle <path.tar>` (או `--root <dir>`) עכשיו
  מייצר אוטומטית מניפסט, פולט `--manifest-out/--bundle-out` אופציונלי, ו
  מקבל `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  ועקיפות `--expires-at-height`. `iroha content pack --root <dir>` בונה
  tarball + מניפסט דטרמיניסטי מבלי להגיש דבר.
- **תצורה**: כפתורי מטמון/אישור חיים תחת `content.*` ב-`iroha_config`
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) והם נאכפים בזמן הפרסום.
- **SLO + מגבלות**: `content.max_requests_per_second` / `request_burst` ו
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` פקק בצד הקריאה
  תפוקה; Torii אוכפת הן לפני הגשת בתים והן לפני ייצוא
  `torii_content_requests_total`, `torii_content_request_duration_seconds`, ו
  מדדי `torii_content_response_bytes_total` עם תוויות תוצאה. חביון
  יעדים חיים תחת `content.target_p50_latency_ms` /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **בקרות ניצול לרעה**: דלי התעריפים מקודדים על ידי אסימון UAID/API/IP מרוחק, ו-
  מגן PoW אופציונלי (`content.pow_difficulty_bits`, `content.pow_header`) יכול
  להידרש לפני הקריאה. ברירת המחדל של פריסת פסים של DA מגיעה
  `content.stripe_layout` והם מהדהדים בקבלות / חשישי מניפסט.
- **קבלות והוכחות לתביעה**: מצורפות תגובות מוצלחות
  `sora-content-receipt` (בסיס 64 Norito ממוסגר `ContentDaReceipt` בייטים) נושאת
  `bundle_id`, `path`, `file_hash`, `served_bytes`, טווח הבייטים המוגש,
  `chunk_root` / `stripe_layout`, התחייבות PDP אופציונלית וחותמת זמן כך
  לקוחות יכולים להצמיד את מה שנלקח מבלי לקרוא מחדש את הגוף.

הפניות עיקריות:- דגם נתונים: `crates/iroha_data_model/src/content.rs`
- ביצוע: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- מטפל Torii: `crates/iroha_torii/src/content.rs`
- עוזר CLI: `crates/iroha_cli/src/content.rs`