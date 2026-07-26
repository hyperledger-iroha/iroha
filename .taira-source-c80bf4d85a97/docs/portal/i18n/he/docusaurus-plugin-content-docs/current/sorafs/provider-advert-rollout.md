---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

> מותאם מ- [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# תוכנית rollout ותאימות של adverts לספקי SoraFS

תוכנית זו מתאמת את המעבר מ-adverts סובלניים של ספקים אל משטח `ProviderAdvertV1`
המוכפף לממשל במלואו ונדרש לאחזור chunks מרובי-מקור. היא מתמקדת בשלושה deliverables:

- **מדריך מפעיל.** פעולות שלב-אחר-שלב שעל ספקי אחסון להשלים לפני כל gate.
- **כיסוי טלמטריה.** dashboards ו-alerts שבהם Observability ו-Ops משתמשים כדי
  לוודא שהרשת מקבלת רק adverts תואמים.
- **ציר זמן תאימות.** תאריכים מפורשים לדחיית envelopes ישנים כדי שצוותי SDK
  ו-tooling יוכלו לתכנן את ה-releases שלהם.

ה-rollout מתואם עם אבני הדרך SF-2b/2c ב-
[roadmap של הגירת SoraFS](./migration-roadmap) ומניח שמדיניות הקבלה ב-
[provider admission policy](./provider-admission-policy) כבר בתוקף.

## ציר זמן שלבים

| שלב | חלון (יעד) | התנהגות | פעולות מפעיל | מיקוד Observability |
|-------|-----------------|-----------|------------------|-------------------|
| **R0 – תצפית בסיסית** | עד **2025-03-31** | Torii מקבל גם adverts שאושרו על ידי governance וגם payloads ישנים שקדמו ל-`ProviderAdvertV1`. לוגי ingestion מזהירים כאשר adverts חסרים `chunk_range_fetch` או `profile_aliases` קנוניים. | - לחדש adverts דרך pipeline לפרסום provider advert (ProviderAdvertV1 + governance envelope) עם `profile_id=sorafs.sf1@1.0.0`, `profile_aliases` קנוניים ו-`signature_strict=true`. <br />- להריץ את בדיקות `sorafs_fetch` החדשות מקומית; יש לבצע triage לאזהרות על capabilities לא מוכרות. | לפרסם פאנלים זמניים ב-Grafana (ראו למטה) ולהגדיר ספי alert אך להשאירם במצב אזהרה בלבד. |
| **R1 – שער אזהרה** | **2025-04-01 → 2025-05-15** | Torii ממשיך לקבל adverts ישנים אך מגדיל `torii_sorafs_admission_total{result="warn"}` כאשר payload חסר `chunk_range_fetch` או נושא capabilities לא מוכרות ללא `allow_unknown_capabilities=true`. כלי CLI נכשל כעת ברה-יצירה אם ה-handle הקנוני חסר. | - לסובב adverts ב-staging וב-production כדי לכלול payloads של `CapabilityType::ChunkRangeFetch`, ובעת GREASE testing להגדיר `allow_unknown_capabilities=true`. <br />- לעדכן runbooks תפעוליים עם שאילתות טלמטריה חדשות. | לקדם dashboards לסבב on-call; להגדיר אזהרות כאשר אירועי `warn` עוברים 5% מהתעבורה במשך 15 דקות. |
| **R2 – Enforcement** | **2025-05-16 → 2025-06-30** | Torii דוחה adverts שחסרים envelopes של governance, handle קנוני לפרופיל או capability `chunk_range_fetch`. handles ישנים מסוג `namespace-name` כבר אינם ניתנים לפרסור. capabilities לא מוכרות ללא GREASE opt-in נכשלות כעת עם `reason="unknown_capability"`. | - לוודא ש-envelopes של production קיימים תחת `torii.sorafs.admission_envelopes_dir` ולסובב את יתר adverts הישנים. <br />- לוודא ש-SDKs מפיקים רק handles קנוניים ועוד aliases אופציונליים לתאימות לאחור. | להפעיל pager alerts: `torii_sorafs_admission_total{result="reject"}` > 0 למשך 5 דקות מחייב פעולה. לעקוב אחר יחס קבלה והיסטוגרמות סיבות הקבלה. |

## רשימת בדיקות למפעילים

1. **מיפוי adverts.** רשמו כל advert שפורסם ותעדו:
   - נתיב ה-governing envelope (`defaults/nexus/sorafs_admission/...` או מקבילה ב-production).
   - `profile_id` ו-`profile_aliases` של ה-advert.
   - רשימת capabilities (מינימום `torii_gateway` ו-`chunk_range_fetch`).
   - דגל `allow_unknown_capabilities` (נדרש כאשר קיימים TLVs שמורים ל-vendor).
2. **חידוש עם tooling של ספק.**
   - בנו מחדש את ה-payload עם publisher של provider advert, תוך הקפדה על:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` עם `max_span` מוגדר
     - `allow_unknown_capabilities=<true|false>` כאשר קיימים TLVs של GREASE
   - בדקו דרך `/v1/sorafs/providers` ו-`sorafs_fetch`; יש לבצע triage לאזהרות על
     capabilities לא מוכרות.
3. **אימות מוכנות multi-source.**
   - הריצו `sorafs_fetch` עם `--provider-advert=<path>`; ה-CLI נכשל כעת כאשר
     `chunk_range_fetch` חסר ומדפיס אזהרות עבור capabilities לא מוכרות שהתעלם מהן.
     שמרו את דו"ח ה-JSON וארכבו אותו עם לוגי תפעול.
4. **הכנת renewals.**
   - הגישו envelopes של `ProviderAdmissionRenewalV1` לפחות 30 יום לפני ה-enforcement
     ב-gateway (R2). ה-renewals חייבים לשמור על ה-handle הקנוני ועל סט ה-capabilities;
     רק stake, endpoints או metadata צריכים להשתנות.
5. **תקשורת עם צוותים תלויים.**
   - בעלי SDK חייבים לשחרר גרסאות שמציגות אזהרות למפעילים כאשר adverts נדחים.
   - DevRel מכריז על כל מעבר שלב; כללו קישורי dashboards ולוגיקת הסף למטה.
6. **התקנת dashboards ו-alerts.**
   - יבאו את ה-export של Grafana והציבו תחת **SoraFS / Provider Rollout** עם UID
     `sorafs-provider-admission`.
   - ודאו שכללי alert מצביעים לערוץ `sorafs-advert-rollout` המשותף ב-staging וב-production.

## טלמטריה ודשבורדים

המדדים הבאים כבר זמינים דרך `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — סופר תוצאות של קבלה, דחייה
  ואזהרות. הסיבות כוללות `missing_envelope`, `unknown_capability`, `stale`, ו-`policy_violation`.

Grafana export: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
ייבאו את הקובץ לריפו dashboards משותף (`observability/dashboards`) ועדכנו רק את
UID של ה-datasource לפני פרסום.

ה-board מתפרסם תחת תיקיית Grafana **SoraFS / Provider Rollout** עם UID יציב
`sorafs-provider-admission`. כללי alert
`sorafs-admission-warn` (warning) ו-`sorafs-admission-reject` (critical)
מוגדרים מראש להשתמש במדיניות ההתרעה `sorafs-advert-rollout`; עדכנו את contact
point אם רשימת היעדים משתנה במקום לערוך את JSON של ה-dashboard.

פאנלים מומלצים ב-Grafana:

| Panel | Query | Notes |
|-------|-------|-------|
| **Admission outcome rate** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Stack chart להצגת accept vs warn vs reject. התרעה כאשר warn > 0.05 * total (warning) או reject > 0 (critical). |
| **Warning ratio** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | סדרת זמן בודדת שמזינה את סף ה-pager (5% warning rate בגלגול 15 דקות). |
| **Rejection reasons** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | מניע triage ב-runbook; צרפו קישורים לצעדי mitigation. |
| **Refresh debt** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | מציין ספקים שהחמיצו את refresh deadline; בדקו מול לוגי discovery cache. |

Artefacts של CLI עבור dashboards ידניים:

- `sorafs_fetch --provider-metrics-out` כותב מוני `failures`, `successes` ו-
  `disabled` לכל ספק. יבאו ל-dashboards ad-hoc כדי לנטר dry-runs של orchestrator
  לפני מעבר ספקים ב-production.
- שדות `chunk_retry_rate` ו-`provider_failure_rate` בדו"ח JSON מדגישים throttling
  או סימפטומים של payloads stale שמקדימים לעיתים דחיות admission.

### מבנה dashboard Grafana

Observability מפרסמת board ייעודי — **SoraFS Provider Admission
Rollout** (`sorafs-provider-admission`) — תחת **SoraFS / Provider Rollout** עם
מזהי panel קנוניים:

- Panel 1 — *Admission outcome rate* (stacked area, יחידות "ops/min").
- Panel 2 — *Warning ratio* (single series), עם הביטוי
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Rejection reasons* (time series לפי `reason`), ממוינות לפי
  `rate(...[5m])`.
- Panel 4 — *Refresh debt* (stat), משקף את השאילתה בטבלה לעיל ומסומן
  ב-deadlines של refresh שנמשכו מה-migration ledger.

העתיקו (או צרו) את שלד ה-JSON בריפו dashboards של התשתית
`observability/dashboards/sorafs_provider_admission.json`, ואז עדכנו רק את UID
של ה-datasource; מזהי panels וכללי alert מוזכרים ב-runbooks למטה, לכן הימנעו
משינוי המספור בלי לעדכן את התיעוד.

לנוחות, הריפו מספק הגדרת dashboard רפרנס ב-
`docs/source/grafana_sorafs_admission.json`; העתיקו לתיקיית Grafana אם צריך
נקודת התחלה לבדיקות מקומיות.

### כללי התראה של Prometheus

הוסיפו את קבוצת הכללים הבאה ל-
`observability/prometheus/sorafs_admission.rules.yml` (צרו את הקובץ אם זו קבוצת
הכללים הראשונה של SoraFS) וכללו אותה בקונפיגורציית Prometheus. החליפו
`<pagerduty>` בתווית הניתוב האמיתית של סבב ה-on-call.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

הריצו `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
לפני דחיפת שינויים כדי לוודא שהתחביר עובר `promtool check rules`.

## מטריצת תאימות

| מאפייני advert | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` קיים, aliases קנוניים, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| חסר capability `chunk_range_fetch` | ⚠️ Warn (ingest + telemetry) | ⚠️ Warn | ❌ Reject (`reason="missing_capability"`) | ❌ Reject |
| TLVs של capability לא מוכרת ללא `allow_unknown_capabilities=true` | ✅ | ⚠️ Warn (`reason="unknown_capability"`) | ❌ Reject | ❌ Reject |
| `refresh_deadline` פג תוקף | ❌ Reject | ❌ Reject | ❌ Reject | ❌ Reject |
| `signature_strict=false` (fixtures דיאגנוסטיים) | ✅ (פיתוח בלבד) | ⚠️ Warn | ⚠️ Warn | ❌ Reject |

כל הזמנים הם UTC. תאריכי enforcement משוקפים ב-migration ledger ולא ישתנו ללא
הצבעת council; כל שינוי מחייב עדכון הקובץ הזה וה-ledger באותו PR.

> **הערת יישום:** R1 מציג את הסדרה `result="warn"` בתוך
> `torii_sorafs_admission_total`. הפאץ' של ingestion ב-Torii שמוסיף את התווית החדשה
> מנוהל לצד משימות הטלמטריה של SF-2; עד אז השתמשו בדגימת לוגים למעקב אחרי adverts ישנים.

## תקשורת וטיפול באירועים

- **דיווח סטטוס שבועי.** DevRel מפיץ סיכום קצר של מדדי admission, אזהרות פתוחות
  ו-deadlines קרובים.
- **תגובה לאירועים.** כאשר alerts של `reject` מופעלים, אנשי on-call:
  1. שולפים את ה-advert הבעייתי דרך Torii discovery (`/v1/sorafs/providers`).
  2. מריצים מחדש את בדיקת ה-advert ב-pipeline של הספק ומשווים עם
     `/v1/sorafs/providers` כדי לשחזר את התקלה.
  3. מתאמים עם הספק את החלפת ה-advert לפני refresh deadline הבא.
- **קפיאת שינויים.** אין שינויים בסכימת capabilities במהלך R1/R2 ללא אישור ועדת
  rollout; ניסויי GREASE חייבים להתבצע בחלון התחזוקה השבועי ולהירשם ב-migration ledger.

## מקורות

- [SoraFS Node/Client Protocol](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Provider Admission Policy](./provider-admission-policy)
- [Migration Roadmap](./migration-roadmap)
- [Provider Advert Multi-Source Extensions](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)
