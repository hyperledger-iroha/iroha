---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/soranet/privacy-metrics-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ac94afb0b9ca7f9c094134a43ca0d0b284febc90b59824a3ec37a7845d95925b
source_last_modified: "2025-11-14T04:43:22.468269+00:00"
translation_last_reviewed: 2026-01-30
---

:::note מקור קנוני
משקף `docs/source/soranet/privacy_metrics_pipeline.md`. שמרו על שתי הגרסאות מסונכרנות עד שהסט הישן של התיעוד יופסק.
:::

# צינור מדדי פרטיות של SoraNet

SNNet-8 מציג משטח טלמטריה מודע לפרטיות עבור runtime של relay. ה-relay מאגד אירועי handshake ו-circuit לבאקטים של דקה ומייצא רק מוני Prometheus גסים, כך ש-circuits בודדים נשארים בלתי ניתנים לקישור תוך מתן נראות אופרטיבית למפעילים.

## סקירת האגרגטור

- מימוש ה-runtime נמצא ב-`tools/soranet-relay/src/privacy.rs` כ-`PrivacyAggregator`.
- ה-buckets ממופים לדקת שעון קיר (`bucket_secs`, ברירת מחדל 60 שניות) ומאוחסנים בטבעת מוגבלת (`max_completed_buckets`, ברירת מחדל 120). ה-shares של collectors שומרים backlog מוגבל משלהם (`max_share_lag_buckets`, ברירת מחדל 12) כך שנפחי Prio ישנים נשטפים כ-buckets suppressed במקום לדלוף בזיכרון או להסתיר collectors תקועים.
- `RelayConfig::privacy` ממופה ישירות ל-`PrivacyConfig` ומציג knobs לכוונון (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). ה-runtime הייצורי שומר על ברירות המחדל בעוד SNNet-8a מציג ספי אגירה מאובטחת.
- מודולי runtime רושמים אירועים דרך helpers טיפוסיים: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes`, ו-`record_gar_category`.

## נקודת admin של relay

מפעילים יכולים לדגום את מאזין ה-admin של ה-relay לתצפיות גולמיות דרך `GET /privacy/events`. נקודת הקצה מחזירה JSON מופרד שורות (`application/x-ndjson`) המכיל payloads מסוג `SoranetPrivacyEventV1` המשוקפים מה-`PrivacyEventBuffer` הפנימי. ה-buffer שומר את האירועים החדשים ביותר עד `privacy.event_buffer_capacity` רשומות (ברירת מחדל 4096) ומתרוקן בקריאה, לכן scrapers צריכים לפקוד בתדירות מספקת כדי להימנע מפערים. האירועים מכסים את אותם אותות handshake, throttle, verified bandwidth, active circuit ו-GAR שמזינים את מוני Prometheus, ומאפשרים ל-collectors downstream לארכב breadcrumbs בטוחים לפרטיות או להזין workflows של אגירה מאובטחת.

## תצורת relay

מפעילים מכוונים את cadence של טלמטריית הפרטיות בקובץ התצורה של relay דרך סעיף `privacy`:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

ברירות המחדל של השדות תואמות את מפרט SNNet-8 ונבדקות בזמן טעינה:

| שדה | תיאור | ברירת מחדל |
|------|--------|-------------|
| `bucket_secs` | רוחב כל חלון אגירה (שניות). | `60` |
| `min_handshakes` | מינימום תורמים לפני ש-bucket יכול לפלוט מונים. | `12` |
| `flush_delay_buckets` | מספר buckets שהושלמו לפני ניסיון flush. | `1` |
| `force_flush_buckets` | גיל מקסימלי לפני פליטת bucket suppressed. | `6` |
| `max_completed_buckets` | backlog של buckets נשמרים (מונע זיכרון בלתי מוגבל). | `120` |
| `max_share_lag_buckets` | חלון שמירה ל-collector shares לפני suppression. | `12` |
| `expected_shares` | מספר Prio collector shares נדרש לפני שילוב. | `2` |
| `event_buffer_capacity` | backlog אירועי NDJSON עבור זרם ה-admin. | `4096` |

הגדרת `force_flush_buckets` נמוך מ-`flush_delay_buckets`, איפוס הספים או השבתת מנגנון השימור נכשלים כעת באימות כדי למנוע פריסות שידליפו טלמטריה לכל relay.

המגבלה `event_buffer_capacity` מגבילה גם את `/admin/privacy/events`, ומבטיחה ש-scrapers לא יוכלו להתעכב ללא סוף.

## Prio collector shares

SNNet-8a פורס collectors כפולים שמפיקים buckets מסוג Prio עם שיתוף סודות. ה-orchestrator מנתח כעת את זרם ה-NDJSON `/privacy/events` עבור רשומות `SoranetPrivacyEventV1` ו-shares `SoranetPrivacyPrioShareV1`, ומעביר אותן אל `SoranetSecureAggregator::ingest_prio_share`. buckets נפלטים כאשר מגיעות `PrivacyBucketConfig::expected_shares` תרומות, תוך שיקוף התנהגות ה-relay. ה-shares נבדקים ליישור buckets ולצורת histogram לפני שהם משולבים ל-`SoranetPrivacyBucketMetricsV1`. אם מספר ה-handshakes המשולב יורד מתחת ל-`min_contributors`, ה-bucket מיוצא כ-`suppressed`, בהתאם להתנהגות האגרגטור בתוך relay. חלונות suppressed פולטים כעת תווית `suppression_reason` כדי לאפשר למפעילים להבחין בין `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, ו-`forced_flush_window_elapsed` בעת אבחון פערי טלמטריה. הסיבה `collector_window_elapsed` מופעלת גם כאשר Prio shares מתעכבים מעבר ל-`max_share_lag_buckets`, מה שמבליט collectors תקועים בלי להשאיר מצברים ישנים בזיכרון.

## נקודות קליטה של Torii

Torii חושף כעת שתי נקודות HTTP מוגנות טלמטריה כך ש-relays ו-collectors יוכלו להעביר תצפיות בלי לשלב טרנספורט ייעודי:

- `POST /v1/soranet/privacy/event` מקבל payload מסוג `RecordSoranetPrivacyEventDto`. הגוף עוטף `SoranetPrivacyEventV1` יחד עם תווית `source` אופציונלית. Torii מאמת את הבקשה מול פרופיל הטלמטריה הפעיל, רושם את האירוע, ומשיב HTTP `202 Accepted` בצירוף מעטפת Norito JSON הכוללת את חלון החישוב (`bucket_start_unix`, `bucket_duration_secs`) ומצב ה-relay.
- `POST /v1/soranet/privacy/share` מקבל payload מסוג `RecordSoranetPrivacyShareDto`. הגוף נושא `SoranetPrivacyPrioShareV1` ורמז `forwarded_by` אופציונלי כדי לאפשר למפעילים לאמת זרימות collectors. הגשות מוצלחות מחזירות HTTP `202 Accepted` עם מעטפת Norito JSON שמסכמת את ה-collector, חלון ה-bucket ורמז suppression; כשלי אימות ממופים לתשובת טלמטריה `Conversion` כדי לשמר טיפול שגיאות דטרמיניסטי בין collectors. לולאת האירועים של ה-orchestrator פולטת כעת את ה-shares הללו כשהיא מבצעת polling ל-relays, ושומרת על סינכרון מצבר ה-Prio של Torii עם buckets ב-relay.

שתי נקודות הקצה מכבדות את פרופיל הטלמטריה: הן מחזירות `503 Service Unavailable` כאשר המטריקות מושבתות. לקוחות יכולים לשלוח גוף Norito בינארי (`application/x.norito`) או Norito JSON (`application/x.norito+json`); השרת מנהל מו"מ על הפורמט אוטומטית באמצעות ה-extractors הסטנדרטיים של Torii.

## מדדי Prometheus

כל bucket מיוצא נושא תוויות `mode` (`entry`, `middle`, `exit`) ו-`bucket_start`. משפחות המדדים הבאות נפלטות:

| Metric | Description |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | טקסונומיית handshake עם `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | מוני throttle עם `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | משכי cooldown מצטברים שסופקו ע"י handshakes throttled. |
| `soranet_privacy_verified_bytes_total` | רוחב פס מאומת מהוכחות מדידה מעוורות. |
| `soranet_privacy_active_circuits_{avg,max}` | ממוצע ושיא של circuits פעילים לכל bucket. |
| `soranet_privacy_rtt_millis{percentile}` | אומדני אחוזונים של RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | מוני Governance Action Report מגובים hash לפי digest קטגוריה. |
| `soranet_privacy_bucket_suppressed` | buckets שנעצרו משום שסף התורמים לא הושג. |
| `soranet_privacy_pending_collectors{mode}` | מצברי collector shares שממתינים לשילוב, מקובצים לפי מצב relay. |
| `soranet_privacy_suppression_total{reason}` | מוני buckets suppressed עם `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` כך שהדשבורדים יוכלו לייחס פערי פרטיות. |
| `soranet_privacy_snapshot_suppression_ratio` | יחס suppressed/נוקז בסנאפשוט האחרון (0-1), שימושי לתקציבי התראות. |
| `soranet_privacy_last_poll_unixtime` | חותמת זמן UNIX של polling מוצלח אחרון (מזין התראת collector-idle). |
| `soranet_privacy_collector_enabled` | מד שמתהפך ל-`0` כאשר collector הפרטיות מושבת או נכשל בהפעלה (מזין התראת collector-disabled). |
| `soranet_privacy_poll_errors_total{provider}` | כשלי polling מקובצים לפי alias של relay (מוגדל על שגיאות פענוח, כשלי HTTP או קודי סטטוס לא צפויים). |

Buckets ללא תצפיות נשארים שקטים, ומשאירים את הדשבורדים נקיים בלי ליצור חלונות מלאים באפסים.

## הנחיות תפעול

1. **דשבורדים** - תרשים את המדדים לעיל מקובצים לפי `mode` ו-`window_start`. הדגישו חלונות חסרים כדי לחשוף בעיות collector או relay. השתמשו ב-`soranet_privacy_suppression_total{reason}` כדי להבדיל בין חוסר תורמים לבין suppression שמונע על ידי collectors בעת תחקור הפערים. נכס Grafana כולל כעת פאנל ייעודי **"Suppression Reasons (5m)"** שמוזן מאותם מונים יחד עם סטט **"Suppressed Bucket %"** שמחשב `sum(soranet_privacy_bucket_suppressed) / count(...)` לכל בחירה כדי שמפעילים יזהו חריגות תקציב במבט אחד. סדרת **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) והסטט **"Snapshot Suppression Ratio"** מדגישים collectors תקועים וסטיית תקציב בזמן ריצות אוטומטיות.
2. **התראות** - הפעילו אזעקות ממונים בטוחים לפרטיות: קפיצות דחיית PoW, תדירות cooldown, סטיית RTT ודחיות קיבולת. מכיוון שהמונים מונוטוניים בתוך כל bucket, כללי קצב פשוטים עובדים היטב.
3. **תגובת תקרית** - הסתמכו תחילה על נתונים מצטברים. כאשר צריך דיבוג עמוק יותר, בקשו מה-relays לשחזר snapshots של buckets או לבדוק הוכחות מדידה מעוורות במקום לאסוף יומני תעבורה גולמיים.
4. **שימור** - גרדו בתדירות מספקת כדי לא לעבור את `max_completed_buckets`. exporters צריכים להתייחס לפלט Prometheus כמקור הקנוני ולמחוק buckets מקומיים לאחר ההעברה.

## ניתוח suppression וריצות אוטומטיות

קבלת SNNet-8 תלויה בהוכחה ש-collectors אוטומטיים נשארים בריאים וש-suppression נשארת בגבולות המדיניות (≤10% buckets לכל relay בכל חלון של 30 דקות). הכלים הנדרשים לעמידה בשער הזה מסופקים כעת עם העץ; מפעילים חייבים לשלב אותם בטקסים השבועיים שלהם. פאנלי suppression החדשים של Grafana משקפים את קטעי PromQL הבאים, ומעניקים לצוותי כוננות נראות חיה לפני שהם נאלצים להישען על שאילתות ידניות.

### מתכוני PromQL לסקירת suppression

המפעילים צריכים להחזיק את עזרי ה-PromQL הבאים בהישג יד; שניהם מופיעים בדשבורד Grafana המשותף (`dashboards/grafana/soranet_privacy_metrics.json`) ובכללי Alertmanager:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

השתמשו בתוצאת היחס כדי לוודא שהסטט **"Suppressed Bucket %"** נשאר מתחת לתקציב המדיניות; חברו את גלאי הקפיצות ל-Alertmanager לקבלת משוב מהיר כאשר מספר התורמים יורד באופן לא צפוי.

### CLI לדוח buckets אופליין

ה-workspace חושף `cargo xtask soranet-privacy-report` עבור לכידות NDJSON חד-פעמיות. כוונו אותו לייצואי admin של relay אחד או יותר:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

העוזר מזרים את הלכידה דרך `SoranetSecureAggregator`, מדפיס סיכום suppression ל-stdout, ובאופציה כותב דוח JSON מובנה דרך `--json-out <path|->`. הוא מכבד את אותם knobs כמו ה-collector החי (`--bucket-secs`, `--min-contributors`, `--expected-shares`, וכו'), מה שמאפשר למפעילים לשחזר לכידות היסטוריות תחת ספים שונים בעת ניתוח תקלה. צרפו את ה-JSON לצד צילומי Grafana כדי שהשער האנליטי של SNNet-8 יישאר ניתן לביקורת.

### צ'ק-ליסט להרצה אוטומטית ראשונה

הממשל עדיין דורש הוכחה שהריצה האוטומטית הראשונה עמדה בתקציב suppression. העוזר מקבל כעת `--max-suppression-ratio <0-1>` כך ש-CI או המפעילים יכולים להיכשל במהירות כאשר buckets suppressed חורגים מהחלון המותר (ברירת מחדל 10%) או כאשר עדיין אין buckets כלל. תהליך מומלץ:

1. ייצאו NDJSON מנקודות admin של relay יחד עם הזרם `/v1/soranet/privacy/event|share` של ה-orchestrator אל `artifacts/sorafs_privacy/<relay>.ndjson`.
2. הריצו את העוזר עם תקציב המדיניות:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   הפקודה מדפיסה את היחס הנצפה ויוצאת עם קוד שגיאה כאשר התקציב נחצה **או** כאשר עדיין אין buckets מוכנים, כדי לסמן שטלמטריה עדיין לא הופקה להרצה. מדדים חיים צריכים להראות שה-`soranet_privacy_pending_collectors` מתנקזים לכיוון אפס ושה-`soranet_privacy_snapshot_suppression_ratio` נשאר מתחת לאותו תקציב בזמן ההרצה.
3. ארכבו את פלט ה-JSON ואת לוג ה-CLI עם חבילת הראיות של SNNet-8 לפני החלפת ברירת המחדל של הטרנספורט, כדי שמבקרים יוכלו לשחזר את אותם ארטיפקטים.

## צעדים הבאים (SNNet-8a)

- לשלב את ה-collectors הדו-ראשיים של Prio ולחבר את קליטת ה-shares ל-runtime כדי שה-relays וה-collectors יפיקו payloads עקביים של `SoranetPrivacyBucketMetricsV1`. *(בוצע — ראו `ingest_privacy_payload` ב-`crates/sorafs_orchestrator/src/lib.rs` והבדיקות הנלוות.)*
- לפרסם את הדשבורד המשותף של Prometheus ואת כללי ההתראה שמכסים פערי suppression, בריאות collectors והתרעות על ירידת אנונימיות. *(בוצע — ראו `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` ופריטי הוולידציה.)*
- להפיק ארטיפקטים לכיול פרטיות דיפרנציאלית המתוארים ב-`privacy_metrics_dp.md`, כולל notebooks משחזרים ו-digest ממשל. *(בוצע — ה-notebook והארטיפקטים נוצרו על ידי `scripts/telemetry/run_privacy_dp.py`; מעטפת ה-CI `scripts/telemetry/run_privacy_dp_notebook.sh` מריצה את ה-notebook דרך workflow `.github/workflows/release-pipeline.yml`; ה-digest נשמר ב-`docs/source/status/soranet_privacy_dp_digest.md`.)*

הגרסה הנוכחית מספקת את בסיס SNNet-8: טלמטריה דטרמיניסטית ובטוחה לפרטיות שמשתלבת ישירות עם scrapers ודשבורדים קיימים של Prometheus. ארטיפקטי הכיול של פרטיות דיפרנציאלית זמינים, workflow של ה-release pipeline שומר על פלטי ה-notebook עדכניים, והעבודה שנותרה מתמקדת בניטור ההרצה האוטומטית הראשונה ובהרחבת אנליטיקות ההתראה של suppression.
