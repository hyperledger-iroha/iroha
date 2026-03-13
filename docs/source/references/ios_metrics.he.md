<!-- Hebrew translation of docs/source/references/ios_metrics.md -->

---
lang: he
direction: rtl
source: docs/source/references/ios_metrics.md
status: complete
translator: manual
---

<!--
הפניה לרודמפ עבור מדדי Swift dashboard. יש לסנכרן תרגומים:
docs/source/references/ios_metrics.ja.md
docs/source/references/ios_metrics.he.md
-->

<div dir="rtl">

# מדדי דאשבורד Swift (Parity & CI)

מסמך זה מתאר את סכמת ה-JSON והכלים בשימוש עבור דשבורדי Swift SDK. הוא משמש כחוזה עבור יצואני הטלמטריה שיזינו את תצוגות ה-parity וה-CI המוזכרות ברודמפ וב-`status.md`.
קראו אותו יחד עם `docs/source/swift_xcframework_device_matrix.md` (מטריצת הליינים),
מדריך טריאז' הפריטי ו־CI operations כדי להבין כיצד מטא-דאטת `device_tag` וספי ה-SLA
זורמים ב-Buildkite, בדשבורדים ובתהליכי הכוננות.

## פידים

- `dashboards/data/mobile_parity.sample.json`
- `dashboards/data/mobile_ci.sample.json`
- `dashboards/data/swift_schema.sample.json`
- `dashboards/data/swift_salt_status.sample.json`
- `dashboards/data/mobile_pipeline_metadata.sample.json`
- `dashboards/data/swift_schema.sample.json`

קבצי הדוגמה נמצאים בריפו לצורך תיעוד הצורה המצופה. פידים ממשיים צריכים להיווצר ע״י יצואני טלמטריה ולהיות ממוקמים באותו נתיב (הריפו מתעלם מקבצים שאינם `.sample.json`).

## סכמת `mobile_parity`

| שדה | טיפוס | הערות |
|-----|-------|-------|
| `generated_at` | מחרוזת (ISO 8601) | חותמת זמן ליצירת הסנאפשוט. |
| `fixtures` | אובייקט | דיפי parity של Norito וגיל הפריט הוותיק ביותר. |
| `pipeline` | אובייקט | סטטוס אינטגרציה של `/v2/pipeline` (הרצה אחרונה, כשלים). |
| `pipeline.metadata` | אובייקט (אופציונלי) | נתוני טיימינג משותפים של עבודה/בדיקות (`job_name`, ‏`duration_seconds`, ‏`tests[]`) שמוזרמים דרך `MOBILE_PARITY_PIPELINE_METADATA`; השדה `metadata_source` מתעד את הנתיב/ה-URL לצורכי ביקורת. |
| `regen_sla` | אובייקט | רצף עמידה ב-SLA של רענון פיקסצ'רים. |
| `alerts` | מערך | ערכי התראה חופשיים (אופציונלי). |
| `acceleration` | אובייקט (אופציונלי) | נתוני Metal/NEON/StrongBox עבור parity וביצועים (`enabled`, ‏`parity`, ‏`perf_delta_pct`). |
| `telemetry` | אובייקט (אופציונלי) | תקציר מצב הרדקציה (`salt_epoch`, גיל הסיבוב, `overrides_open`, ‏`device_profile_alignment`, ‏`schema_version`, ‏`notes`). |

## סכמת `mobile_ci`

| שדה | טיפוס | הערות |
|-----|-------|-------|
| `generated_at` | מחרוזת (ISO 8601) | חותמת זמן ליצירת הסנאפשוט. |
| `buildkite` | אובייקט | שיעורי הצלחה של ליינים, עומק תורים, זמני ריצה. |
| `devices` | אובייקט | ספירת Pass/Fail לפול אמולטור מול StrongBox. |
| `alert_state` | אובייקט | מוני כשל רציפים ואירועים פתוחים. |
| `acceleration_bench` | אובייקט (אופציונלי) | מדדי Bench המשווים Metal מול CPU או NEON. |

### `buildkite.lanes[]`

| שדה | טיפוס | הערות |
|-----|-------|-------|
| `name` | מחרוזת | מזהה הליין (לדוגמה `ci/xcode-swift-parity`, ‏`ci/xcframework-smoke:iphone-sim`). |
| `success_rate_14_runs` | מספר | שיעור הצלחה ב-14 הריצות האחרונות (0–1). |
| `last_failure` | מחרוזת (ISO 8601) או `null` | חותמת זמן של הכשל האחרון. |
| `flake_count` | מספר שלם | מספר הפלייקים הידועים (קלט ידני). |
| `mttr_hours` | מספר | זמן התאוששות ממוצע (MTTR) בשעות. |
| `device_tag` | מחרוזת (אופציונלי) | תגית קצרה עבור הדשבורד (למשל `iphone-sim`, ‏`strongbox`, ‏`mac-fallback`). |

## סכמת `connect.error`

עטיפת `ConnectError` ב-Swift (ראו `docs/source/connect_error_taxonomy.md`) מספקת את
`telemetryAttributes(fatal:httpStatus:)`, והיצואנים מעבירים את המילון הזה כאירוע OTEL מובנה.
יש לכלול את השדות הבאים:

| שדה | טיפוס | הערות |
|-----|-------|-------|
| `category` | מחרוזת | אחד מהערכים `transport` / `codec` / `authorization` / `timeout` / `queueOverflow` / `internal`. |
| `code` | מחרוזת | מזהה יציב (למשל `client.closed`, ‏`network.timeout`). |
| `fatal` | מחרוזת (`"true"`/`"false"`) | האם השגיאה חיסלה את הסשן. |
| `http_status` | מחרוזת (אופציונלי) | קוד HTTP/WebSocket רלוונטי. |
| `underlying` | מחרוזת (אופציונלי) | תיאור דיבאג של השגיאה המקורית. |
| `sid` | מחרוזת (אופציונלי) | מזהה הסשן (רצוי בגיבוב) כאשר מותר ללוג. |
| `queue_depth` | מספר שלם (אופציונלי) | עומק התור בעת הכשל (לניתוח overflow). |

יש לשגר אירוע אחד עבור כל שגיאה בעת שהיא נחשפת לשכבת האפליקציה, כדי להימנע מרעש.
בלוחות הבקרה מחברים את feed `connect.error` למדדי עומק התור והתחברויות חוזרות כדי לזהות מגמות.

## כלי עזר

- רנדר לשני הדשבורדים: `make swift-dashboards` (מריץ מאמת סכמה ורנדררים ב-Swift).
  הפקודה גם מכבדת משתני סביבה אופציונליים `SWIFT_PARITY_FEED` / `SWIFT_CI_FEED`, כך שניתן להצביע
  על פידים חיים במקום קבצי הדוגמה ועדיין להשתמש באותו תהליך אימות/רנדר.
- נקודת כניסה ל-CI: ‏`ci/check_swift_dashboards.sh`.
- רנדר ידני: ‏`scripts/render_swift_dashboards.sh [/path/to/parity.json [/path/to/ci.json]]`.
- מאמת סכמה: ‏`scripts/check_swift_dashboard_data.py <files…>`.
- העשרת טלמטריה: ‏`scripts/swift_enrich_parity_feed.py --input parity.json --salt-epoch …` מוסיף ל-feed את נתוני הסולט/override
  (מוזן דרך משתני סביבה ב-`ci/swift_status_export.sh`) כדי שהדשבורד ויצוא הסטטוס יכללו את בלוק ה-telemetry.
- איסוף טלמטריה: ‏`scripts/swift_collect_redaction_status.py --salt-config dashboards/data/swift_salt_status.sample.json --overrides-store artifacts/swift_telemetry_overrides.json`
  בונה באופן אוטומטי את אובייקט ה-`telemetry` מתוך נתוני הסולט וה-overrides.
- CLI לניהול overrides: ‏`python3 scripts/swift_status_export.py telemetry-override {list,create,revoke}`
  (או `scripts/swift_telemetry_override.py` הוותיק) שומר את ה-ledger
  (`artifacts/swift_telemetry_overrides.json`) עבור בעלי התפקידים (support/SRE) והסיבות שנכנסות לפיד.
- סכמה ב-JSON: ‏`docs/source/references/ios_metrics.schema.json` (נבדק באמצעות `python3 -m jsonschema …` בתוך `make swift-dashboards`).
- רפרנס לכיסוי מכשירים: ‏`docs/source/swift_xcframework_device_matrix.md` מגדיר את מטריצת הליינים של Buildkite.

## יצוא Prometheus (`swift_parity_success_total`)

- ‏`swift_status_export.py` יכול להפיק קובץ טקסט בפורמט Prometheus על ידי `--metrics-path /path/to/swift_status.prom`.
  היצואן שומר מונים `swift_parity_success_total` ו-`swift_parity_failure_total` באמצעות קובץ מצב JSON (ברירת מחדל
  `artifacts/swift_status_metrics_state.json`, ניתן לעדכן עם `--metrics-state …`).
- ‏`swift_parity_success_total` יוגדל כאשר בסנאפשוט האחרון אין פערים פתוחים, `regen_sla.breach` שקרי,
  ואין בדיקות `/v2/pipeline` שנכשלו.
- הקובץ כולל גם מדדי gauge עבור מצב עכשווי (`swift_parity_status`), מספר הפערים, גיל הפער הוותיק ביותר ושעות מאז רענון הפיקסלים האחרון,
  כך שאוספי textfile ולוחות הבקרה של המשמרת יוכלו להתריע על סטייה מה-SLA.
- כאשר בלוק ה-telemetry קיים היצואן מפיק גם:
  - `swift_telemetry_overrides_open` – ‏gauge שמדווח על מספר החריגות/overrides הפעילות בסנאפשוט.
  - `swift_telemetry_salt_rotation_age_hours` – ‏gauge שמודד כמה שעות עברו מאז סיבוב הסולט האחרון.

## הנחיות ליצואנים

1. להפיק JSON חוקי ב-UTF-8 שתואם את הסכמה לעיל.
2. לדלג על בלוקים אופציונליים (`acceleration`,‏ `acceleration_bench`) כאשר אין נתונים.
3. לשמור על חותמות זמן ב-UTC (ISO 8601 עם סיומת `Z`).
4. לכלול מדדי ביצועים כערכים מספריים גולמיים (למשל מילישניות או MB/s) כדי שהרנדררים ב-Swift יוכלו להפיק ייצוגים קריאים.
5. להעלות את הפידים למיקום מאובטח (S3, ארטיפקט build וכו') ולסנכרן אותם ל-`dashboards/data/` במהלך עבודות הדיווח. הריפו מתעלם מקבצים שאינם sample ולכן האחריות לניהול השמירה על היצואן.
6. בהרצה תחת CI, יש לרשום גם מטא-דאטה של Buildkite `ci/xcframework-smoke:<lane>:device_tag` כדי שכלי הדשבורד ידעו לשייך טלמטריה למסלולי המכשירים בלי לנתח מחרוזות destination.

## עבודה עתידית

- לחבר את היצואנים ל-Buildkite לשם פרסום פידים חיים.
- להרחיב את הדשבורדים בגרפים היסטוריים (spark-lines) כשנתוני סדרות-זמן יהיו זמינים.
- להוסיף בדיקות parity אוטומטיות המשוות בין Metal ל-CPU עבור Merkle/CRC64.

</div>
