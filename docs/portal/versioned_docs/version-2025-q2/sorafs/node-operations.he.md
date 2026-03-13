---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T15:38:30.655980+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: node-operations-he
slug: /sorafs/node-operations-he
---

:::הערה מקור קנוני
מראות `docs/source/sorafs/runbooks/sorafs_node_ops.md`. שמור את שני העותקים מיושרים על פני מהדורות.
:::

## סקירה כללית

ספר הפעלה זה מנחה מפעילים דרך אימות פריסת `sorafs-node` משובצת בתוך Torii. כל מקטע ממפה ישירות לתוצרי SF-3: נסיעות סיכה/אחזור הלוך ושוב, הפעלה מחדש של התאוששות, דחיית מכסה ודגימת PoR.

## 1. דרישות מוקדמות

- אפשר את עובד האחסון ב-`torii.sorafs.storage`:

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

- ודא שלתהליך Torii יש גישת קריאה/כתיבה ל-`data_dir`.
- אשר שהצומת מפרסם את הקיבולת הצפויה דרך `GET /v2/sorafs/capacity/state` ברגע שהצהרה נרשמה.
- כאשר החלקה מופעלת, לוחות מחוונים חושפים גם את מונה ה-GiB·hour/PoR הגולמי וגם את מוני ה-GiB·hour/PoR המוחלקים כדי להדגיש מגמות נטולות ריצוד לצד ערכי ספוט.

### CLI Dry Run (אופציונלי)

לפני חשיפת נקודות קצה HTTP, תוכל לבדוק את קצה האחסון האחורי באמצעות ה-CLI המצורף.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

הפקודות מדפיסות סיכומי JSON של Norito ומסרבות אי-התאמות של פרופילים או עיכול, מה שהופך אותן לשימושיות עבור בדיקות עשן CI לפני חיווט Torii.【crates/sorafs_node/tests/cli.rs#L1】

ברגע ש-Torii פעיל, אתה יכול לאחזר את אותם חפצים באמצעות HTTP:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

שתי נקודות הקצה מוגשות על ידי עובד האחסון המשובץ, כך שבדיקות עשן CLI ובדיקות שער נשארות מסונכרנות.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.59】

## 2. הצמד → אחזר הלוך ושוב

1. הפק חבילת מניפסט + מטען (לדוגמה עם `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. שלח את המניפסט עם קידוד base64:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   הבקשה JSON חייבת להכיל `manifest_b64` ו-`payload_b64`. תגובה מוצלחת מחזירה את `manifest_id_hex` ואת תקציר המטען.
3. אחזר את הנתונים המוצמדים:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-פענח את השדה `data_b64` וודא שהוא תואם לבייטים המקוריים.

## 3. הפעל מחדש את תרגיל השחזור

1. הצמד לפחות מניפסט אחד כמו לעיל.
2. הפעל מחדש את תהליך Torii (או את כל הצומת).
3. שלח מחדש את בקשת האחזור. המטען עדיין חייב להיות ניתן לאחזור והתקציר המוחזר חייב להתאים לערך שלפני ההפעלה מחדש.
4. בדוק את `GET /v2/sorafs/storage/state` כדי לוודא ש-`bytes_used` משקף את המניפסטים המתמשכים לאחר האתחול מחדש.

## 4. מבחן דחיית מכסות

1. הורידו באופן זמני את `torii.sorafs.storage.max_capacity_bytes` לערך קטן (לדוגמה גודל של מניפסט בודד).
2. הצמד מניפסט אחד; הבקשה אמורה להצליח.
3. נסה להצמיד מניפסט שני בגודל דומה. על Torii לדחות את הבקשה עם HTTP `400` והודעת שגיאה המכילה `storage capacity exceeded`.
4. שחזר את מגבלת הקיבולת הרגילה בסיום.

## 5. בדיקת דגימת PoR

1. הצמד מניפסט.
2. בקש מדגם PoR:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. ודא שהתגובה מכילה `samples` עם הספירה המבוקשת ושכל הוכחה מאומתת מול שורש המניפסט המאוחסן.

## 6. ווי אוטומציה

- בדיקות CI / עשן יכולות לעשות שימוש חוזר בבדיקות הממוקדות שנוספו ב:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```המכסה את `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`, ו-`por_sampling_returns_verified_proofs`.
- לוחות מחוונים צריכים לעקוב אחר:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` ו-`torii_sorafs_storage_fetch_inflight`
  - מדדי הצלחה/כשל של PoR הופיעו דרך `/v2/sorafs/capacity/state`
  - ניסיונות פרסום בהסדר דרך `sorafs_node_deal_publish_total{result=success|failure}`

מעקב אחר תרגילים אלה מבטיח שעובד האחסון המשובץ יכול להטמיע נתונים, לשרוד אתחול מחדש, לכבד מכסות מוגדרות וליצור הוכחות PoR דטרמיניסטיות לפני שהצומת מפרסם קיבולת לרשת הרחבה יותר.
