---
lang: he
direction: rtl
source: docs/amx.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 68b349701a8c9ac7f0a22fc46f1af38d06011e63283bcd2431d9b707cb45a392
source_last_modified: "2025-12-12T15:13:07.309952+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/amx.md -->

# מדריך ביצוע ותפעול AMX

**סטטוס:** טיוטה (NX-17)  
**קהל יעד:** מהנדסי פרוטוקול ליבה, AMX/קונצנזוס, SRE/טלמטריה, צוותי SDK ו-Torii  
**הקשר:** משלים את פריט המפה "Documentation (owner: Docs) — update `docs/amx.md` with timing diagrams, error catalog, operator expectations, and developer guidance for generating/using PVOs."【roadmap.md:2497】

## סיכום

טרנזקציות AMX אטומיות בין dataspaces מאפשרות להגשה אחת לגעת במספר מרחבי נתונים
(DS) תוך שמירה על סופיות של 1 שניה לסלוט, קודי כשל דטרמיניסטיים וסודיות עבור
פרגמנטים פרטיים. מדריך זה מסכם את מודל התזמון, טיפול שגיאות קנוני, דרישות ראיות
לאופרטורים וציפיות למפתחים עבור Proof Verification Objects (PVOs) כך שהמסירה
של הרודמאפ תהיה עצמאית גם מחוץ למסמך העיצוב `docs/source/nexus.md`.

הבטחות מרכזיות:

- כל הגשת AMX מקבלת תקציבי prepare/commit דטרמיניסטיים; חריגות מסתיימות בקודי
  כשל מתועדים במקום להיתקע בליין.
- דגימות DA שחורגות מהתקציב נרשמות כראיות זמינות חסרות והטרנזקציה נשארת בתור
  לסלוט הבא במקום לעצור את התפוקה.
- PVOs מנתקים הוכחות כבדות מהסלוט של 1 שניה בכך שהם מאפשרים לקליינטים/בצ’רים
  לרשום ארטיפקטים מראש שהמארח מאמת במהירות בזמן הסלוט.
- מארחי IVM גוזרים מדיניות AXT לכל dataspace על בסיס Space Directory: handles
  חייבים לכוון ל-lane המפורסם בקטלוג, להציג את root ה-manifest האחרון, לעמוד
  ב-`expiry_slot`, `handle_era` ו-`sub_nonce` מינימליים, ולדחות dataspaces לא
  מוכרים עם `PermissionDenied` לפני ביצוע.
- פקיעת סלוטים משתמשת ב-`nexus.axt.slot_length_ms` (ברירת מחדל `1` מ״ש, מאומת
  בין `1` ל-`600_000` מ״ש) וב-`nexus.axt.max_clock_skew_ms` המוגבל (ברירת מחדל
  `0` מ״ש, מוגבל ע״י אורך הסלוט ו-`60_000` מ״ש). מארחים מחשבים
  `current_slot = block.creation_time_ms / slot_length_ms`, מחילים את ההטיה על
  בדיקות פקיעה/הוכחה, ודוחים handles שמצהירים על הטיה גדולה מהגבול המוגדר.
- TTL של מטמון הוכחות מגביל שימוש חוזר: `nexus.axt.proof_cache_ttl_slots`
  (ברירת מחדל `1`, מאומת `1`–`64`) מגביל כמה זמן הוכחות מאושרות/דחויות נשמרות
  במטמון; הרשומות נמחקות כשה-TTL או `expiry_slot` פוקעים כדי למנוע replay.
- שמירת replay ledger: `nexus.axt.replay_retention_slots` (ברירת מחדל `128`,
  מאומת `1`–`4_096`) קובע חלון מינימום של היסטוריית שימוש ב-handle עבור דחיית
  replay בין peers/restarts; התאימו לחלון התוקף הארוך ביותר שאתם מנפיקים.
  ה-ledger נשמר ב-WSV, נטען באתחול, ונגזר דטרמיניסטית כאשר חלון השמירה ופקיעת
  ה-handle חלפו (המאוחר ביניהם) כדי שמעבר peers לא יפתח חורי replay.
- דיבוג סטטוס מטמון: Torii חושף `/v1/debug/axt/cache` (gate של telemetry/dev)
  שמחזיר גרסת snapshot של מדיניות AXT, הדחייה האחרונה (lane/סיבה/גרסה), הוכחות
  במטמון (dataspace/סטטוס/manifest root/slots), ורמזי דחייה
  (`next_min_handle_era`/`next_min_sub_nonce`). השתמשו בו כדי לוודא שסבבי slot/
  manifest משתקפים במטמון ולהחליף handles דטרמיניסטית בזמן troubleshooting.

## מודל תזמון סלוטים

### ציר זמן

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- התקציבים מיושרים לתוכנית הלדג'ר: mempool 70 מ״ש, DA commit ≤300 מ״ש, קונצנזוס
  300 מ״ש, IVM/AMX 250 מ״ש, settlement 40 מ״ש, guard 40 מ״ש.【roadmap.md:2529】
- טרנזקציות שחורגות מחלון DA נרשמות כראיות זמינות חסרות ומנסות שוב בסלוט הבא;
  חריגות אחרות מסומנות בקודים כגון `AMX_TIMEOUT` או `SETTLEMENT_ROUTER_UNAVAILABLE`.
- חתך guard סופג יצוא טלמטריה וביקורת סופית כך שהסלוט נסגר ב-1 שניה גם אם
  ה-exporters מאחרים מעט.
- טיפים לתצורה: ברירות מחדל שומרות על תוקף קשיח (`slot_length_ms = 1`,
  `max_clock_skew_ms = 0`). לקדנציה של 1 שניה הגדירו `slot_length_ms = 1_000`
  ו-`max_clock_skew_ms = 250`; לקדנציה של 2 שניות השתמשו ב-`2_000` ו-`500`.
  ערכים מחוץ לחלון המאומת (`1`–`600_000` מ״ש או `max_clock_skew_ms` גדול מאורך
  הסלוט/`60_000` מ״ש) נדחים בזמן parse, והטיית handle מוצהרת חייבת להישאר בגבול.

### תרשים swim lane בין DS

```text
Client        DS A (public)        DS B (private)        Nexus Lane        Settlement
  │ submit tx │                     │                     │                 │
  │──────────▶│ prepare fragment    │                     │                 │
  │           │ proof + DA part     │ prepare fragment    │                 │
  │           │───────────────┬────▶│ proof + DA part     │                 │
  │           │               │     │─────────────┬──────▶│ Merge proofs    │
  │           │               │     │             │       │ verify PVO/DA   │
  │           │               │     │             │       │────────┬────────▶ apply
  │◀──────────│ result + code │◀────│ result + code │◀────│ outcome│          receipt
```

כל פרגמנט DS חייב לסיים את חלון ה-prepare של 30 מ״ש לפני שה-lane מרכיב את הסלוט.
הוכחות חסרות נשארות ב-mempool לסלוט הבא במקום לחסום peers.

### רשימת בדיקות טלמטריה

| מדד / Trace | מקור | SLO / Alert | הערות |
|----------------|--------|-------------|-------|
| `iroha_slot_duration_ms` (histogram) / `iroha_slot_duration_ms_latest` (gauge) | `iroha_telemetry` | p95 ≤ 1000 ms | שער CI מתואר ב-`ans3.md`. |
| `iroha_da_quorum_ratio` | `iroha_telemetry` (commit hook) | ≥0.95 בחלון 30 דקות | נגזר מטלמטריה של missing-availability כך שכל בלוק מעדכן את ה-gauge (`crates/iroha_core/src/telemetry.rs:3524`,`crates/iroha_core/src/telemetry.rs:4558`). |
| `iroha_amx_prepare_ms` | מארח IVM | p95 ≤ 30 ms לכל DS | מניע ביטולי `AMX_TIMEOUT`. |
| `iroha_amx_commit_ms` | מארח IVM | p95 ≤ 40 ms לכל DS | מכסה מיזוג דלתא + הרצת טריגרים. |
| `iroha_ivm_exec_ms` | מארח IVM | התרעה אם >250 ms לכל lane | תואם לחלון הרצת ה-IVM overlay. |
| `iroha_amx_abort_total{stage}` | Executor | התרעה אם >0.05 ביטולים/סלוט או קפיצות חדות | תוויות שלב: `prepare`, `exec`, `commit`. |
| `iroha_amx_lock_conflicts_total` | מתזמן AMX | התרעה אם >0.1 קונפליקטים/סלוט | מצביע על R/W sets לא מדויקים. |
| `iroha_axt_policy_reject_total{lane,reason}` | מארח IVM | לעקוב אחרי קפיצות | מבדיל בין דחיות manifest/lane/era/sub_nonce/expiry. |
| `iroha_axt_policy_snapshot_cache_events_total{event}` | מארח IVM | צפוי cache_miss רק בהפעלה/שינוי manifest | החמצות רצופות מצביעות על hydration מיושן. |
| `iroha_axt_proof_cache_events_total{event}` | מארח IVM | צפוי בעיקר `hit`/`miss` | קפיצות `reject`/`expired` לרוב מצביעות על drift של manifest או הוכחות מיושנות. |
| `iroha_axt_proof_cache_state{dsid,status,manifest_root_hex,verified_slot}` | מארח IVM | בדיקת הוכחות במטמון | ערך ה-gauge הוא expiry_slot (עם הטיה) עבור ההוכחה. |
| ראיות זמינות חסרות (`sumeragi_da_gate_block_total{reason="missing_local_data"}`) | טלמטריית lane | התרעה אם >5% ל-DS | מצביע על עיכוב attesters או הוכחות. |

`/v1/debug/axt/cache` משקף את gauge `iroha_axt_proof_cache_state` עם snapshot לפי
 dataspace (סטטוס, manifest root, verified/expiry slots) עבור אופרטורים.

`iroha_amx_commit_ms` ו-`iroha_ivm_exec_ms` משתמשים באותם buckets של latency כמו
`iroha_amx_prepare_ms`. מונה הביטולים מתייג כל דחייה עם מזהה lane ושלב
(`prepare` = בניית/ולידציית overlay, `exec` = הרצת chunk ב-IVM,
`commit` = מיזוג דלתא + replay טריגרים) כדי שהטלמטריה תדגיש אם עומס נובע מחוסר
תאימות בקריאות/כתיבות או ממיזוג מצב סופי.

האופרטורים חייבים לארכב את המדדים הללו לצורכי ביקורת לצד ראיות קבלת סלוט,
ולציין רגרסיות ב-`status.md`.

### fixtures זהב ל-AXT

fixtures של Norito עבור descriptor/handle/policy snapshot נמצאים ב-
`crates/iroha_data_model/tests/fixtures/axt_golden.rs`, עם כלי יצירה ב-
`crates/iroha_data_model/tests/axt_policy_vectors.rs` (`print_golden_vectors`).
CoreHost משתמש באותם fixtures בבדיקה `core_host_enforces_fixture_snapshot_fields`
(`crates/ivm/tests/core_host_policy.rs`) כדי לבדוק lane binding, התאמת manifest
root, עדכניות expiry_slot, מינימום handle_era/sub_nonce, ודחיית dataspace חסר.
- fixture JSON רב-dataspace (`crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json`) מקבע את סכמת descriptor/touch, בייטים קנוניים של Norito וקישור Poseidon (`compute_descriptor_binding`). בדיקת `axt_descriptor_fixture` מגנה על הבייטים המקודדים, ו-SDKs יכולים להשתמש ב-`AxtDescriptorBuilder::builder` יחד עם `TouchManifest::from_read_write` כדי להרכיב דוגמאות דטרמיניסטיות עבור docs/SDKs.

### מיפוי קטלוג lanes ומניפסטים

- snapshots של מדיניות AXT נבנים ממערך manifests של Space Directory ומקטלוג lanes.
  כל dataspace ממופה ל-lane המוגדר; manifests פעילים תורמים את hash ה-manifest,
  epoch הפעלה (`min_handle_era`) ורצפת sub-nonce. קישורי UAID ללא manifest פעיל
  עדיין מפיקים entry של מדיניות עם manifest root מאופס כך ש-gating של lane נשאר
  פעיל עד שמניפסט אמיתי עולה.
- `current_slot` ב-snapshot נגזר מחותמת הזמן של הבלוק האחרון שהתחייב
  (`creation_time_ms / slot_length_ms`), עם fallback לגובה בלוק לפני שיש header
  מחויב.
- טלמטריה חושפת את ה-snapshot המוטען כ-`iroha_axt_policy_snapshot_version`
  (64 הביטים הנמוכים של hash ה-Norito), ואירועי מטמון דרך
  `iroha_axt_policy_snapshot_cache_events_total{event=cache_hit|cache_miss}`.
  מוני דחייה משתמשים בתוויות `lane`, `manifest`, `era`, `sub_nonce`, ו-`expiry`
  כדי שאופרטורים יזהו מיד איזה שדה חסם handle.

### רשימת בדיקות לקומפוזביליות בין dataspaces

- ודאו שכל dataspace הרשום ב-Space Directory כולל lane ומניפסט פעיל; סבבי
  רוטציה צריכים לרענן bindings ו-manifest roots לפני הנפקת handles חדשים.
  שורשים מאופסים משמעם שה-handles יידחו עד שיהיו manifests.
- באתחול ואחרי שינויי Space Directory, צפו ל-`cache_miss` אחד ולאחריו `cache_hit`
  רציף במדד snapshot; קצב החמצות גבוה מצביע על מקור manifests מיושן או חסר.
- כאשר handle נדחה, בדקו את `iroha_axt_policy_reject_total{lane,reason}` ואת גרסת
  ה-snapshot כדי להחליט אם לדרוש handle חדש (`expiry`/`era`/`sub_nonce`) או לתקן
  binding של lane/manifest (`lane`/`manifest`). נקודת debug של Torii
  `/v1/debug/axt/cache` מחזירה `reject_hints` עם `dataspace`, `target_lane`,
  `next_min_handle_era`, ו-`next_min_sub_nonce` כדי לרענן handles דטרמיניסטית אחרי
  עדכון מדיניות.

### דוגמת SDK: הוצאה מרחוק ללא יציאת טוקן

1. בנו descriptor של AXT שמפרט את ה-dataspace שמחזיק את הנכס ואת נגיעות הקריאה/
   כתיבה הנדרשות מקומית; שמרו על דטרמיניזם כדי שה-hash של ה-binding יישאר יציב.
2. קראו `AXT_TOUCH` עבור ה-dataspace המרוחק עם ה-manifest הצפוי; אפשר לצרף הוכחה
   עם `AXT_VERIFY_DS_PROOF` אם המארח דורש זאת.
3. בקשו או רעננו handle לנכס והפעילו `AXT_USE_ASSET_HANDLE` עם
   `RemoteSpendIntent` שמוציא בתוך ה-dataspace המרוחק (ללא bridge). האכיפה
   משתמשת ב-`remaining`, `per_use`, `sub_nonce`, `handle_era`, ו-`expiry_slot`
   בהתאם ל-snapshot לעיל.
4. בצעו commit עם `AXT_COMMIT`; אם המארח מחזיר `PermissionDenied`, השתמשו בתווית
   הדחייה כדי להחליט אם לבקש handle חדש (expiry/sub_nonce/era) או לתקן binding
   של manifest/lane.

## ציפיות אופרטורים

1. **מוכנות לפני סלוט**
   - ודאו שבריכות attesters של DA לפי פרופיל (A=12, B=9, C=7) בריאות; churn
     של attesters מתועד ב-snapshot של Space Directory עבור הסלוט.
   - אמתו ש-`iroha_amx_prepare_ms` נמוך מהתקציב ברצים מייצגים לפני הפעלת עומסים חדשים.

2. **ניטור תוך סלוט**
   - התריעו על spikes של missing-availability (>5% לשני סלוטים רצופים) ועל
     `AMX_TIMEOUT`, כי שניהם מצביעים על חריגות תקציב.
   - עקבו אחרי שימוש במטמון PVO (`iroha_pvo_cache_hit_ratio`, מיוצא משירות ההוכחות)
     כדי להוכיח שהוולידציה מחוץ למסלול עומדת בקצב ההגשות.

3. **איסוף ראיות**
   - צרפו קבוצות קבלות DA, היסטוגרמות AMX prepare ודוחות מטמון PVO לחבילת
     הארטיפקטים הלילית המופנית מ-`status.md`.
   - רשמו תוצאות תרגילי כאוס ב-`ops/drill-log.md` בכל פעם שמריצים jitter של DA,
     תקיעות אורקל או בדיקות ריקון באפר.

4. **תחזוקת Runbook**
   - עדכנו את runbooks של Android/Swift SDK בכל פעם שקודי שגיאה או overrides של
     AMX משתנים כדי שהצוותים הקליינטים יירשו את הסמנטיקה הדטרמיניסטית.
   - שמרו על קטעי תצורה (כגון `iroha_config.amx.*`) מסונכרנים עם הפרמטרים
     הקנוניים ב-`docs/source/nexus.md`.

## טלמטריה ופתרון תקלות

### תקציר טלמטריה

| מקור | מה ללכוד | פקודה/נתיב | ציפיות ראיות |
|--------|--------------|----------------|-----------------------|
| Prometheus (`iroha_telemetry`) | SLOs של סלוט ו-AMX: `iroha_slot_duration_ms`, `iroha_amx_prepare_ms`, `iroha_amx_commit_ms`, `iroha_da_quorum_ratio`, `iroha_amx_abort_total{stage}` | לסקרייפ `https://$TORII/telemetry/metrics` או לייצא מהדשבורדים ב-`docs/source/telemetry.md`. | לצרף snapshots של היסטוגרמות (והיסטוריית alerts אם הופעלו) לחבילת `status.md` הלילית. |
| Torii RBC snapshots | backlog של DA/RBC: backlog פר-סשן, מטא-דטה של view/height, ומוני זמינות (`sumeragi_da_gate_block_total{reason="missing_local_data"}`; `sumeragi_rbc_da_reschedule_total` הוא legacy). | `GET /v1/sumeragi/rbc` ו-`GET /v1/sumeragi/rbc/sessions` (ראו `docs/source/samples/sumeragi_rbc_status.md`). | לשמור את JSON עם חותמות זמן כאשר מתריעות התראות AMX DA, לצרף לחבילת אירוע. |
| מדדי שירות הוכחות | בריאות מטמון PVO: `iroha_pvo_cache_hit_ratio`, מוני מילוי/פינוי, עומק תור הוכחות | `GET /metrics` בשירות ההוכחות (`IROHA_PVO_METRICS_URL`) או דרך OTLP משותף. | לייצא hit ratio ועומק תור לצד מדדי הסלוט. |
| Acceptance harness | עומס מעורב (slot/DA/RBC/PVO) תחת jitter מבוקר | להריץ מחדש `ci/acceptance/slot_1s.yml` (או job זהה ב-CI) ולארכב ב-`artifacts/acceptance/slot_1s/<timestamp>/`. | נדרש לפני GA וכל פעם ששינויי pacemaker/DA מיושמים; לצרף סיכום YAML ו-snapshots של Prometheus. |

### Playbook לפתרון תקלות

| סימפטום | בדיקה ראשונה | תיקון מומלץ |
|---------|---------------|---------------------|
| `iroha_slot_duration_ms` p95 עולה מעל 1 000 ms | יצוא Prometheus מ-`/telemetry/metrics` וסנאפשוט `/v1/sumeragi/rbc` כדי לאשר דחיות DA; להשוות מול הארטיפקט האחרון של `ci/acceptance/slot_1s.yml`. | להקטין batch של AMX או להפעיל אגרגטורים נוספים (`sumeragi.collectors.k`), ואז להריץ מחדש את acceptance harness. |
| Spike של missing availability | שדות backlog ב-`/v1/sumeragi/rbc/sessions` לצד דשבורדי בריאות attesters. | להסיר attesters לא בריאים, להגדיל זמנית את `redundant_send_r` להאצת delivery, ולפרסם את ההערות ב-`status.md`. |
| `PVO_MISSING_OR_EXPIRED` חוזר בקבלות | מדדי מטמון שירות הוכחות + לוגי ה-scheduler. | לחדש PVOs מיושנים, לקצר קדנציית רוטציה, ולהבטיח שכל SDK מרענן handle לפני `expiry_slot`. לצרף מדדי שירות הוכחות לחבילת הראיות. |
| `AMX_LOCK_CONFLICT` או `AMX_TIMEOUT` תכופים | `iroha_amx_lock_conflicts_total`, `iroha_amx_prepare_ms`, ו-manifests מושפעים. | להריץ מחדש את Norito static analyzer, לתקן selectors של read/write (או לפצל batch), ולפרסם fixtures מעודכנים. |
| התראות `SETTLEMENT_ROUTER_UNAVAILABLE` | לוגי Settlement router (`docs/settlement-router.md`), דשבורדי buffer treasury והקבלות. | לטעון מאגרי XOR או להעביר את lane ל-XOR בלבד, לתעד את פעולת ה-treasury, ולהריץ מחדש את מבחן הסלוט כדי להוכיח התאוששות. |

### אותות דחייה של AXT

- קודי הסיבה נשמרים כ-`AxtRejectReason` (`lane`, `manifest`, `era`, `sub_nonce`,
  `expiry`, `missing_policy`, `policy_denied`, `proof`, `budget`, `replay_cache`,
  `descriptor`, `duplicate`). אימות בלוקים מציג
  `AxtEnvelopeValidationFailed { message, reason, snapshot_version }`, כך שאירועים
  יכולים לקשור את הדחייה ל-snapshot מדיניות ספציפי.
- `/v1/debug/axt/cache` מחזיר `{ policy_snapshot_version, last_reject, cache, hints }`,
  כאשר `last_reject` כולל lane/סיבה/גרסה של הדחייה האחרונה ו-`hints` מספק
  `next_min_handle_era`/`next_min_sub_nonce` לצד מצב ההוכחות במטמון.
- תבנית alert: להתריע כאשר `iroha_axt_policy_reject_total{reason="manifest"}` או
  `{reason="expiry"}` מזנקים בחלון של 5 דקות, לצרף snapshot של `last_reject` +
  `policy_snapshot_version` מה-endpoint של Torii, ולהשתמש ברמזים כדי לבקש handles
  חדשים לפני ניסיון חוזר.

## Proof Verification Objects (PVOs)

### מבנה

PVOs הם מעטפות בקידוד Norito שמאפשרות ללקוחות להוכיח עבודה כבדה מראש. השדות
הקנוניים:

| שדה | תיאור |
|-------|-------------|
| `circuit_id` | מזהה סטטי למערכת הוכחות/טענה (למשל `amx.transfer.v1`). |
| `vk_hash` | hash מסוג Blake2b-256 למפתח אימות המוזכר ב-manifest של ה-DS. |
| `proof_digest` | digest מסוג Poseidon של מטען ההוכחה הסריאלי המאוחסן ברג'יסטר ה-PVO. |
| `max_k` | גבול עליון ל-AIR domain; מארחים דוחים הוכחות שחורגות מהגודל המוצהר. |
| `expiry_slot` | גובה סלוט שאחריו הארטיפקט לא תקף; מונע הוכחות ישנות. |
| `profile` | רמז אופציונלי (למשל פרופיל DS A/B/C) כדי לסייע לבצ’רים לאגד הוכחות עם פרופיל משותף. |

סכמת Norito נמצאת לצד הגדרות הנתונים ב-`crates/iroha_data_model/src/nexus` כך
ש-SDKs יכולים לגזור אותה ללא serde.

### צינור יצירה

1. **לקמפל מטא-דטה של circuit** — להוציא `circuit_id`, מפתח אימות, וגודל תחום
   מקסימלי מה-prover build (לרוב דרך דוחות `fastpq_prover`).
2. **לייצר ארטיפקטים של הוכחה** — להריץ prover מחוץ לסלוט ולאחסן transcripts מלאים
   והתחייבויות.
3. **לרשום בשירות ההוכחות** — לשלוח את מעטפת ה-PVO ב-Norito ל-verifier מחוץ
   לסלוט (ראו NX-17). השירות מאמת פעם אחת, מקבע את ה-digest, וחושף handle דרך Torii.
4. **להפנות בטרנזקציות** — לצרף את handle ה-PVO ל-builders של AMX (`amx_touch` או
   helpers ברמת SDK). מארחים בודקים את ה-digest, מאמתים את התוצאה מהמטמון, ורק
   אם המטמון קר מבצעים אימות בסלוט.
5. **לסובב לפי תוקף** — SDKs חייבים לרענן handles לפני `expiry_slot`. ארטיפקטים
   שפג תוקפם גורמים ל-`PVO_MISSING_OR_EXPIRED`.

### רשימת בדיקות למפתחים

- להצהיר על read/write sets במדויק כדי ש-AMX יוכל להקדים locks ולהימנע מ-`AMX_LOCK_CONFLICT`.
- לצרף הוכחות allowance דטרמיניסטיות באותו עדכון UAID כאשר העברות בין DS נוגעות
  ל-DS מוסדרים.
- אסטרטגיית retry: missing availability evidence → ללא פעולה (הטרנזקציה נשארת
  ב-mempool); `AMX_TIMEOUT` או `PVO_MISSING_OR_EXPIRED` → לבנות ארטיפקטים מחדש
  ולהשתמש ב-backoff אקספוננציאלי.
- בדיקות צריכות לכלול cache hits ו-cold starts (כדי לאלץ את המארח לאמת את
  ההוכחה עם אותו `max_k`) כדי לשמור על דטרמיניזם.
- Proof blobs (`ProofBlob`) **חייבים** לקודד `AxtProofEnvelope { dsid, manifest_root,
  da_commitment?, proof }`; מארחים מקשרים הוכחות ל-manifest root מה-Space Directory
  ושומרים תוצאות pass/fail במטמון לפי dataspace/slot עם
  `iroha_axt_proof_cache_events_total{event="hit|miss|expired|reject|cleared"}`.
  ארטיפקטים שפגו או שאינם תואמי manifest נדחים לפני commit, וניסיונות חוזרים
  באותו סלוט מקוצרים על בסיס `reject` במטמון.
- שימוש חוזר במטמון הוכחות הוא ברמת סלוט: הוכחות מאומתות נשארות חמות בכל אותו
  סלוט ומוסרות אוטומטית עם התקדמות הסלוט כדי לשמור על דטרמיניזם.

### מנתח read/write סטטי

Selectors קומפילטיים חייבים להתאים להתנהגות החוזה לפני ש-AMX יכול להכין locks או
ליישם manifests של UAID. המודול החדש `ivm::analysis` (`crates/ivm/src/analysis.rs`)
מספק `analyze_program(&[u8])` שמפענח ארטיפקט `.to`, סופר קריאות/כתיבות של רגיסטרים,
פעולות זיכרון ושימוש ב-syscalls, ומייצר דוח ידידותי ל-JSON שה-SDK manifests יכולים
להטמיע. הריצו אותו לצד `koto_lint` כאשר מפרסמים UAIDs כך שסיכום ה-R/W ייכנס
לחבילת הראיות המופנית בביקורות NX-17.

## אכיפת מדיניות Space Directory

אימות handle של AXT נשען כברירת מחדל על snapshot של Space Directory כאשר למארח
יש גישה אליו (CoreHost בבדיקות, WsvHost בזרימות אינטגרציה). רשומות מדיניות
לכל dataspace כוללות `manifest_root`, `target_lane`, `min_handle_era`,
`min_sub_nonce`, ו-`current_slot`. המארחים אוכפים:

- lane binding: `target_lane` של handle חייב להתאים לערך ב-Space Directory;
- manifest binding: ערכי `manifest_root` שאינם אפס חייבים להתאים ל-`manifest_view_root` של handle;
- epoch gating: `handle_era` חייב להיות ≥ `min_handle_era` וכל `sub_nonce` חייב להיות ≥ `min_sub_nonce`;
- slot expiry: `expiry_slot` חייב להיות ≥ `current_slot` (עם allowance של clock skew);
- policy gating: handles עם `policy` לא תואם ל-`AxtPolicyKind` נדחים;
- proof gating: כאשר יש manifest שמחייב הוכחה, handles ללא הוכחה נדחים;
- replay gating: שימוש חוזר באותו handle בתוך חלון השמירה נדחה.

מארחים מחזירים `PermissionDenied` כאשר כל אחד מהתנאים האלו נכשל, והדחייה נשמרת
עם `AxtRejectReason` המתאים כדי שהקליינטים יוכלו להציג תקלות דטרמיניסטיות.

</div>
