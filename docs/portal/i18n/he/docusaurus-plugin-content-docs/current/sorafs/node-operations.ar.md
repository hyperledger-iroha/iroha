---
lang: ar
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3862ed7ad88363629294782cc24ba791bfadc76b3522b2bbb30f0b9f0946c01b
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
id: node-operations
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/runbooks/sorafs_node_ops.md`. שמרו על סנכרון שתי הגרסאות עד שהסט הישן של תיעוד Sphinx יוסר.
:::

## סקירה כללית

ראנבוק זה מנחה את המפעילים באימות פריסה משולבת של `sorafs-node` בתוך Torii. כל
סעיף ממופה ישירות לתוצרי SF-3: סבבי pin/fetch, התאוששות אחרי הפעלה מחדש,
דחיית מכסה ודגימות PoR.

## 1. דרישות מקדימות

- הפעילו את עובד האחסון ב-`torii.sorafs.storage`:

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- ודאו שלתהליך Torii יש גישת קריאה/כתיבה ל-`data_dir`.
- אשרו שהצומת מפרסם את הקיבולת הצפויה דרך `GET /v1/sorafs/capacity/state` לאחר
  שנרשמה הצהרה.
- כאשר ההחלקה מופעלת, הדשבורדים חושפים גם את מוני GiB·hour/PoR הגולמיים וגם את
  המוחלקים כדי להדגיש מגמות ללא ג'יטר לצד ערכים נקודתיים.

### הרצת בדיקה של CLI (אופציונלי)

לפני חשיפת נקודות קצה HTTP אפשר לבצע בדיקת תקינות ל-backend האחסון עם ה-CLI
המובנה.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

הפקודות מדפיסות תקצירי Norito JSON ומסרבות לחוסר התאמה של פרופיל chunk או digest,
מה שהופך אותן לשימושיות לבדיקות smoke של CI לפני חיבור Torii.【crates/sorafs_node/tests/cli.rs#L1】

### חזרה על הוכחת PoR

המפעילים יכולים כעת להריץ מחדש מקומית ארטיפקטים של PoR שהונפקו על ידי ממשל לפני
העלאה ל-Torii. ה-CLI משתמש באותו נתיב ingest של `sorafs-node`, כך שהרצות מקומיות
חושפות את אותן שגיאות אימות שה-HTTP API היה מחזיר.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

הפקודה פולטת תקציר JSON (digest של manifest, מזהה ספק, digest של proof, מספר
הדגימות ותוצאת verdict אופציונלית). ספקו `--manifest-id=<hex>` כדי לוודא שה-manifest
המאוחסן תואם ל-digest של האתגר, ו-`--json-out=<path>` כאשר רוצים לארכב את הסיכום
עם הארטיפקטים המקוריים כהוכחת ביקורת. הוספת `--verdict` מאפשרת לתרגל את לולאת
האתגר → ההוכחה → פסק הדין במצב אופליין לפני קריאה ל-HTTP API.

לאחר ש-Torii פעיל ניתן לשלוף את אותם ארטיפקטים דרך HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

שתי נקודות הקצה מוגשות על ידי עובד האחסון המשולב, כך שבדיקות smoke של CLI ובדיקות
gateway נשארות מסונכרנות.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. סבב Pin → Fetch

1. הפיקו חבילה של manifest + payload (למשל עם
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. שלחו את ה-manifest בקידוד base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   ה-JSON של הבקשה חייב לכלול `manifest_b64` ו-`payload_b64`. תגובה מוצלחת מחזירה
   `manifest_id_hex` ואת digest ה-payload.
3. אחזרו את הנתונים שהוצמדו:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   פענחו base64 את השדה `data_b64` ואמתו שהוא תואם לבייטים המקוריים.

## 3. תרגיל התאוששות אחרי הפעלה מחדש

1. הצמידו לפחות manifest אחד כפי שמתואר למעלה.
2. הפעילו מחדש את תהליך Torii (או את הצומת כולו).
3. שלחו שוב את בקשת ה-fetch. ה-payload חייב להישאר בר-שליפה וה-digest המוחזר
   חייב להתאים לערך שלפני ההפעלה מחדש.
4. בדקו את `GET /v1/sorafs/storage/state` כדי לוודא ש-`bytes_used` משקף את
   ה-manifests שנשמרו לאחר האתחול מחדש.

## 4. בדיקת דחיית מכסה

1. הורידו זמנית את `torii.sorafs.storage.max_capacity_bytes` לערך קטן (למשל הגודל
   של manifest יחיד).
2. הצמידו manifest אחד; הבקשה אמורה להצליח.
3. נסו להצמיד manifest נוסף בגודל דומה. Torii חייב לדחות את הבקשה עם HTTP `400`
   והודעת שגיאה שמכילה `storage capacity exceeded`.
4. החזירו את מגבלת הקיבולת הרגילה כשמסיימים.

## 5. בדיקת דגימת PoR

1. הצמידו manifest.
2. בקשו דגימת PoR:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. ודאו שהתגובה מכילה `samples` במספר הדגימות המבוקש ושכל הוכחה מתאמת מול שורש
   ה-manifest המאוחסן.

## 6. Hooks לאוטומציה

- CI / בדיקות smoke יכולות לעשות שימוש חוזר בבדיקות הייעודיות שנוספו ב:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  שמכסות `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`,
  ו-`por_sampling_returns_verified_proofs`.
- הדשבורדים צריכים לעקוב אחר:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` ו-`torii_sorafs_storage_fetch_inflight`
  - מוני הצלחה/כשל של PoR שמוצגים דרך `/v1/sorafs/capacity/state`
  - ניסיונות פרסום settlement דרך `sorafs_node_deal_publish_total{result=success|failure}`

ביצוע התרגילים הללו מבטיח שעובד האחסון המשולב יוכל לקלוט נתונים, לשרוד הפעלות
מחדש, לכבד מכסות מוגדרות ולייצר הוכחות PoR דטרמיניסטיות לפני שהצומת מפרסם קיבולת
לרשת הרחבה.
