---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-04T10:50:53.604570+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Chunking → Manifest Pipeline

המלווה הזה להתחלה המהירה עוקב אחר הצינור מקצה לקצה שהופך לגולמי
בתים למניפסטים של Norito המתאימים ל-SoraFS Pin Registry. התוכן הוא
מותאם מ-[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
עיין במסמך זה עבור המפרט הקנוני ויומן השינויים.

## 1. נתח באופן דטרמיניסטי

SoraFS משתמש בפרופיל SF-1 (`sorafs.sf1@1.0.0`): גלגול בהשראת FastCDC
hash עם גודל נתח מינימלי של 64KiB, יעד של 256KiB, מקסימום 512KiB ו-
`0x0000ffff` מסכת שבירה. הפרופיל רשום ב
`sorafs_manifest::chunker_registry`.

### עוזרי חלודה

- `sorafs_car::CarBuildPlan::single_file` - פולט קיזוז נתחים, אורכים ו
  BLAKE3 מעכל בזמן הכנת מטא נתונים של CAR.
- `sorafs_car::ChunkStore` - הזרמת עומסים, מחזיקה מטא נתונים של נתחים, ו
  נגזר מעץ הדגימה של 64KiB / 4KiB הוכחת אחזור (PoR).
- `sorafs_chunker::chunk_bytes_with_digests` - עוזר הספרייה מאחורי שני ה-CLI.

### כלי CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

ה-JSON מכיל את הקיזוזים המסודרים, האורכים ותקצירי הנתחים. תמשיכו את
תכנן בעת בניית מניפסטים או מפרטי אחזור מתזמר.

### עדי PoR

`ChunkStore` חושף את `--por-proof=<chunk>:<segment>:<leaf>` ו
`--por-sample=<count>` כך שמבקרים יכולים לבקש ערכות עדים דטרמיניסטיות. זוג
הדגלים האלה עם `--por-proof-out` או `--por-sample-out` כדי להקליט את ה-JSON.

## 2. עטוף מניפסט

`ManifestBuilder` משלב מטא נתונים של נתחים עם קבצים מצורפים לניהול:

- Root CID (dag-cbor) והתחייבויות CAR.
- הוכחות כינוי ותביעות יכולות ספק.
- חתימות מועצה ומטא נתונים אופציונליים (למשל, מזהי build).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

תפוקות חשובות:

- `payload.manifest` – בתים מניפסט מקודדים ב-Norito.
- `payload.report.json` - סיכום קריא אנושי/אוטומציה, כולל
  `chunk_fetch_specs`, `payload_digest_hex`, CAR Digests ומטא נתונים כינויים.
- `payload.manifest_signatures.json` - מעטפה המכילה מניפסט BLAKE3
  תקציר, תקציר SHA3 ב-chunk-plan וחתימות Ed25519 ממוינות.

השתמש ב-`--manifest-signatures-in` כדי לאמת מעטפות שסופקו על ידי חיצוני
החותמים לפני כתיבתם חזרה, ו-`--chunker-profile-id` או
`--chunker-profile=<handle>` כדי לנעול את בחירת הרישום.

## 3. פרסם והצמד

1. **הגשת ממשל** - ספק את תקציר המניפסט ואת החתימה
   מעטפה למועצה כדי לקבל את הסיכה. מבקרים חיצוניים צריכים
   אחסן את ה-chunk-plan SHA3 digest לצד ה-manifest digest.
2. **מטעני סיכה** - העלה את ארכיון CAR (ואינדקס CAR אופציונלי) שאליו מתייחסים
   במניפסט ל-Pin Registry. ודא שהמניפסט ו-CAR חולקים את
   אותו בסיס CID.
3. **הקלט טלמטריה** - הישמר את דוח ה-JSON, עדי PoR וכל אחזור
   מדדים בחפצי שחרור. רשומות אלה מאכילות לוחות מחוונים של מפעילים ו
   לעזור לשחזר בעיות מבלי להוריד מטענים גדולים.

## 4. סימולציית אחזור מרובי ספקים

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` מגדיל את ההקבלה לכל ספק (`#4` למעלה).
- הטיית תזמון של מנגינות `@<weight>`; ברירת המחדל היא 1.
- `--max-peers=<n>` מגביל את מספר הספקים המתוכננים להרצה כאשר
  גילוי מניב יותר מועמדים מהרצוי.
- `--expect-payload-digest` ו-`--expect-payload-len` שומרים מפני שקט
  שחיתות.
- `--provider-advert=name=advert.to` מאמת את יכולות הספק לפני כן
  שימוש בהם בסימולציה.
- `--retry-budget=<n>` עוקף את ספירת הניסיון החוזר לכל נתח (ברירת מחדל: 3) אז CI
  יכול להעלות רגרסיות מהר יותר בעת בדיקת תרחישי כשל.

`fetch_report.json` מציג מדדים מצטברים (`chunk_retry_total`,
`provider_failure_rate` וכו') מתאים להצהרות CI ולצפיות.

## 5. עדכוני רישום וממשל

כאשר מציעים פרופילי צ'אנקר חדשים:

1. מחבר את המתאר ב-`sorafs_manifest::chunker_registry_data`.
2. עדכון `docs/source/sorafs/chunker_registry.md` ואמנות קשורות.
3. חידוש מתקנים (`export_vectors`) ולכידת מניפסטים חתומים.
4. הגש את דוח הציות לצ'טר עם חתימות ממשל.

אוטומציה צריכה להעדיף ידיות קנוניות (`namespace.name@semver`) וליפול