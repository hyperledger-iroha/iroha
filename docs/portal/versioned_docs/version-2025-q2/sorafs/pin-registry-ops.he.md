---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T15:38:30.656337+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: pin-registry-ops-he
slug: /sorafs/pin-registry-ops-he
---

:::הערה מקור קנוני
מראות `docs/source/sorafs/runbooks/pin_registry_ops.md`. שמור את שתי הגרסאות מיושרות בין הגרסאות.
:::

## סקירה כללית

ספר ריצה זה מתעד כיצד לנטר ולבחון את רישום הפינים SoraFS והסכמי רמת שירות השכפול שלו (SLAs). מקור המדדים מ-`iroha_torii` ומיוצאים דרך Prometheus תחת מרחב השמות `torii_sorafs_*`. Torii דוגמת את מצב הרישום במרווח של 30 שניות ברקע, כך שמרכזי המחוונים נשארים עדכניים גם כאשר אין אופרטורים מבצעים סקר את נקודות הקצה `/v1/sorafs/pin/*`. ייבא את לוח המחוונים שנאסף (`docs/source/grafana_sorafs_pin_registry.json`) לפריסת Grafana מוכנה לשימוש שממפה ישירות לקטעים למטה.

## סימוכין מטרי

| מדד | תוויות | תיאור |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | מלאי מניפסט על השרשרת לפי מצב מחזור חיים. |
| `torii_sorafs_registry_aliases_total` | — | ספירת כינויים של מניפסט פעילים שנרשמו ברישום. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | צבר הזמנות שכפול מפולח לפי סטטוס. |
| `torii_sorafs_replication_backlog_total` | — | מד נוחות שיקוף הזמנות `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | חשבונאות SLA: `met` סופרת הזמנות שהושלמו בתוך המועד האחרון, `missed` מצטברת השלמות מאוחרות + תפוגות, `pending` משקף הזמנות שלא נותרו. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | חביון השלמה מצטבר (תקופות בין הנפקה להשלמה). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | חלונות רפויים בהזמנה ממתינה (מועד אחרון מינוס עידן ההנפקה). |

כל המדדים מתאפסים בכל משיכה של תמונת מצב, כך שלוחות מחוונים צריכים לדגום בקצב `1m` או מהר יותר.

## לוח המחוונים של Grafana

לוח המחוונים JSON מגיע עם שבעה לוחות המכסים את זרימות העבודה של המפעיל. השאילתות מפורטות להלן לעיון מהיר אם אתה מעדיף לבנות תרשימים מותאמים אישית.

1. **מחזור חיים של מניפסט** – `torii_sorafs_registry_manifests_total` (מקובץ לפי `status`).
2. **טרנד קטלוג כינוי** – `torii_sorafs_registry_aliases_total`.
3. **תור הזמנות לפי סטטוס** – `torii_sorafs_registry_orders_total` (מקובץ לפי `status`).
4. **פיגור לעומת הזמנות שפג תוקפם** - משלב `torii_sorafs_replication_backlog_total` ו-`torii_sorafs_registry_orders_total{status="expired"}` לרוויה של פני השטח.
5. **יחס הצלחה של SLA** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **חביון לעומת רפיון דדליין** - שכבת על `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` ו-`torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. השתמש בטרנספורמציות Grafana כדי להוסיף תצוגות `min_over_time` כאשר אתה צריך את הרצפה הרפה המוחלטת, לדוגמה:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **הזמנות שלא נענו (תעריף שעה)** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## ספי התראה- **הצלחה SLA  0**
  - סף: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - פעולה: בדוק את מניפסטי הממשל כדי לאשר את נטישת הספקים.
- **השלמה עמ' 95 > ממוצע של מועד אחרון**
  - סף: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - פעולה: ודא שהספקים מתחייבים לפני מועדים; לשקול הוצאת שיבוץ מחדש.

### דוגמה Prometheus כללים

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Triage Workflow

1. **זהה סיבה**
   - אם SLA מפספס עלייה בזמן שהצבר נותר נמוך, התמקד בביצועי הספק (כשלי PoR, השלמות מאוחרות).
   - אם העומס גדל עם החמצות יציבות, בדוק את הקבלה (`/v1/sorafs/pin/*`) כדי לאשר מניפסטים הממתינים לאישור המועצה.
2. **אמת סטטוס ספק**
   - הפעל את `iroha app sorafs providers list` וודא שהיכולות המפורסמות תואמות לדרישות השכפול.
   - בדוק את מדדי `torii_sorafs_capacity_*` כדי לאשר הצלחת GiB ו-PoR.
3. **הקצה מחדש שכפול**
   - הנפק הזמנות חדשות באמצעות `sorafs_manifest_stub capacity replication-order` כאשר צבר ההזמנות (`stat="avg"`) יורד מתחת ל-5 עידנים (אריזה של Manifest/CAR משתמש ב-`iroha app sorafs toolkit pack`).
   - הודע לממשל אם כינויים חסרים כריכות מניפסט אקטיביות (`torii_sorafs_registry_aliases_total` יורד באופן בלתי צפוי).
4. **תוצאת המסמך**
   - רשום הערות תקריות ביומן הפעולות של SoraFS עם חותמות זמן ותקצירי מניפסט מושפעים.
   - עדכן ספר ריצה זה אם מוצגים מצבי כשל או לוחות מחוונים חדשים.

## תוכנית השקה

בצע את ההליך המשלב הזה בעת הפעלה או החזקה של מדיניות המטמון הכינוי בייצור:1. **הכן תצורה**
   - עדכון `torii.sorafs_alias_cache` ב-`iroha_config` (משתמש → בפועל) עם ה-TTLs וחלונות החסד המוסכמים: `positive_ttl`, `refresh_window`, `hard_expiry`, I100NI830,030,030 `revocation_ttl`, `rotation_max_age`, `successor_grace`, ו-`governance_grace`. ברירות המחדל תואמות למדיניות ב-`docs/source/sorafs_alias_policy.md`.
   - עבור SDK, הפיץ את אותם ערכים דרך שכבות התצורה שלהם (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` ב-Rust / NAPI / Python bindings) כך שאכיפת הלקוח תתאים לשער.
2. **ריצה יבשה בהיערכות**
   - פרוס את שינוי התצורה לאשכול בימוי המשקף את טופולוגיית הייצור.
   - הפעל את `cargo xtask sorafs-pin-fixtures` כדי לאשר את התקני הכינוי הקנוניים שעדיין מפענחים וחוזרים; כל חוסר התאמה מרמז על סחיפה מניפסט במעלה הזרם שיש לטפל בו תחילה.
   - הפעל את נקודות הקצה `/v1/sorafs/pin/{digest}` ו-`/v1/sorafs/aliases` עם הוכחות סינתטיות המכסות מקרים טריים, חלון רענון, פג תוקפם ומקרים שפג תוקפם קשה. אמת את קודי מצב ה-HTTP, הכותרות (`Sora-Proof-Status`, `Retry-After`, `Warning`), ושדות הגוף של JSON מול ספר ההפעלה הזה.
3. **אפשר בייצור**
   - גלגל את התצורה החדשה דרך חלון השינוי הסטנדרטי. החל אותו תחילה על Torii, ולאחר מכן הפעל מחדש את שירותי השערים/SDK ברגע שהצומת מאשר את המדיניות החדשה ביומנים.
   - ייבא את `docs/source/grafana_sorafs_pin_registry.json` אל Grafana (או עדכן לוחות מחוונים קיימים) והצמד את לוחות הרענון של המטמון הכינוי לסביבת העבודה של NOC.
4. **אימות לאחר הפריסה**
   - צג `torii_sorafs_alias_cache_refresh_total` ו-`torii_sorafs_alias_cache_age_seconds` למשך 30 דקות. קוצים בעיקומי `error`/`expired` צריכים להיות מתואמים עם חלונות רענון המדיניות; צמיחה בלתי צפויה פירושה שהמפעילים חייבים לבדוק הוכחות כינוי ובריאות הספק לפני שהם ממשיכים.
   - אשר שהיומנים בצד הלקוח מציגים את אותן החלטות מדיניות (ערכות SDK יציגו שגיאות כאשר ההוכחה מיושנת או שפג תוקפם). היעדר אזהרות לקוח מעיד על תצורה שגויה.
5. **נפילה**
   - אם הנפקת הכינוי מפגרת וחלון הרענון יוצא לעתים קרובות, הרפה זמנית את המדיניות על ידי הגדלת `refresh_window` ו-`positive_ttl` בתצורה, ולאחר מכן פריסה מחדש. שמור את `hard_expiry` שלם כך שהוכחות מעופשות באמת עדיין נדחות.
   - חזור לתצורה הקודמת על ידי שחזור תמונת המצב הקודמת של `iroha_config` אם הטלמטריה תמשיך להציג ספירות גבוהות של `error`, ואז פתח תקרית כדי להתחקות אחר עיכובים ביצירת כינויים.

## חומרים קשורים

- `docs/source/sorafs/pin_registry_plan.md` - יישום מפת דרכים והקשר ממשל.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - פעולות אחסון עובדות, משלימות את ספר הרישום הזה.
