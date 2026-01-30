---
lang: pt
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 34d18f4fd945a1081c8b3ee8c12576b5eac6c76f79e51a6e123e84c41449f5b3
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
---

# מדריך התחלה מהירה ל‑SoraFS

מדריך מעשי זה עובר דרך פרופיל ה‑chunker הדטרמיניסטי SF-1,
חתימת המניפסטים וזרימת שליפה מרובת ספקים שמעמידים את צינור האחסון של SoraFS.
שלבו אותו עם [צלילה עמוקה בצינור המניפסטים](manifest-pipeline.md)
להערות תכנון ולחומר עזר לדגלי CLI.

## דרישות מקדימות

- Toolchain של Rust (`rustup update`), ה‑workspace משוכפל מקומית.
- אופציונלי: [זוג מפתחות Ed25519 תואם OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  לחתימת מניפסטים.
- אופציונלי: Node.js ≥ 18 אם אתם מתכננים לצפות בפורטל Docusaurus.

הגדירו `export RUST_LOG=info` בזמן ניסויים כדי להציג הודעות CLI שימושיות.

## 1. רענון ה‑fixtures הדטרמיניסטיות

צרו מחדש את וקטורי ה‑chunking הקנוניים של SF-1. הפקודה גם מפיקה מעטפות מניפסט
חתומות כאשר מספקים `--signing-key`; השתמשו ב‑`--allow-unsigned` רק במהלך פיתוח מקומי.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

תוצרים:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (אם נחתם)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. חלקו payload ובדקו את התכנית

השתמשו ב‑`sorafs_chunker` כדי לחלק קובץ או ארכיון שרירותי:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

שדות מפתח:

- `profile` / `break_mask` – מאשר את פרמטרי `sorafs.sf1@1.0.0`.
- `chunks[]` – היסטים מסודרים, אורכים ודיגסטים BLAKE3 של ה‑chunks.

ל‑fixtures גדולים יותר, הריצו את בדיקת הרגרסיה המבוססת על proptest כדי להבטיח
שה‑chunking בסטרימינג ובאצווה נשארים מסונכרנים:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. בנו וחתמו מניפסט

עטפו את תוכנית ה‑chunks, הכינויים וחתימות הממשל במניפסט באמצעות
`sorafs-manifest-stub`. הפקודה למטה מדגימה payload של קובץ יחיד; העבירו נתיב
תיקייה כדי לארוז עץ (ה‑CLI סורק אותו בסדר לקסיקוגרפי).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

בדקו את `/tmp/docs.report.json` עבור:

- `chunking.chunk_digest_sha3_256` – דיגסט SHA3 של היסטים/אורכים, תואם את fixtures של
  ה‑chunker.
- `manifest.manifest_blake3` – דיגסט BLAKE3 שנחתם במעטפת המניפסט.
- `chunk_fetch_specs[]` – הוראות שליפה מסודרות לאורקסטרטורים.

כשאתם מוכנים לספק חתימות אמיתיות, הוסיפו את הארגומנטים `--signing-key` ו‑`--signer`.
הפקודה מאמתת כל חתימת Ed25519 לפני כתיבת המעטפת.

## 4. סימולציה של שליפה מרובת ספקים

השתמשו ב‑CLI של fetch לפיתוח כדי לשחזר את תוכנית ה‑chunks מול ספק אחד או יותר.
זה אידיאלי ל‑CI smoke tests ולפרוטוטייפינג של אורקסטרטור.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

בדיקות:

- `payload_digest_hex` חייב להתאים לדוח המניפסט.
- `provider_reports[]` מציג ספירות הצלחה/כשל לכל ספק.
- `chunk_retry_total` שאינו אפס מדגיש התאמות back-pressure.
- העבירו `--max-peers=<n>` כדי להגביל את מספר הספקים המתוזמנים לריצה ולשמור על
  סימולציות ה‑CI ממוקדות במועמדים הראשיים.
- `--retry-budget=<n>` עוקף את מספר הנסיונות החוזרים המוגדר כברירת מחדל לכל chunk (3)
  כדי לחשוף רגרסיות אורקסטרטור מהר יותר בעת הזרקת כשלים.

הוסיפו `--expect-payload-digest=<hex>` ו‑`--expect-payload-len=<bytes>` כדי להיכשל מהר
כאשר ה‑payload המשוחזר חורג מה‑מניפסט.

## 5. הצעדים הבאים

- **אינטגרציית ממשל** – העבירו את דיגסט המניפסט ואת `manifest_signatures.json`
  לזרימת העבודה של המועצה כדי ש‑Pin Registry יוכל לפרסם זמינות.
- **משא ומתן על הרישום** – עיינו ב‑[`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  לפני רישום פרופילים חדשים. האוטומציה צריכה להעדיף handles קנוניים
  (`namespace.name@semver`) על פני מזהים מספריים.
- **אוטומציית CI** – הוסיפו את הפקודות לעיל לפייפלייני release כדי שהמסמכים,
  ה‑fixtures וה‑artifacts יפרסמו מניפסטים דטרמיניסטיים לצד מטאדטה חתומה.
