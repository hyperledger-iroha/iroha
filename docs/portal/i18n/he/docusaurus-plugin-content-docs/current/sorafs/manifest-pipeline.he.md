---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 592fd6d9649c79ba16b61d019b2193ba7cd49d10cb4a8b0d7ae93ba19e5b7482
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
---

# Chunking של SoraFS → Pipeline של מניפסטים

מסמך משלים זה ל‑quickstart מתאר את ה‑pipeline מקצה לקצה שממיר בתים גולמיים למניפסטים של
Norito המתאימים ל‑Pin Registry של SoraFS. התוכן הותאם מ‑[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
עיינו במסמך הזה עבור המפרט הקנוני ויומן השינויים.

## 1. Chunking דטרמיניסטי

SoraFS משתמש בפרופיל SF-1 (`sorafs.sf1@1.0.0`): hash מתגלגל בהשראת FastCDC עם גודל chunk
מינימלי של 64 KiB, יעד של 256 KiB, מקסימום של 512 KiB ומסכת שבירה `0x0000ffff`. הפרופיל
רשום ב‑`sorafs_manifest::chunker_registry`.

### Helpers של Rust

- `sorafs_car::CarBuildPlan::single_file` – מפיק היסטים, אורכים ודיגסטים BLAKE3 של ה‑chunks
  בזמן הכנת מטא‑נתוני CAR.
- `sorafs_car::ChunkStore` – מזרִים payloads, שומר מטא‑נתוני chunks ומפיק את עץ הדגימה
  Proof-of-Retrievability (PoR) בגודל 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – helper ספרייה מאחורי שתי ה‑CLI.

### כלי CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

ה‑JSON מכיל היסטים מסודרים, אורכים ודיגסטים של chunks. שמרו את התוכנית בעת בניית מניפסטים
או מפרטי שליפה לאורקסטרטור.

### עדי PoR

`ChunkStore` חושף `--por-proof=<chunk>:<segment>:<leaf>` ו‑`--por-sample=<count>` כדי שמבקרים
יוכלו לבקש סטים דטרמיניסטיים של עדים. שלבו את הדגלים עם `--por-proof-out` או
`--por-sample-out` כדי לשמור את ה‑JSON.

## 2. עטיפת מניפסט

`ManifestBuilder` משלב מטא‑נתוני chunks עם נספחי ממשל:

- CID שורש (dag-cbor) והתחייבויות CAR.
- הוכחות alias והצהרות יכולת של ספקים.
- חתימות המועצה ומטא‑נתונים אופציונליים (למשל, מזהי build).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

פלטים חשובים:

- `payload.manifest` – בתים של מניפסט מקודד Norito.
- `payload.report.json` – תקציר קריא לאנשים/אוטומציה, כולל `chunk_fetch_specs`,
  `payload_digest_hex`, דיגסטים של CAR ומטא‑נתוני alias.
- `payload.manifest_signatures.json` – מעטפה המכילה את דיגסט ה‑BLAKE3 של המניפסט, דיגסט
  SHA3 של תוכנית ה‑chunks וחתימות Ed25519 ממוינות.

השתמשו ב‑`--manifest-signatures-in` כדי לאמת מעטפות שסופקו על ידי חותמים חיצוניים לפני
כתיבה מחדש, וב‑`--chunker-profile-id` או `--chunker-profile=<handle>` כדי לנעול את בחירת
הרג'יסטרי.

## 3. פרסום ופינינג

1. **שליחה לממשל** – ספקו את דיגסט המניפסט ואת מעטפת החתימות למועצה כדי שה‑pin יוכל להיות
   מאושר. מבקרים חיצוניים צריכים לשמור את דיגסט ה‑SHA3 של תוכנית ה‑chunks לצד דיגסט המניפסט.
2. **פינינג payloads** – העלו את ארכיון ה‑CAR (ואת אינדקס ה‑CAR האופציונלי) המוזכר במניפסט
   ל‑Pin Registry. ודאו שהמניפסט וה‑CAR חולקים את אותו CID שורש.
3. **רישום טלמטריה** – שמרו את הדוח JSON, עדי PoR וכל מדדי fetch בארטיפקטים של הריליס. הרשומות
   הללו מזינות דשבורדים למפעילים ועוזרות לשחזר תקלות בלי להוריד payloads גדולים.

## 4. סימולציית fetch מרובת‑ספקים

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` מגדיל את המקביליות לכל ספק (`#4` לעיל).
- `@<weight>` מכוונן את הטיית התזמון; ברירת המחדל היא 1.
- `--max-peers=<n>` מגביל את מספר הספקים המתוזמנים להרצה כאשר הגילוי מחזיר יותר מועמדים
  מהנדרש.
- `--expect-payload-digest` ו‑`--expect-payload-len` מגנים מפני השחתה שקטה.
- `--provider-advert=name=advert.to` מאמת את יכולות הספק לפני השימוש בסימולציה.
- `--retry-budget=<n>` מחליף את מספר הניסיונות לכל chunk (ברירת מחדל: 3) כדי ש‑CI תחשוף
  רגרסיות מהר יותר בעת בדיקת תרחישי כשל.

`fetch_report.json` מציג מדדים מצטברים (`chunk_retry_total`, `provider_failure_rate`, וכו')
שמתאימים לאסרטים של CI ולתצפיתיות.

## 5. עדכוני רג'יסטרי וממשל

כאשר מציעים פרופילי chunker חדשים:

1. כתבו את המתאר ב‑`sorafs_manifest::chunker_registry_data`.
2. עדכנו את `docs/source/sorafs/chunker_registry.md` ואת ה‑charters הקשורים.
3. שחזרו fixtures (`export_vectors`) ואספו מניפסטים חתומים.
4. הגישו את דוח תאימות ה‑charter עם חתימות ממשל.

האוטומציה צריכה להעדיף handles קנוניים (`namespace.name@semver`) ולחזור ל‑IDs מספריים רק
כאשר נדרש תאימות לאחור.
