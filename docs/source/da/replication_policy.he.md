---
lang: he
direction: rtl
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T15:38:30.661849+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# מדיניות שכפול זמינות נתונים (DA-4)

_סטטוס: בתהליך — בעלים: Core Protocol WG / Storage Team / SRE_

צינור הבליעה של DA אוכף כעת יעדי שמירה דטרמיניסטיים עבור
כל מחלקה כתם המתואר ב-`roadmap.md` (זרם עבודה DA-4). Torii מסרב
מתמשכות מעטפות שמירה המסופקות על ידי המתקשר שאינן תואמות את המוגדר
מדיניות, המבטיחה שכל אימות/צומת אחסון שומר על הנדרש
מספר תקופות והעתקים מבלי להסתמך על כוונת המגיש.

## מדיניות ברירת מחדל

| כיתת בלוב | שימור חם | שימור קור | העתקים נדרשים | כיתת אחסון | תג ממשל |
|------------|---------------|----------------|------------------|----------------|----------------|
| `taikai_segment` | 24 שעות | 14 ימים | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 שעות | 7 ימים | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 שעות | 180 ימים | 3 | `cold` | `da.governance` |
| _ברירת מחדל (כל המחלקות האחרות)_ | 6 שעות | 30 ימים | 3 | `warm` | `da.default` |

ערכים אלה מוטמעים ב-`torii.da_ingest.replication_policy` ומוחלים על
כל ההגשות של `/v1/da/ingest`. Torii משכתב מניפסטים עם האכיפה
פרופיל שימור ופולט אזהרה כאשר מתקשרים מספקים ערכים לא תואמים כך
אופרטורים יכולים לזהות ערכות SDK מיושנות.

### שיעורי זמינות של טאיקאי

מניפסטים של ניתוב Taikai (מטא נתונים `taikai.trm`) כוללים כעת
רמז `availability_class` (`Hot`, `Warm`, או `Cold`). כאשר קיים, Torii
בוחר את פרופיל השמירה התואם מ-`torii.da_ingest.replication_policy`
לפני חלוקת המטען, מה שמאפשר למפעילי אירועים לשדרג לאחור לא פעיל
ביצועים מבלי לערוך את טבלת המדיניות הגלובלית. ברירות המחדל הן:

| שיעור זמינות | שימור חם | שימור קור | העתקים נדרשים | כיתת אחסון | תג ממשל |
|--------------------|--------------|----------------|------------------|----------------|----------------|
| `hot` | 24 שעות | 14 ימים | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 שעות | 30 ימים | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 שעה | 180 ימים | 3 | `cold` | `da.taikai.archive` |

אם המניפסט משמיט את `availability_class`, נתיב ההטמעה נופל בחזרה ל-
פרופיל `hot` כך שזרמים חיים ישמרו על סט ההעתקים המלא שלהם. מפעילים יכולים
לעקוף את הערכים האלה על ידי עריכת החדש
בלוק `torii.da_ingest.replication_policy.taikai_availability` בתצורה.

## תצורה

הפוליסה חיה תחת `torii.da_ingest.replication_policy` וחושפת א
תבנית *ברירת מחדל* בתוספת מערך של עקיפות לכל מחלקה. מזהי כיתה הם
לא רגיש לאותיות גדולות וקבל את `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, או `custom:<u16>` עבור הרחבות שאושרו על ידי ממשל.
שיעורי אחסון מקבלים `hot`, `warm` או `cold`.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

השאר את הבלוק ללא נגיעה כדי לפעול עם ברירות המחדל המפורטות לעיל. להדק א
class, עדכן את דריסת ההתאמה; לשנות את קו הבסיס לשיעורים חדשים,
ערוך `default_retention`.כדי להתאים שיעורי זמינות ספציפיים של Taikai, הוסף ערכים תחת
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## סמנטיקה של אכיפה

- Torii מחליף את `RetentionPolicy` שסופק על ידי המשתמש בפרופיל האכיפה
  לפני החתיכה או פליטה גלויה.
- מניפסטים שנבנו מראש שמצהירים על פרופיל שמירה לא תואם נדחים
  עם `400 schema mismatch` כך שלקוחות מיושנים לא יכולים להחליש את החוזה.
- כל אירוע עקיפה מתועד (`blob_class`, מדיניות שנשלחה לעומת מדיניות צפויה)
  כדי להציג מתקשרים שאינם עומדים בדרישות במהלך ההשקה.

ראה `docs/source/da/ingest_plan.md` (רשימת אימות) עבור השער המעודכן
מכסה אכיפת שמירה.

## זרימת עבודה של שכפול מחדש (מעקב DA-4)

אכיפת שמירה היא רק הצעד הראשון. גם המפעילים צריכים להוכיח זאת
מניפסטים חיים ופקודות שכפול נשארים מיושרים עם המדיניות המוגדרת כך
ש-SoraFS יכול לשכפל מחדש באופן אוטומטי כתמים שאינם עומדים בדרישות.

1. **צפה בסחף.** Torii פולט
   `overriding DA retention policy to match configured network baseline` בכל פעם
   מתקשר מגיש ערכי שימור מיושנים. חבר את היומן הזה עם
   `torii_sorafs_replication_*` טלמטריה כדי לזהות חסרונות העתק או עיכוב
   פריסות מחדש.
2. **כוונות שונות לעומת העתקים חיים.** השתמש במסייע הביקורת החדש:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   הפקודה טוענת `torii.da_ingest.replication_policy` מהמסופק
   config, מפענח כל מניפסט (JSON או Norito), ואופציונלי מתאים לכל
   `ReplicationOrderV1` מטענים לפי תקציר מניפסט. הסיכום מסמן שניים
   תנאים:

   - `policy_mismatch` - פרופיל שמירת המניפסט שונה מהפרופיל הנאכף
     מדיניות (זה לא אמור לקרות אלא אם Torii מוגדר בצורה שגויה).
   - `replica_shortfall` - סדר השכפול החי דורש פחות העתקים מאשר
     `RetentionPolicy.required_replicas` או מספק פחות מטלות משלו
     מטרה.

   מצב יציאה שאינו אפס מצביע על מחסור פעיל ולכן אוטומציה של CI/on-call
   יכול לדפדף באופן מיידי. צרף את דוח JSON ל-
   חבילת `docs/examples/da_manifest_review_template.md` להצבעות בפרלמנט.
3. **הפעל שכפול מחדש.** כאשר הביקורת מדווחת על חוסר, הנפקה חדשה
   `ReplicationOrderV1` באמצעות כלי הממשל המתוארים ב
   `docs/source/sorafs/storage_capacity_marketplace.md` והפעל מחדש את הביקורת
   עד שהערכת העתק מתכנסת. לעקיפות חירום, צמד את פלט CLI
   עם `iroha app da prove-availability` כך ש-SREs יכולים להתייחס לאותו תקציר
   ועדויות PDP.

כיסוי רגרסיה חי ב-`integration_tests/tests/da/replication_policy.rs`;
החבילה שולחת מדיניות שמירה לא מתאימה ל-`/v1/da/ingest` ומאמתת
שהמניפסט שאוחזר חושף את הפרופיל הנכפה במקום המתקשר
כוונה.

## טלמטריה ולוחות מחוונים להוכחת בריאות (DA-5 bridge)

פריט מפת הדרכים **DA-5** דורש שתוצאות אכיפת PDP/PoTR יהיו ניתנות לביקורת
זמן אמת. אירועים `SorafsProofHealthAlert` מניעים כעת קבוצה ייעודית של
מדדי Prometheus:

- `torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
- `torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
- `torii_sorafs_proof_health_window_end_epoch{provider_id}`

לוח **SoraFS PDP & PoTR Health** Grafana
(`dashboards/grafana/sorafs_pdp_potr_health.json`) חושף כעת את האותות האלה:- *התראות בריאות הוכחה לפי טריגר* משרטטת את שיעורי ההתראה לפי דגל טריגר/עונש כך
  מפעילי Taikai/CDN יכולים להוכיח אם התקפות PDP בלבד, PoTR בלבד או כפולות הן
  ירי.
- *ספקים ב-Cooldown* מדווחים על הסכום החי של ספקים כרגע מתחת ל-a
  SorafsProofHealthAlert התקררות.
- *תמונת מצב של חלון הוכחה בריאות* ממזגת את מוני ה-PDP/PoTR, סכום הקנס,
  דגל ה-cooldown, ו-strike window end epoch לכל ספק אז מבקרי ממשל
  יכול לצרף את הטבלה לחבילות תקריות.

ספרי הפעלה צריכים לקשר פאנלים אלה בעת הצגת ראיות לאכיפת DA; הם
לקשור את כשלי ההוכחה של CLI ישירות למטא נתונים של עונשים על השרשרת
לספק את וו הנצפה שנקרא במפת הדרכים.