---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/chunker-conformance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4549913f06e2428c92ad7b209cf48265101014ca692ee60775cccbf3bfcba4ca
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
id: chunker-conformance
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/chunker_conformance.md`. שמרו על שתי הגרסאות מסונכרנות עד שהמסמכים הישנים ייצאו משימוש.
:::

מדריך זה מקבע את הדרישות שכל מימוש חייב לעמוד בהן כדי להישאר תואם לפרופיל ה-chunker הדטרמיניסטי של SoraFS (SF1).
הוא גם מתעד את תהליך הרגנרציה, מדיניות החתימה ושלבי האימות כדי שצרכני fixtures ב-SDKs יישארו מסונכרנים.

## פרופיל קנוני

- Seed קלט (hex): `0000000000dec0ded`
- גודל יעד: 262144 bytes (256 KiB)
- גודל מינימלי: 65536 bytes (64 KiB)
- גודל מקסימלי: 524288 bytes (512 KiB)
- פולינום rolling: `0x3DA3358B4DC173`
- Seed טבלת gear: `sorafs-v1-gear`
- Break mask: `0x0000FFFF`

מימוש ייחוס: `sorafs_chunker::chunk_bytes_with_digests_profile`.
כל האצה SIMD חייבת להפיק גבולות ו-digests זהים.

## חבילת fixtures

`cargo run --locked -p sorafs_chunker --bin export_vectors` מייצר מחדש את
fixtures ומוציא את הקבצים הבאים תחת `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — גבולות chunk קנוניים לצרכני Rust,
  TypeScript ו-Go. כל קובץ מצהיר על ה-handle הקנוני כערך הראשון ב-`profile_aliases`,
  ולאחריו alias ישנים (למשל `sorafs.sf1@1.0.0`, ואז `sorafs.sf1@1.0.0`). הסדר נאכף על ידי
  `ensure_charter_compliance` ואסור לשנותו.
- `manifest_blake3.json` — manifest מאומת BLAKE3 שמכסה כל קובץ fixtures.
- `manifest_signatures.json` — חתימות המועצה (Ed25519) על digest ה-manifest.
- `sf1_profile_v1_backpressure.json` ו-corpora גולמיים בתוך `fuzz/` —
  תרחישי streaming דטרמיניסטיים המשמשים לבדיקות back-pressure של ה-chunker.

### מדיניות חתימה

רגנרציה של fixtures **חייבת** לכלול חתימה תקפה של המועצה. המחולל דוחה פלט לא חתום
אלא אם `--allow-unsigned` נמסר במפורש (מיועד לניסויים מקומיים בלבד). מעטפות החתימה הן append-only
ומבצעות דה-דופליקציה לפי חותם.

כדי להוסיף חתימת מועצה:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## אימות

Helper ה-CI `ci/check_sorafs_fixtures.sh` מריץ מחדש את המחולל עם
`--locked`. אם fixtures חורגים או חסרות חתימות, העבודה נכשלת. השתמשו
בסקריפט הזה ב-workflows ליליים ולפני שליחת שינויי fixtures.

שלבי אימות ידניים:

1. הריצו `cargo test -p sorafs_chunker`.
2. הפעילו `ci/check_sorafs_fixtures.sh` מקומית.
3. ודאו ש-`git status -- fixtures/sorafs_chunker` נקי.

## Playbook לשדרוג

בעת הצעת פרופיל chunker חדש או עדכון SF1:

ראו גם: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) עבור
דרישות מטאדאטה, תבניות הצעה וצ'קליסטים של ולידציה.

1. נסחו `ChunkProfileUpgradeProposalV1` (ראו RFC SF-1) עם פרמטרים חדשים.
2. רגנרו fixtures דרך `export_vectors` ותעדו את digest ה-manifest החדש.
3. חתמו על ה-manifest עם הקוורום הנדרש של המועצה. כל החתימות חייבות
   להתווסף ל-`manifest_signatures.json`.
4. עדכנו fixtures של SDK מושפעים (Rust/Go/TS) והבטיחו פריטי parity חוצי runtime.
5. רגנרו corpora fuzz אם הפרמטרים משתנים.
6. עדכנו את המדריך עם handle פרופיל חדש, seeds ו-digest.
7. שלחו את השינוי יחד עם בדיקות מעודכנות ועדכוני roadmap.

שינויים שמשפיעים על גבולות chunk או digests בלי לעקוב אחרי תהליך זה
אינם תקפים ואסור למזג אותם.
