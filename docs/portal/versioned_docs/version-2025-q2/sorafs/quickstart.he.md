---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-04T17:06:14.405886+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS התחלה מהירה

מדריך מעשי זה עובר דרך פרופיל הצ'אנקר הדטרמיניסטי SF-1,
חתימת מניפסט, וזרימת אחזור מרובה ספקים העומדים בבסיס ה-SoraFS
צינור אחסון. התאם אותו עם [צלילת צנרת גלויה לעומק](manifest-pipeline.md)
עבור הערות עיצוב וחומר התייחסות לדגל CLI.

## דרישות מוקדמות

- שרשרת כלי חלודה (`rustup update`), סביבת עבודה משובטת מקומית.
- אופציונלי: [זוג מפתחות Ed25519 שנוצר על ידי OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  לחתימה על מניפסטים.
- אופציונלי: Node.js ≥ 18 אם אתה מתכנן לצפות בתצוגה מקדימה של פורטל Docusaurus.

הגדר את `export RUST_LOG=info` תוך כדי ניסוי כדי להציג הודעות CLI מועילות.

## 1. רענן את הגופים הדטרמיניסטיים

צור מחדש את וקטורי החתכים הקנוניים של SF-1. הפקודה גם פולטת חתום
מעטפות מניפסט כאשר `--signing-key` מסופק; השתמש ב-`--allow-unsigned`
במהלך פיתוח מקומי בלבד.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

פלטים:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (אם חתום)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. חלק מטען ובדוק את התוכנית

השתמש ב-`sorafs_chunker` כדי לחלק קובץ או ארכיון שרירותיים:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

שדות מפתח:

- `profile` / `break_mask` - מאשר את הפרמטרים `sorafs.sf1@1.0.0`.
- `chunks[]` - הוזמנו קיזוזים, אורכים ועיכובי BLAKE3 נתחים.

עבור מתקנים גדולים יותר, הפעל את הרגרסיה המגובה ב-proptest כדי להבטיח סטרימינג ו
נתחי אצווה הישאר מסונכרן:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. בנה וחתום על מניפסט

עטפו את תוכנית הנתחים, הכינויים וחתימות הממשל למניפסט באמצעות
`sorafs-manifest-stub`. הפקודה למטה מציגה מטען של קובץ יחיד; לעבור
נתיב ספרייה לאריזת עץ (ה-CLI מטייל בו מבחינה לקסיקוגרפית).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

בדוק את `/tmp/docs.report.json` עבור:

- `chunking.chunk_digest_sha3_256` - תקציר SHA3 של קיזוזים/אורכים, תואם את
  אביזרי צ'אנקר.
- `manifest.manifest_blake3` - תקציר BLAKE3 חתום במעטפת המניפסט.
- `chunk_fetch_specs[]` - הוראות אחזור הוזמנו עבור מתזמרים.

כשתהיה מוכן לספק חתימות אמיתיות, הוסף `--signing-key` ו-`--signer`
טיעונים. הפקודה מאמתת כל חתימה של Ed25519 לפני כתיבת ה-
מעטפה.

## 4. הדמיית אחזור מרובה ספקים

השתמש ב-CLI של המפתחים כדי להפעיל מחדש את תוכנית הנתחים מול אחד או יותר
ספקים. זה אידיאלי עבור מבחני עשן CI ואב-טיפוס של מתזמר.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

קביעות:

- `payload_digest_hex` חייב להתאים לדוח המניפסט.
- `provider_reports[]` מציג ספירות הצלחה/כישלונות לכל ספק.
- `chunk_retry_total` ללא אפס מדגיש התאמות של לחץ גב.
- עברו את `--max-peers=<n>` כדי להגביל את מספר הספקים המתוכננים להרצה
  ולשמור על סימולציות CI ממוקדות במועמדים העיקריים.
- `--retry-budget=<n>` עוקף את ברירת המחדל של ספירת הניסיון החוזר לכל נתח (3) כך שאתה
  יכול להציף רגרסיות מתזמר מהר יותר בעת הזרקת כשלים.

הוסף את `--expect-payload-digest=<hex>` ו-`--expect-payload-len=<bytes>` כדי להיכשל
מהיר כאשר המטען המשוחזר חורג מהמניפסט.

## 5. השלבים הבאים- **שילוב ממשל** - צינור ה-manifest digest ו
  `manifest_signatures.json` לתוך זרימת העבודה של המועצה כדי שמרשם הפינים יוכל
  לפרסם זמינות.
- **משא ומתן ברישום** - התייעצו עם [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  לפני רישום פרופילים חדשים. אוטומציה צריכה להעדיף ידיות קנוניות
  (`namespace.name@semver`) על מזהים מספריים.
- **אוטומציה של CI** - הוסף את הפקודות שלמעלה כדי לשחרר צינורות כך שמסמכים,
  מתקנים וחפצים מפרסמים מניפסטים דטרמיניסטיים לצד חתומים
  מטא נתונים.