---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sns/governance-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 66baa457e66d31ea97563b96e7984c50667491b623ab25672afcde1aca863f6f
source_last_modified: "2026-01-20T13:32:38+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: governance-playbook
lang: he
direction: rtl
source: docs/portal/docs/sns/governance-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sns/governance_playbook.md` וכעת משמש כהעתק
הפורטל הקנוני. קובץ המקור נשאר עבור PRs תרגום.
:::

# פלייבוק הממשל של Sora Name Service (SN-6)

**סטטוס:** נוסח 2026-03-24 - מקור חי למוכנות SN-1/SN-6  
**קישורי roadmap:** SN-6 "Compliance & Dispute Resolution", SN-7 "Resolver & Gateway Sync", מדיניות כתובות ADDR-1/ADDR-5  
**דרישות מקדימות:** סכמת רישום ב-[`registry-schema.md`](./registry-schema.md), חוזה API של registrar ב-[`registrar-api.md`](./registrar-api.md), הנחיות UX לכתובות ב-[`address-display-guidelines.md`](./address-display-guidelines.md), וכללי מבנה חשבון ב-[`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

פלייבוק זה מתאר כיצד גופי הממשל של Sora Name Service (SNS) מאמצים אמנות,
מאשרים רישומים, מסלימים מחלוקות ומוכיחים שמצבי resolver ו-gateway נשארים
מסונכרנים. הוא עומד בדרישת ה-roadmap שה-CLI `sns governance ...`, מניופסטים של
Norito וארטיפקטי ביקורת ישתפו מקור תפעולי אחד לפני N1 (השקה ציבורית).

## 1. היקף וקהל יעד

המסמך מיועד ל:

- חברי Governance Council שמצביעים על אמנות, מדיניות סיומות ותוצאות מחלוקת.
- חברי guardian board שמנפיקים הקפאות חירום ובוחנים ביטולים.
- stewards של סיומות שמפעילים תורים של registrar, מאשרים מכרזים ומנהלים חלוקת
  הכנסות.
- מפעילי resolver/gateway האחראים להפצת SoraDNS, לעדכוני GAR ול-guardrails של
  טלמטריה.
- צוותי תאימות, אוצרות ותמיכה שחייבים להוכיח שכל פעולה ממשלית הותירה ארטיפקטים
  של Norito שניתנים לביקורת.

הוא מכסה את שלבי הבטא הסגורה (N0), ההשקה הציבורית (N1) וההתרחבות (N2)
המפורטים ב-`roadmap.md`, תוך קישור כל זרימת עבודה לראיות הדרושות, לדשבורדים
ולנתיבי הסלמה.

## 2. תפקידים ומפת קשר

| תפקיד | אחריות מרכזית | ארטיפקטים וטלמטריה עיקריים | הסלמה |
|-------|---------------|----------------------------|-------|
| Governance Council | מנסח ומאשר אמנות, מדיניות סיומות, פסקי מחלוקת ורוטציות steward. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, פתקי הצבעה של המועצה המאוחסנים באמצעות `sns governance charter submit`. | יו"ר המועצה + מעקב סדר יום הממשל. |
| Guardian Board | מנפיק הקפאות soft/hard, קנונים חירומיים וביקורות 72 h. | כרטיסי guardian שמופקים על ידי `sns governance freeze`, ומניופסטי override שנשמרים תחת `artifacts/sns/guardian/*`. | רוטציית guardian on-call (<=15 min ACK). |
| Suffix Stewards | מפעילים תורי registrar, מכרזים, מדרגות מחיר ותקשורת עם לקוחות; מאשרים תאימות. | מדיניות steward ב-`SuffixPolicyV1`, דפי מחירי ייחוס, acknowledgements של steward המאוחסנים לצד מזכרים רגולטוריים. | מוביל תוכנית steward + PagerDuty לפי סיומת. |
| Registrar & Billing Ops | מפעילים נקודות קצה `/v1/sns/*`, מאזנים תשלומים, משדרים טלמטריה ושומרים snapshots של CLI. | API של registrar ([`registrar-api.md`](./registrar-api.md)), מדדים `sns_registrar_status_total`, הוכחות תשלום תחת `artifacts/sns/payments/*`. | duty manager של registrar וקישור אוצרות. |
| Resolver & Gateway Operators | שומרים על SoraDNS, GAR ומצב gateway מסונכרנים עם אירועי registrar; משדרים מדדי שקיפות. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | resolver SRE on-call + bridge של gateway ops. |
| Treasury & Finance | מיישמים חלוקת הכנסות 70/30, carve-outs להפניות, דיווחי מס/אוצרות והצהרות SLA. | מניופסטי צבירת הכנסות, יצוא Stripe/אוצרות, נספחי KPI רבעוניים תחת `docs/source/sns/regulatory/`. | בקר כספים + קצין תאימות. |
| Compliance & Regulatory Liaison | עוקב אחר חובות גלובליות (EU DSA וכו'), מעדכן KPI covenants ומגיש גילויים. | מזכרים רגולטוריים ב-`docs/source/sns/regulatory/`, מצגות ייחוס, רשומות `ops/drill-log.md` לתרגילי tabletop. | מוביל תוכנית תאימות. |
| Support / SRE On-call | מטפל באירועים (התנגשויות, דריפט חיוב, תקלות resolver), מתאם הודעות ללקוחות ובעלים של runbooks. | תבניות אירוע, `ops/drill-log.md`, ראיות מעבדה מבוימות, תמלילי Slack/war-room תחת `incident/`. | רוטציית SNS on-call + הנהלת SRE. |

## 3. ארטיפקטים קנוניים ומקורות נתונים

| ארטיפקט | מיקום | מטרה |
|---------|-------|------|
| אמנה + נספחי KPI | `docs/source/sns/governance_addenda/` | אמנות חתומות עם בקרת גרסאות, KPI covenants והחלטות ממשל שמקושרות להצבעות CLI. |
| סכמת רישום | [`registry-schema.md`](./registry-schema.md) | מבני Norito קנוניים (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| חוזה registrar | [`registrar-api.md`](./registrar-api.md) | payloads REST/gRPC, מדדי `sns_registrar_status_total`, וציפיות governance hook. |
| מדריך UX לכתובות | [`address-display-guidelines.md`](./address-display-guidelines.md) | רינדורים קנוניים של i105 (מועדף) ודחוסים (אפשרות שנייה) שמוחזרים על ידי ארנקים/אקספלוררים. |
| מסמכי SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | גזירת host דטרמיניסטית, זרימת tailer לשקיפות וכללי התראה. |
| מזכרים רגולטוריים | `docs/source/sns/regulatory/` | הערות intake לפי תחום שיפוט (למשל EU DSA), acknowledgements של steward, נספחי תבנית. |
| Drill log | `ops/drill-log.md` | רישום של תרגילי כאוס ו-IR הנדרשים לפני יציאה משלבים. |
| אחסון ארטיפקטים | `artifacts/sns/` | הוכחות תשלום, כרטיסי guardian, דיפי resolver, יצוא KPI ופלט CLI חתום מ-`sns governance ...`. |

כל פעולת ממשל חייבת להפנות לפחות לארטיפקט אחד בטבלה לעיל כדי שמבקרים יוכלו
לשחזר את מסלול ההחלטה בתוך 24 שעות.

## 4. פלייבוקים למחזור חיים

### 4.1 תנועות אמנה ו-steward

| שלב | בעלים | CLI / עדות | הערות |
|------|-------|------------|-------|
| טיוטת נספח ודלתות KPI | רפורטר מועצה + מוביל steward | תבנית Markdown תחת `docs/source/sns/governance_addenda/YY/` | לכלול מזהי KPI covenant, hooks של טלמטריה ותנאי הפעלה. |
| הגשת הצעה | יו"ר המועצה | `sns governance charter submit --input SN-CH-YYYY-NN.md` (מפיק `CharterMotionV1`) | ה-CLI מפיק מניופסט Norito תחת `artifacts/sns/governance/<id>/charter_motion.json`. |
| הצבעה ו-guardian acknowledgement | מועצה + guardians | `sns governance ballot cast --proposal <id>` ו-`sns governance guardian-ack --proposal <id>` | לצרף פרוטוקולים מגובים ב-hash והוכחות קוורום. |
| קבלת steward | תוכנית steward | `sns governance steward-ack --proposal <id> --signature <file>` | נדרש לפני שינוי מדיניות סיומות; לרשום מעטפת תחת `artifacts/sns/governance/<id>/steward_ack.json`. |
| הפעלה | registrar ops | לעדכן `SuffixPolicyV1`, לרענן caches של registrar, לפרסם הערה ב-`status.md`. | חותמת זמן הפעלה נרשמת ב-`sns_governance_activation_total`. |
| Audit log | תאימות | להוסיף רשומה ל-`docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` ול-drill log אם בוצע tabletop. | לכלול הפניות לדשבורדי טלמטריה ודיפי מדיניות. |

### 4.2 אישורי רישום, מכרז ותמחור

1. **Preflight:** ה-registrar בודק את `SuffixPolicyV1` כדי לאשר רמת מחיר, תנאים
   זמינים וחלונות grace/redemption. יש לשמור על דפי המחיר מסונכרנים עם טבלת
   הדרגות 3/4/5/6-9/10+ (דרגת בסיס + מקדמי סיומת) המתועדת ב-roadmap.
2. **מכרזים sealed-bid:** לבריכות premium, הפעילו את מחזור 72 h commit / 24 h reveal
   באמצעות `sns governance auction commit` / `... reveal`. פרסמו את רשימת ה-commit
   (hashes בלבד) תחת `artifacts/sns/auctions/<name>/commit.json` כדי שמבקרים יוכלו
   לאמת אקראיות.
3. **אימות תשלום:** registrars מאמתים `PaymentProofV1` מול חלוקת האוצר
   (70% treasury / 30% steward עם carve-out referral <=10%). שמרו את ה-Norito JSON
   תחת `artifacts/sns/payments/<tx>.json` וקשרו אותו בתגובת registrar (`RevenueAccrualEventV1`).
4. **Hook ממשל:** לצרף `GovernanceHookV1` עבור שמות premium/guarded עם הפניות למזהי
   הצעות מועצה וחתימות steward. היעדר hooks יוצר `sns_err_governance_missing`.
5. **הפעלה + סנכרון resolver:** לאחר ש-Torii מפיק את אירוע הרישום, להפעיל את tailer
   השקיפות של resolver כדי לוודא שהמצב החדש של GAR/zone הופץ (ראו 4.5).
6. **חשיפת לקוח:** לעדכן את ledger ללקוחות (wallet/explorer) דרך fixtures משותפים
   ב-[`address-display-guidelines.md`](./address-display-guidelines.md), ולוודא
   שרינדורים i105 ודחוסים תואמים להנחיות copy/QR.

### 4.3 חידושים, חיוב והתאמת אוצרות

- **תהליך חידוש:** registrars אוכפים חלון grace של 30 יום + חלון redemption של 60 יום
  המופיעים ב-`SuffixPolicyV1`. אחרי 60 יום רצף reopen הולנדי (7 ימים, עמלה 10x
  שיורדת 15%/יום) מופעל אוטומטית דרך `sns governance reopen`.
- **חלוקת הכנסה:** כל חידוש או העברה יוצר `RevenueAccrualEventV1`. יצואי אוצרות
  (CSV/Parquet) חייבים להתאים לאירועים אלה מדי יום; לצרף הוכחות תחת
  `artifacts/sns/treasury/<date>.json`.
- **Carve-outs להפניות:** אחוזי referral אופציונליים נעקבים לפי סיומת על ידי
  הוספת `referral_share` למדיניות steward. registrars מפיקים את החלוקה הסופית
  ושומרים מניופסטי referral לצד הוכחת התשלום.
- **קצב דיווח:** פיננסים מפרסמים נספחי KPI חודשיים (רישומים, חידושים, ARPU,
  שימוש במחלוקות/bond) תחת `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. הדשבורדים
  צריכים למשוך מאותן טבלאות export כדי שמספרי Grafana יתאימו לראיות ה-ledger.
- **סקירת KPI חודשית:** נקודת הבדיקה של יום שלישי הראשון מאגדת את מוביל הפיננסים,
  steward בתורנות ו-PM התוכנית. פתחו את [SNS KPI dashboard](./kpi-dashboard.md)
  (הטמעת פורטל `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`), ייצאו
  טבלאות throughput + revenue של registrar, רשמו דלתות בנספח והצמידו את הארטריפקטים
  לממו. פתחו אינסידנט אם הסקירה מצאה הפרות SLA (חלונות freeze >72 h, קפיצות שגיאה
  של registrar, דריפט ARPU).

### 4.4 הקפאות, מחלוקות וערעורים

| שלב | בעלים | פעולה ועדות | SLA |
|------|-------|-------------|-----|
| בקשת soft freeze | Steward / תמיכה | לפתוח טיקט `SNS-DF-<id>` עם הוכחות תשלום, הפניה ל-bond מחלוקת וסלקטור(ים) מושפעים. | <=4 h מרגע הקליטה. |
| טיקט guardian | Guardian board | `sns governance freeze --selector <i105> --reason <text> --until <ts>` מפיק `GuardianFreezeTicketV1`. לשמור את JSON הטיקט תחת `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h ביצוע. |
| אישור מועצה | Governance council | לאשר או לדחות הקפאות, לתעד את ההחלטה עם קישור לטיקט guardian ול-digest של bond המחלוקת. | מושב מועצה הבא או הצבעה אסינכרונית. |
| פאנל בוררות | תאימות + steward | לכנס פאנל של 7 מושבעים (לפי roadmap) עם פתקי הצבעה hash דרך `sns governance dispute ballot`. לצרף קבלות הצבעה אנונימיות לחבילת אינסידנט. | פסק דין <=7 ימים לאחר הפקדת bond. |
| ערעור | Guardian + מועצה | ערעורים מכפילים את ה-bond וחוזרים על תהליך המושבעים; לרשום מניופסט Norito `DisputeAppealV1` ולהפנות לטיקט הראשי. | <=10 ימים. |
| הפשרת הקפאה ושיקום | Registrar + resolver ops | לבצע `sns governance unfreeze --selector <i105> --ticket <id>`, לעדכן סטטוס registrar ולהפיץ דיפי GAR/resolver. | מייד לאחר פסק הדין. |

קנונים חירומיים (הקפאות שמופעלות ע"י guardian <=72 h) עוקבים אחרי אותו זרם אך
דורשים ביקורת מועצה רטרואקטיבית והערת שקיפות תחת `docs/source/sns/regulatory/`.

### 4.5 הפצת resolver ו-gateway

1. **Hook לאירוע:** כל אירוע רישום נשלח לזרם האירועים של resolver
   (`tools/soradns-resolver` SSE). ops של resolver נרשמים ומקליטים diffs דרך tailer
   השקיפות (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **עדכון תבנית GAR:** gateways חייבים לעדכן תבניות GAR שמופנות על ידי
   `canonical_gateway_suffix()` ולחתום מחדש על רשימת `host_pattern`. לשמור diffs
   ב-`artifacts/sns/gar/<date>.patch`.
3. **פרסום zonefile:** להשתמש בשלד zonefile המתואר ב-`roadmap.md` (name, ttl, cid, proof)
   ולדחוף אותו ל-Torii/SoraFS. לשמור Norito JSON ב-`artifacts/sns/zonefiles/<name>/<version>.json`.
4. **בדיקת שקיפות:** להריץ `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   כדי לוודא שההתראות ירוקות. לצרף את פלט הטקסט של Prometheus לדוח השקיפות השבועי.
5. **ביקורת gateway:** לתעד דגימות של כותרות `Sora-*` (cache policy, CSP, GAR digest)
   ולצרף ללוג הממשל כדי לאפשר לאופרטורים להוכיח שה-gateway שירת את השם החדש עם
   guardrails מיועדים.

## 5. טלמטריה ודיווח

| אות | מקור | תיאור / פעולה |
|-----|------|---------------|
| `sns_registrar_status_total{result,suffix}` | Torii registrar handlers | מונה הצלחה/שגיאה לרישומים, חידושים, הקפאות, העברות; התראה כאשר `result="error"` קופץ לפי סיומת. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | מדדי Torii | SLOs של לטנטיות ל-handlers של API; מזין דשבורדים הבנויים מ-`torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` ו-`soradns_bundle_cid_drift_total` | tailer שקיפות resolver | מזהה הוכחות מיושנות או drift של GAR; guardrails מוגדרים ב-`dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | Governance CLI | מונה שמוגדל בכל הפעלה של charter/addendum; משמש ליישוב החלטות מועצה מול addenda שפורסמו. |
| `guardian_freeze_active` gauge | Guardian CLI | עוקב אחר חלונות freeze soft/hard לכל סלקטור; יש לזמן SRE אם הערך נשאר `1` מעבר ל-SLA המוצהר. |
| Dashboards של נספחי KPI | Finance / Docs | Rollups חודשיים שמתפרסמים לצד מזכרים רגולטוריים; הפורטל מטמיע אותם דרך [SNS KPI dashboard](./kpi-dashboard.md) כדי ש-stewards ורגולטורים יראו אותה תצוגת Grafana. |

## 6. דרישות ראיות וביקורת

| פעולה | ראיות לארכוב | אחסון |
|-------|--------------|--------|
| שינוי אמנה / מדיניות | מניופסט Norito חתום, תמליל CLI, diff KPI, acknowledgement steward. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| רישום / חידוש | payload `RegisterNameRequestV1`, `RevenueAccrualEventV1`, הוכחת תשלום. | `artifacts/sns/payments/<tx>.json`, לוגים של API registrar. |
| מכרז | מניופסטי commit/reveal, seed אקראי, גיליון חישוב זוכה. | `artifacts/sns/auctions/<name>/`. |
| הקפאה / הפשרה | טיקט guardian, hash של הצבעת מועצה, URL של log אינסידנט, תבנית תקשורת ללקוחות. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| הפצת resolver | diff של zonefile/GAR, קטע JSONL של tailer, snapshot של Prometheus. | `artifacts/sns/resolver/<date>/` + דוחות שקיפות. |
| Intake רגולטורי | memo intake, tracker דדליינים, acknowledgement steward, סיכום שינויי KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. רשימת בדיקה לשערי שלב

| שלב | קריטריוני יציאה | חבילת ראיות |
|------|------------------|-------------|
| N0 - בטא סגורה | סכמת רישום SN-1/SN-2, CLI registrar ידני, drill guardian הושלם. | motion אמנה + ACK steward, לוגי dry-run של registrar, דוח שקיפות resolver, רשומה ב-`ops/drill-log.md`. |
| N1 - השקה ציבורית | מכרזים + מדרגות מחיר קבועות ל-`.sora`/`.nexus`, registrar בשירות עצמי, auto-sync של resolver, דשבורדי חיוב. | diff של גיליון מחירים, תוצאות CI של registrar, נספח תשלום/KPI, פלט tailer שקיפות, הערות rehearsal של אינסידנט. |
| N2 - התרחבות | `.dao`, APIs לריסלר, פורטל מחלוקות, scorecards של steward, דשבורדי אנליטיקה. | צילומי מסך של הפורטל, מדדי SLA של מחלוקות, יצוא scorecards של steward, אמנת ממשל מעודכנת עם מדיניות ריסלר. |

יציאות שלב דורשות drills של tabletop (מסלול רישום תקין, freeze, outage של resolver)
עם ארטיפקטים מצורפים ב-`ops/drill-log.md`.

## 8. תגובת אינסידנטים והסלמה

| טריגר | חומרה | בעלים מיידי | פעולות חובה |
|-------|-------|--------------|-------------|
| drift ב-resolver/GAR או הוכחות מיושנות | Sev 1 | resolver SRE + guardian board | לזמן on-call של resolver, לאסוף פלט tailer, להחליט האם להקפיא שמות מושפעים, לפרסם עדכון סטטוס כל 30 min. |
| תקלה ב-registrar, כשל חיוב או שגיאות API נרחבות | Sev 1 | duty manager של registrar | לעצור מכרזים חדשים, לעבור ל-CLI ידני, להודיע stewards/אוצרות, לצרף לוגים של Torii למסמך אינסידנט. |
| מחלוקת שם יחיד, mismatch תשלום או הסלמת לקוח | Sev 2 | steward + מוביל תמיכה | לאסוף הוכחות תשלום, להחליט אם נדרש freeze soft, לענות לפונה במסגרת ה-SLA, לתעד את התוצאה ב-tracker המחלוקות. |
| ממצא ביקורת תאימות | Sev 2 | liaison תאימות | לנסח תוכנית תיקון, לשמור memo תחת `docs/source/sns/regulatory/`, לקבוע ישיבת מועצה להמשך. |
| drill או rehearsal | Sev 3 | PM התוכנית | לבצע את התרחיש המוסרט מתוך `ops/drill-log.md`, לארכב ארטיפקטים, לסמן פערים כמשימות ב-roadmap. |

כל אינסידנט חייב ליצור `incident/YYYY-MM-DD-sns-<slug>.md` עם טבלאות בעלות,
לוגי פקודות והפניות לראיות שנוצרו לאורך הפלייבוק.

## 9. הפניות

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (SNS, DG, ADDR קטעים)

יש לעדכן את הפלייבוק הזה בכל פעם שנוסח האמנות, משטחי CLI או חוזי טלמטריה
משתנים; פריטי roadmap שמפנים ל-`docs/source/sns/governance_playbook.md` צריכים
תמיד להתאים לגרסה העדכנית.
