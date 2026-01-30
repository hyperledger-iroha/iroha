---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/node-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 13cb6fc389c40d641c252112f6d687c16c25eb89922b7a7bbdc8e985efdd5075
source_last_modified: "2025-11-14T04:43:21.892889+00:00"
translation_last_reviewed: 2026-01-30
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/sorafs_node_plan.md`. שמרו על שתי הגרסאות מסונכרנות עד שהמסמכים הישנים של Sphinx יופסקו.
:::

SF-3 מספקת את ה-crate הראשון שניתן להרצה `sorafs-node` שממיר תהליך Iroha/Torii לספק אחסון SoraFS. השתמשו בתכנית הזו לצד [מדריך אחסון הצומת](node-storage.md), [מדיניות קבלת ספקים](provider-admission-policy.md) ו-[מפת הדרכים לשוק קיבולת האחסון](storage-capacity-marketplace.md) בעת תזמון המסירות.

## היקף יעד (אבן דרך M1)

1. **אינטגרציית Chunk Store.** לעטוף את `sorafs_car::ChunkStore` ב-backend מתמיד המאחסן bytes של chunks, manifests ועצי PoR בתיקיית הנתונים המוגדרת.
2. **נקודות קצה של Gateway.** לחשוף נקודות קצה HTTP של Norito להגשת pin, fetch של chunks, דגימות PoR וטלמטריית אחסון בתוך תהליך Torii.
3. **צנרת קונפיגורציה.** להוסיף struct הגדרות `SoraFsStorage` (דגל הפעלה, קיבולת, ספריות, מגבלות מקביליות) ולחבר דרך `iroha_config`, `iroha_core` ו-`iroha_torii`.
4. **קצבה/תזמון.** לאכוף מגבלות דיסק/מקביליות שהוגדרו על ידי המפעיל ולתזמן בקשות עם back-pressure.
5. **טלמטריה.** להוציא מדדים/לוגים עבור הצלחת pin, השהיית fetch של chunks, ניצול קיבולת ותוצאות דגימת PoR.

## פירוק עבודה

### A. מבנה crate ומודולים

| משימה | אחראי | הערות |
|------|-------|-------|
| ליצור `crates/sorafs_node` עם מודולים: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | צוות Storage | לייצא מחדש טיפוסים לשימוש חוזר עבור אינטגרציית Torii. |
| לממש `StorageConfig` שממופה מתוך `SoraFsStorage` (user → actual → defaults). | צוות Storage / Config WG | להבטיח ששכבות Norito/`iroha_config` נשארות דטרמיניסטיות. |
| לספק חזית `NodeHandle` ש-Torii משתמש בה כדי לשלוח pins/fetches. | צוות Storage | לעטוף את פנימיות האחסון והצנרת הא-סינכרונית. |

### B. Chunk Store מתמיד

| משימה | אחראי | הערות |
|------|-------|-------|
| לבנות backend על הדיסק שעוטף את `sorafs_car::ChunkStore` עם אינדקס manifest על הדיסק (`sled`/`sqlite`). | צוות Storage | פריסת קבצים דטרמיניסטית: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| לשמור מטא-דאטה של PoR (עצים 64 KiB/4 KiB) באמצעות `ChunkStore::sample_leaves`. | צוות Storage | תומך בשחזור לאחר אתחול; נכשל מהר במקרה של שחיתות. |
| לממש integrity replay בזמן הפעלה (rehash של manifests, הסרת pins לא שלמים). | צוות Storage | חוסם את עליית Torii עד לסיום ה-replay. |

### C. נקודות קצה של Gateway

| נקודת קצה | התנהגות | משימות |
|-----------|---------|--------|
| `POST /sorafs/pin` | מקבל `PinProposalV1`, מאמת manifests, מכניס את ההטענה לתור ומחזיר CID של ה-manifest. | אימות פרופיל chunker, אכיפת קצבות, הזרמת נתונים דרך chunk store. |
| `GET /sorafs/chunks/{cid}` + שאילתת range | מגיש bytes של chunk עם כותרות `Content-Chunker`; מכבד את מפרט יכולת ה-range. | שימוש ב-scheduler ובתקציבי זרימה (קשור ליכולת range של SF-2d). |
| `POST /sorafs/por/sample` | מבצע דגימת PoR עבור manifest ומחזיר bundle של הוכחה. | שימוש בדגימה של chunk store, תגובה עם Norito JSON. |
| `GET /sorafs/telemetry` | תקצירים: קיבולת, הצלחת PoR, ספירת שגיאות fetch. | לספק נתונים ללוחות/מפעילים. |

צנרת הריצה מעבירה אינטראקציות PoR דרך `sorafs_node::por`: ה-tracker רושם כל `PorChallengeV1`, `PorProofV1` ו-`AuditVerdictV1` כדי שמדדי `CapacityMeter` ישקפו את פסיקות הממשל ללא לוגיקה ייעודית ב-Torii.【crates/sorafs_node/src/scheduler.rs#L147】

הערות יישום:

- להשתמש במחסנית Axum של Torii עם payloads של `norito::json`.
- להוסיף סכמות Norito לתגובות (`PinResultV1`, `FetchErrorV1`, מבני טלמטריה).

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` מציג כעת עומק backlog יחד עם epoch/deadline העתיק ביותר וטביעות זמן עדכניות של הצלחה/כשל לכל ספק, באמצעות `sorafs_node::NodeHandle::por_ingestion_status`, ו-Torii רושם את המדדים `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` עבור הדשבורדים.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Scheduler ואכיפת קצבות

| משימה | פרטים |
|------|-------|
| קצבת דיסק | לעקוב אחרי bytes בדיסק; לדחות pins חדשים כאשר חוצים `max_capacity_bytes`. לספק hooks לפינוי עתידי. |
| מקביליות fetch | סמפור גלובלי (`max_parallel_fetches`) יחד עם תקציבים לפי ספק שמבוססים על caps של SF-2d range. |
| תור pins | להגביל עבודות ingest ממתינות; לחשוף נקודות מצב Norito לעומק התור. |
| קצב PoR | worker ברקע שמונע על ידי `por_sample_interval_secs`. |

### E. טלמטריה ולוגים

מדדים (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (היסטוגרמה עם תוויות `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

לוגים / אירועים:

- טלמטריית Norito מובנית עבור ingest של ממשל (`StorageTelemetryV1`).
- התראות כאשר ניצול > 90% או שרשרת כשלי PoR חוצה את הסף.

### F. אסטרטגיית בדיקות

1. **בדיקות יחידה.** התמדה של chunk store, חישובי קצבה, אינבריאנטים של scheduler (ראו `crates/sorafs_node/src/scheduler.rs`).
2. **בדיקות אינטגרציה** (`crates/sorafs_node/tests`). סבב pin → fetch, שחזור לאחר אתחול, דחיית קצבה, אימות הוכחות דגימת PoR.
3. **בדיקות אינטגרציה של Torii.** להריץ Torii עם אחסון מופעל ולהפעיל נקודות HTTP דרך `assert_cmd`.
4. **מפת דרכים לכאוס.** תרגולים עתידיים מדמים מחסור בדיסק, IO איטי והסרת ספקים.

## תלותיות

- מדיניות קבלה SF-2b — לוודא שהצמתים מאמתים מעטפות קבלה לפני פרסום adverts.
- שוק קיבולת SF-2c — לקשור טלמטריה להצהרות קיבולת.
- הרחבות advert של SF-2d — לצרוך יכולת range + תקציבי stream כאשר יהיו זמינים.

## קריטריוני יציאה מהשלב

- `cargo run -p sorafs_node --example pin_fetch` עובד מול fixtures מקומיים.
- Torii נבנה עם `--features sorafs-storage` ועובר בדיקות אינטגרציה.
- תיעוד ([מדריך אחסון הצומת](node-storage.md)) מעודכן עם ברירות מחדל ותצורות CLI; runbook למפעיל זמין.
- טלמטריה נראית בדשבורדים של staging; התראות הוגדרו לרוויה בקיבולת ולכשלים ב-PoR.

## תוצרי תיעוד ותפעול

- לעדכן את [אסמכתת אחסון הצומת](node-storage.md) עם ברירות מחדל, שימוש ב-CLI ושלבי troubleshooting.
- לשמור את [runbook תפעול הצומת](node-operations.md) מסונכרן עם המימוש ככל ש-SF-3 מתפתח.
- לפרסם אסמכתאות API עבור נקודות `/sorafs/*` בפורטל המפתחים ולחבר אותן ל-OpenAPI manifest לאחר שה-handlers של Torii ייכנסו.
