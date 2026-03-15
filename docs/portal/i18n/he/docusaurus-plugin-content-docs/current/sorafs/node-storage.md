---
id: node-storage
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-storage.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/sorafs_node_storage.md`. שמרו על שתי הגרסאות מסונכרנות עד שהסט הישן של תיעוד Sphinx יופסק.
:::

## תכנון אחסון לצומת SoraFS (טיוטה)

מסמך זה מחדד כיצד צומת Iroha (Torii) יכול להצטרף לשכבת זמינות הנתונים של
SoraFS ולהקדיש חלק מהדיסק המקומי לאחסון והגשת chunks. הוא משלים את מפרט
ה-discovery `sorafs_node_client_protocol.md` ואת עבודת ה-fixtures של SF-1b
באמצעות פירוט הארכיטקטורה בצד האחסון, בקרות משאבים וצנרת הקונפיגורציה שחייבת
להיטען בצומת ובמסלולי ה-gateway. התרגולים המעשיים נמצאים ב
[Runbook פעולות הצומת](./node-operations).

### מטרות

- לאפשר לכל validator או תהליך Iroha משלים לחשוף דיסק פנוי כספק SoraFS מבלי לפגוע
  באחריות הליבה של ה-ledger.
- לשמור על מודול האחסון דטרמיניסטי ומונחה Norito: manifests, תוכניות chunk, שורשי
  Proof-of-Retrievability (PoR) ו-adverts של ספק הם מקור האמת.
- לאכוף קצבות מוגדרות על ידי מפעיל כדי שהצומת לא ימצה את המשאבים שלו בקבלת יותר
  מדי בקשות pin או fetch.
- לחשוף בריאות/טלמטריה (דגימות PoR, השהיית fetch של chunk, לחץ דיסק) חזרה לגוב
  וללקוחות.

### ארכיטקטורה ברמה גבוהה

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

מודולים מרכזיים:

- **Gateway**: חושף נקודות קצה Norito HTTP להצעות pin, בקשות fetch של chunks, דגימות PoR וטלמטריה. הוא מאמת payloads של Norito ומנתב את הבקשות אל chunk store. משתמש ב-stack ה-HTTP הקיים של Torii כדי להימנע מדמון נוסף.
- **Pin Registry**: מצב pin של manifests הנעקב ב-`iroha_data_model::sorafs` ו-`iroha_core`. כאשר manifest מתקבל, הרישום שומר את digest ה-manifest, digest של תוכנית chunk, שורש PoR ודגלי יכולת של הספק.
- **Chunk Storage**: מימוש `ChunkStore` על דיסק שמקבל manifests חתומים, מייצר תוכניות chunk באמצעות `ChunkProfile::DEFAULT`, ומאחסן chunks בפריסה דטרמיניסטית. כל chunk משויך לטביעת אצבע תוכן ולמטא-דאטה של PoR כדי שהדגימות יוכלו לאמת בלי לקרוא מחדש את כל הקובץ.
- **Quota/Scheduler**: אוכף מגבלות שהוגדרו על ידי המפעיל (מקסימום bytes בדיסק, מקסימום pins ממתינים, מקסימום fetches מקבילים, TTL ל-chunk) ומסנכרן IO כדי שמשימות ledger לא ייחנקו. ה-scheduler אחראי גם לשרת הוכחות PoR ובקשות דגימה עם CPU מוגבל.

### קונפיגורציה

הוסיפו סעיף חדש ל-`iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # תג אופציונלי קריא
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: מתג השתתפות. כאשר false, ה-gateway מחזיר 503 עבור נקודות קצה של אחסון והצומת אינו מפרסם עצמו ב-discovery.
- `data_dir`: תיקיית השורש לנתוני chunk, עצי PoR וטלמטריית fetch. ברירת המחדל `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: גבול קשיח לנתוני chunks מוצמדים. משימת רקע דוחה pins חדשים עם הגעה לגבול.
- `max_parallel_fetches`: תקרת מקביליות שמוטלת על ידי ה-scheduler כדי לאזן רוחב פס/IO דיסק עם עומס הוולידטור.
- `max_pins`: מספר pins של manifest שהצומת מקבל לפני החלת eviction/back pressure.
- `por_sample_interval_secs`: קצב עבודות דגימת PoR אוטומטיות. כל עבודה מדגמת `N` עלים (ניתן להגדרה לכל manifest) ומוציאה אירועי טלמטריה. הממשל יכול להגדיל את `N` באופן דטרמיניסטי באמצעות המפתח `profile.sample_multiplier` (מספר שלם `1-4`). הערך יכול להיות מספר/מחרוזת יחיד או אובייקט עם overrides לפי פרופיל, למשל `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: מבנה שמשמש את מחולל ה-advert כדי למלא שדות `ProviderAdvertV1` (stake pointer, רמזי QoS, topics). אם הוא מושמט, הצומת משתמש בברירות המחדל מרישום הממשל.

צנרת קונפיגורציה:

- `[sorafs.storage]` מוגדר ב-`iroha_config` כ-`SorafsStorage` ונטען מקובץ הקונפיגורציה של הצומת.
- `iroha_core` ו-`iroha_torii` מעבירים את הגדרות האחסון ל-gateway builder ול-chunk store בעת ההפעלה.
- קיימים overrides ל-dev/test (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), אך בסביבות פרודקשן יש להסתמך על קובץ הקונפיגורציה.

### כלי CLI

בעת שחיבור ממשקי ה-HTTP של Torii עדיין בעבודה, ה-crate `sorafs_node` מספק CLI דק שמאפשר למפעילים להריץ תרגילי ingest/export מול ה-backend המתמיד.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` מצפה ל-manifest `.to` מקודד Norito עם payload תואם. הוא משחזר את תוכנית ה-chunk מתוך פרופיל ה-chunking של ה-manifest, כופה התאמת digest, שומר את קבצי ה-chunk, ובאופן אופציונלי מוציא JSON `chunk_fetch_specs` כדי שכלי downstream יוכלו לבדוק את הפריסה.
- `export` מקבל מזהה manifest וכותב את ה-manifest/payload המאוחסן לדיסק (עם plan JSON אופציונלי) כדי לשמור על fixtures ניתנים לשחזור.

שתי הפקודות מדפיסות תקציר Norito JSON ל-stdout, כך שקל לשלב בסקריפטים. ה-CLI מכוסה בבדיקת אינטגרציה כדי לוודא ש-manifests ו-payloads מבצעים round-trip נכון לפני שממשקי Torii זמינים.【crates/sorafs_node/tests/cli.rs:1】

> התאמה ל-HTTP
>
> ה-gateway של Torii חושף כעת helpers לקריאה בלבד שמבוססים על אותו `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — מחזיר manifest Norito מאוחסן (base64) יחד עם digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — מחזיר את תוכנית ה-chunk הדטרמיניסטית JSON (`chunk_fetch_specs`) לכלי downstream.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> נקודות קצה אלה משקפות את פלט ה-CLI כך שפייפליינים יכולים לעבור מסקריפטים מקומיים לבדיקות HTTP בלי לשנות פרסרים.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### מחזור חיי הצומת

1. **הפעלה**:
   - אם האחסון מופעל, הצומת מאתחל את ה-chunk store עם הספרייה והקיבולת שהוגדרו. זה כולל אימות או יצירה של מסד ה-manifest של PoR והרצת manifests מוצמדים כדי לחמם caches.
   - לרשום את מסלולי ה-gateway של SoraFS (נקודות קצה Norito JSON POST/GET עבור pin, fetch, דגימות PoR וטלמטריה).
   - להפעיל את worker דגימת PoR ומנטר הקצבות.
2. **Discovery / Adverts**:
   - ליצור מסמכי `ProviderAdvertV1` לפי הקיבולת/בריאות הנוכחית, לחתום עם המפתח המאושר על ידי המועצה, ולפרסם דרך ערוץ ה-discovery. להשתמש ברשימת `profile_aliases` כדי לשמור על handles קנוניים וישנים זמינים.
3. **זרימת pin**:
   - ה-gateway מקבל manifest חתום (כולל תוכנית chunk, שורש PoR וחתימות מועצה). מאמת את רשימת ה-aliases (`sorafs.sf1@1.0.0` נדרש) ומוודא שתוכנית ה-chunk תואמת ל-metadata של ה-manifest.
   - בדיקת קצבות. אם קיבולת/מגבלות pin יחרגו, להחזיר שגיאת מדיניות (Norito מובנה).
   - להזרמת נתוני chunk אל `ChunkStore` תוך אימות digests בזמן ingest. לעדכן עצי PoR ולשמור metadata של ה-manifest ברישום.
4. **זרימת fetch**:
   - להגיש בקשות range של chunk מהדיסק. ה-scheduler אוכף `max_parallel_fetches` ומחזיר `429` בעת רוויה.
   - להוציא טלמטריה מובנית (Norito JSON) עם latency, bytes שהוגשו וספירות שגיאה לניטור downstream.
5. **דגימות PoR**:
   - ה-worker בוחר manifests ביחס למשקל (למשל bytes מאוחסנים) ומבצע דגימה דטרמיניסטית באמצעות עץ PoR של chunk store.
   - לשמור תוצאות לביקורות ממשל ולכלול תקצירים ב-adverts של ספק / נקודות קצה לטלמטריה.
6. **Eviction / אכיפת קצבות**:
   - כאשר הקיבולת מתמלאת, הצומת דוחה pins חדשים כברירת מחדל. אופציונלית, מפעילים יוכלו להגדיר מדיניות eviction (למשל TTL, LRU) לאחר שהמודל הממשלי יוסכם; כרגע העיצוב מניח קצבות קשיחות ופעולות unpin ביוזמת המפעיל.

### הצהרת קיבולת ואינטגרציית תזמון

- Torii מעבירה כעת עדכוני `CapacityDeclarationRecord` מ-`/v1/sorafs/capacity/declare` אל `CapacityManager` המוטמע, כך שכל צומת בונה תמונת מצב בזיכרון של הקצאות chunker/lane שלו. ה-manager חושף snapshots לקריאה בלבד לטלמטריה (`GET /v1/sorafs/capacity/state`) ואוכף הזמנות לפי פרופיל או lane לפני קבלת הזמנות חדשות.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- נקודת הקצה `/v1/sorafs/capacity/schedule` מקבלת payloads של `ReplicationOrderV1` מהגוב. כאשר ההזמנה מכוונת לספק המקומי, ה-manager בודק כפילויות תזמון, מאמת קיבולת chunker/lane, מזמין את הפרוסה ומחזיר `ReplicationPlan` שמתאר את הקיבולת שנותרה כדי שכלי orchestration יוכלו להמשיך בהטענה. הזמנות לספקים אחרים נענות ב-`ignored` כדי להקל על תהליכי עבודה מרובי מפעילים.【crates/iroha_torii/src/routing.rs:4845】
- Hooks של השלמה (למשל לאחר הצלחת ingest) קוראים ל-`POST /v1/sorafs/capacity/complete` כדי לשחרר הזמנות דרך `CapacityManager::complete_order`. התגובה כוללת snapshot `ReplicationRelease` (סך נותר, שאריות chunker/lane) כדי שכלי orchestration יוכלו לתזמן את ההזמנה הבאה ללא polling. עבודה עתידית תחבר זאת לפייפליין של chunk store כשהלוגיקה של ingest תיכנס.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- ה-`TelemetryAccumulator` המוטמע ניתן לעדכון דרך `NodeHandle::update_telemetry`, מה שמאפשר ל-workers ברקע לרשום דגימות PoR/uptime ולבסוף להפיק payloads קנוניים `CapacityTelemetryV1` בלי לגעת ביישום הפנימי של ה-scheduler.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### אינטגרציות ועבודה עתידית

- **Governance**: להרחיב את `sorafs_pin_registry_tracker.md` עם טלמטריית אחסון (שיעור הצלחת PoR, ניצול דיסק). מדיניות קבלה יכולה לדרוש קיבולת מינימלית או שיעור הצלחה מינימלי של PoR לפני קבלת adverts.
- **SDKs של לקוחות**: לחשוף את קונפיגורציית האחסון החדשה (מגבלות דיסק, alias) כדי שכלי ניהול יוכלו לאתחל צמתים פרוגרמטית.
- **Telemetry**: לשלב עם סטאק המדדים הקיים (Prometheus / OpenTelemetry) כדי שמדדי האחסון יופיעו בדשבורדים.
- **Security**: להריץ את מודול האחסון ב-pool אסינכרוני ייעודי עם back-pressure ולשקול sandboxing של קריאות chunk באמצעות io_uring או tokio pools מוגבלים כדי למנוע ניצול משאבים על ידי לקוחות זדוניים.

העיצוב הזה שומר על מודול האחסון כאופציונלי ודטרמיניסטי תוך מתן שליטה למפעילים להשתתף בשכבת זמינות הנתונים של SoraFS. המימוש יצריך שינויים ב-`iroha_config`, `iroha_core`, `iroha_torii` וב-gateway של Norito, וכן בכלי advert של ספקים.
