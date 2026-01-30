---
lang: ur
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 07d6ef09263e940b5c77e8676948cbc04de52e9b85a241438064ec5d97a28213
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
id: pin-registry-ops
lang: he
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/runbooks/pin_registry_ops.md`. יש לשמור על שתי הגרסאות מסונכרנות עד שהמסמכים הישנים של Sphinx יצאו משימוש.
:::

## סקירה

ה-runbook מתעד כיצד לנטר ולבצע טריאז' ל-Pin Registry של SoraFS ול-SLA של הרפליקציה. המדדים מגיעים מ-`iroha_torii` ומיוצאים דרך Prometheus תחת מרחב השמות `torii_sorafs_*`. Torii דוגם את מצב ה-registry כל 30 שניות ברקע, כך שהדשבורדים נשארים מעודכנים גם כאשר אין מפעילים שפונים ל-endpoints `/v1/sorafs/pin/*`. יש לייבא את הדשבורד המובנה (`docs/source/grafana_sorafs_pin_registry.json`) כדי לקבל פריסת Grafana מוכנה שממפה ישירות לסעיפים הבאים.

## ייחוס מדדים

| מדד | Labels | תיאור |
| --- | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | מלאי manifests על השרשרת לפי מצב מחזור חיים. |
| `torii_sorafs_registry_aliases_total` | — | מספר ה-aliases הפעילים של manifest שנרשמו ב-registry. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | backlog של פקודות רפליקציה לפי סטטוס. |
| `torii_sorafs_replication_backlog_total` | — | Gauge נוחות המשקף פקודות `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | חשבונאות SLA: `met` סופר פקודות שהושלמו בזמן, `missed` מאגד השלמות מאוחרות + פקיעות, `pending` משקף פקודות ממתינות. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | לטנטיות השלמה מצטברת (איפוקים בין ההנפקה להשלמה). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | חלונות מרווח לפקודות ממתינות (דדליין פחות איפוק ההנפקה). |

כל ה-gauges מתאפסים בכל משיכת snapshot, לכן יש לדגום את הדשבורדים בקצב `1m` או מהיר יותר.

## דשבורד Grafana

ה-JSON של הדשבורד כולל שבעה פאנלים המכסים תרחישי עבודה של מפעילים. השאילתות מוצגות להלן לצורך ייחוס מהיר אם אתם מעדיפים לבנות גרפים מותאמים.

1. **מחזור חיים של manifests** – `torii_sorafs_registry_manifests_total` (קיבוץ לפי `status`).
2. **מגמת קטלוג alias** – `torii_sorafs_registry_aliases_total`.
3. **תור פקודות לפי סטטוס** – `torii_sorafs_registry_orders_total` (קיבוץ לפי `status`).
4. **Backlog לעומת פקודות שפגו** – שילוב `torii_sorafs_replication_backlog_total` עם `torii_sorafs_registry_orders_total{status="expired"}` להצגת רוויה.
5. **יחס הצלחת SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **לטנטיות מול מרווח דדליין** – הצמידו `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` ו-`torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. השתמשו בטרנספורמציות Grafana כדי להוסיף תצוגות `min_over_time` כאשר צריך את רצפת המרווח המוחלטת, למשל:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **פקודות שהוחמצו (קצב 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## ספי התראה

- **הצלחת SLA < 0.95 במשך 15 דקות**
  - סף: `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - פעולה: קריאה ל-SRE; להתחיל טריאז' של backlog הרפליקציה.
- **Backlog ממתין מעל 10**
  - סף: `torii_sorafs_replication_backlog_total > 10` מתמשך 10 דקות
  - פעולה: בדיקת זמינות providers ומתג זמני של Torii.
- **פקודות שפגו > 0**
  - סף: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - פעולה: בדיקת manifests של ממשל כדי לאשר churn של providers.
- **p95 של השלמה > מרווח דדליין ממוצע**
  - סף: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - פעולה: לוודא שה-providers מתחייבים לפני הדדליין; לשקול הקצאה מחדש.

### דוגמאות לכללי Prometheus

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
          summary: "SLA רפליקציה של SoraFS מתחת ליעד"
          description: "יחס הצלחת SLA נשאר מתחת ל-95% במשך 15 דקות."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog רפליקציה של SoraFS מעל הסף"
          description: "פקודות רפליקציה ממתינות עברו את תקציב ה-backlog המוגדר."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "פקודות רפליקציה של SoraFS שפגו"
          description: "לפחות פקודת רפליקציה אחת פגה בחמש הדקות האחרונות."
```

## תהליך טריאז'

1. **זיהוי סיבה**
   - אם החמצות SLA עולות בזמן שה-backlog נמוך, התמקדו בביצועי providers (כשלי PoR, השלמות מאוחרות).
   - אם ה-backlog גדל עם החמצות יציבות, בדקו את ה-admission (`/v1/sorafs/pin/*`) כדי לאשר manifests שממתינים לאישור המועצה.
2. **אימות מצב providers**
   - הריצו `iroha app sorafs providers list` ואמתו שהיכולות המוצהרות תואמות לדרישות הרפליקציה.
   - בדקו את מדדי `torii_sorafs_capacity_*` כדי לאשר GiB מוקצים והצלחת PoR.
3. **הקצאת רפליקציה מחדש**
   - הנפיקו פקודות חדשות דרך `sorafs_manifest_stub capacity replication-order` כאשר מרווח ה-backlog (`stat="avg"`) יורד מתחת ל-5 איפוקים (אריזת manifest/CAR משתמשת ב-`iroha app sorafs toolkit pack`).
   - עדכנו את הממשל אם ל-aliases אין bindings פעילים של manifest (ירידה בלתי צפויה ב-`torii_sorafs_registry_aliases_total`).
4. **תיעוד התוצאה**
   - רשמו הערות אירוע ביומן התפעול של SoraFS עם חותמות זמן ו-digests של manifest מושפעים.
   - עדכנו את ה-runbook אם מופיעים מצבי כשל חדשים או דשבורדים חדשים.

## תוכנית פריסה

פעלו לפי ההליך המדורג הבא כאשר מפעילים או מחמירים את מדיניות ה-alias cache בפרודקשן:

1. **הכנת קונפיגורציה**
   - עדכנו את `torii.sorafs_alias_cache` ב-`iroha_config` (user -> actual) עם ה-TTL וחלונות החסד שהוסכמו: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`. ברירות המחדל תואמות למדיניות שב-`docs/source/sorafs_alias_policy.md`.
   - עבור SDKs, הפיצו את אותם ערכים דרך שכבות התצורה (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` ב-bindings של Rust / NAPI / Python) כדי שהאכיפה בצד הלקוח תתאים ל-gateway.
2. **Dry-run ב-staging**
   - פרסו את שינוי התצורה בקלאסטר staging שמדמה את טופולוגיית הפרודקשן.
   - הריצו `cargo xtask sorafs-pin-fixtures` כדי לוודא שה-alias fixtures הקנוניים עדיין מתפענחים ומבצעים round-trip; כל אי התאמה מצביעה על drift במעלה הזרם שיש לטפל בו קודם.
   - הפעילו את endpoints `/v1/sorafs/pin/{digest}` ו-`/v1/sorafs/aliases` עם הוכחות סינתטיות המכסות fresh, refresh-window, expired ו-hard-expired. אמתו את קודי ה-HTTP, ה-headers (`Sora-Proof-Status`, `Retry-After`, `Warning`) ושדות גוף ה-JSON מול ה-runbook.
3. **הפעלה בפרודקשן**
   - פרסו את התצורה החדשה בחלון שינוי סטנדרטי. החילו על Torii תחילה, ואז אתחלו gateways/שירותי SDK לאחר שה-node מאשר את המדיניות החדשה בלוגים.
   - ייבאו את `docs/source/grafana_sorafs_pin_registry.json` ל-Grafana (או עדכנו דשבורדים קיימים) וקבעו את פאנלי רענון ה-alias cache בסביבת NOC.
4. **אימות לאחר הפריסה**
   - ניטור `torii_sorafs_alias_cache_refresh_total` ו-`torii_sorafs_alias_cache_age_seconds` במשך 30 דקות. קפיצות בעקומות `error`/`expired` צריכות להתאים לחלונות הרענון; גידול בלתי צפוי אומר שעל המפעילים לבדוק הוכחות alias ובריאות providers לפני המשך.
   - ודאו שיומני הלקוח מציגים את אותן החלטות מדיניות (SDKs יציגו שגיאות כאשר ההוכחה stale או expired). היעדר אזהרות בצד הלקוח מעיד על תצורה שגויה.
5. **Fallback**
   - אם הנפקת alias מפגרת וחלון הרענון מופעל לעיתים קרובות, הקלו זמנית את המדיניות על ידי הגדלת `refresh_window` ו-`positive_ttl` בקונפיגורציה ואז פרסו מחדש. שמרו על `hard_expiry` כדי שהוכחות stale באמת יידחו.
   - חזרו לתצורה הקודמת על ידי שחזור ה-snapshot הקודם של `iroha_config` אם הטלמטריה ממשיכה להציג ספירות `error` גבוהות, ואז פתחו אירוע לחקירת עיכובי יצירת alias.

## חומרים קשורים

- `docs/source/sorafs/pin_registry_plan.md` — מפת דרכים למימוש והקשר ממשל.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — תפעול עובדי אחסון, משלים את ה-playbook הזה.
