---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/da/threat-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa720bd35225edd8d53d3d652fe6104d44d759a7f7b3c50165c1c392051aa6b3
source_last_modified: "2026-01-19T07:28:06+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/da/threat-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/da/threat_model.md`. שמרו על שתי הגרסאות
מסונכרנות עד שהמסמכים הישנים יפרשו.
:::

# מודל איומים של Data Availability ב-Sora Nexus

_סקירה אחרונה: 2026-01-19 -- סקירה מתוכננת הבאה: 2026-04-19_

קצב תחזוקה: Data Availability Working Group (<=90 ימים). כל גרסה חייבת להופיע
ב-`status.md` עם קישורים לכרטיסי מיתון פעילים ולארטיפקטים של סימולציות.

## מטרה והיקף

תוכנית Data Availability (DA) שומרת על שידורי Taikai, blobs של Nexus lane,
וארכיבי ממשל נגישים תחת תקלות ביזנטיות, רשת ואופרציות. מודל איומים זה מעגן את
עבודת ההנדסה של DA-1 (ארכיטקטורה ומודל איומים) ומשמש בסיס למשימות DA
הבאות (DA-2 עד DA-10).

רכיבים בהיקף:
- הרחבת ingest ב-Torii וכתיבת metadata של Norito.
- עצי אחסון blobs מבוססי SoraFS (tiers hot/cold) ומדיניות שכפול.
- התחייבויות בלוקים של Nexus (wire formats, proofs, APIs של לקוח קל).
- hooks לאכיפת PDP/PoTR עבור payloads של DA.
- זרימות מפעיל (pinning, eviction, slashing) וצינורות observability.
- אישורי ממשל שמאשרים או מפנים מפעילי DA ותוכן.

מחוץ להיקף למסמך זה:
- מודל כלכלי מלא (נתפס ב-workstream DA-7).
- פרוטוקולי בסיס של SoraFS שכבר מכוסים במודל האיומים של SoraFS.
- ארגונומיה של SDK לקוח מעבר לשיקולי surface של איומים.

## סקירה ארכיטקטונית

1. **הגשה:** לקוחות מגישים blobs דרך API ה-ingest של Torii DA. הצומת מחלק blobs
   ל-chunks, מקודד manifests של Norito (סוג blob, lane, epoch, דגלי codec),
   ומאחסן chunks ב-tier החם של SoraFS.
2. **פרסום:** intents של pin ורמזי שכפול מתפשטים לספקי אחסון דרך ה-registry
   (SoraFS marketplace) עם תגי מדיניות שמציינים יעדי retention hot/cold.
3. **Commitment:** Sequencers של Nexus כוללים התחייבויות blobs (CID + roots KZG
   אופציונליים) בבלוק הקנוני. לקוחות קלים מסתמכים על hash ההתחייבות וה-metadata
   המוצהרת כדי לאמת availability.
4. **שכפול:** צמתי אחסון מושכים shares/chunks שהוקצו, עומדים באתגרי PDP/PoTR,
   ומקדמים נתונים בין tiers חמים וקרים לפי המדיניות.
5. **שליפה:** צרכנים שולפים נתונים דרך SoraFS או שערים מודעי-DA, מאמתים proofs
   ומגישים בקשות תיקון כאשר replicas נעלמות.
6. **ממשל:** הפרלמנט ווועדת הפיקוח DA מאשרים מפעילים, לוחות rent והסלמות
   enforcement. ארטיפקטי ממשל מאוחסנים באותו מסלול DA כדי להבטיח שקיפות.

## נכסים ובעלים

סולם השפעה: **קריטי** שובר בטיחות/חיוניות של ledger; **גבוה** חוסם backfill
DA או לקוחות; **בינוני** מוריד איכות אך נשאר בר-שחזור; **נמוך** השפעה מוגבלת.

| נכס | תיאור | שלמות | זמינות | סודיות | Owner |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (chunks + manifests) | Blobs של Taikai, lane וממשל ב-SoraFS | קריטי | קריטי | בינוני | DA WG / Storage Team |
| Manifests Norito DA | Metadata טיפוסית שמתארת blobs | קריטי | גבוה | בינוני | Core Protocol WG |
| התחייבויות בלוק | CIDs + roots KZG בבלוקים של Nexus | קריטי | גבוה | נמוך | Core Protocol WG |
| לוחות PDP/PoTR | קצב אכיפה עבור replicas של DA | גבוה | גבוה | נמוך | Storage Team |
| Registry מפעילים | ספקי אחסון מאושרים ומדיניות | גבוה | גבוה | נמוך | Governance Council |
| רשומות rent ותמריצים | רישומי ledger ל-rent DA וקנסות | גבוה | בינוני | נמוך | Treasury WG |
| Dashboards observability | SLOs DA, עומק שכפול, התראות | בינוני | גבוה | נמוך | SRE / Observability |
| Intents תיקון | בקשות לשחזור chunks חסרים | בינוני | בינוני | נמוך | Storage Team |

## יריבים ויכולות

| שחקן | יכולות | מניעים | הערות |
| --- | --- | --- | --- |
| לקוח זדוני | שולח blobs פגומים, replay של manifests ישנים, מנסה DoS על ingest. | לשבש שידורי Taikai, להזריק נתונים לא תקינים. | ללא מפתחות מיוחדים. |
| צומת אחסון ביזנטי | מפיל replicas שהוקצו, מזייף proofs PDP/PoTR, משתף פעולה. | לקצר retention של DA, להימנע מ-rent, להחזיק נתונים כבני ערובה. | מחזיק אישורי מפעיל תקפים. |
| Sequencer שנפרץ | משמיט commitments, מבצע equivocation על בלוקים, מסדר מחדש metadata. | להסתיר submissions של DA, ליצור חוסר עקביות. | מוגבל ע"י רוב קונצנזוס. |
| מפעיל פנימי | מנצל גישת ממשל, מתעסק במדיניות retention, מדליף credentials. | רווח כלכלי, חבלה. | גישה לתשתיות hot/cold. |
| יריב רשת | מבצע partition, מעכב replication, מזריק תעבורת MITM. | להוריד availability, לפגוע ב-SLOs. | לא יכול לשבור TLS אך יכול להאט/להפיל קישורים. |
| תוקף observability | מטמפר dashboards/alerts, מסתיר תקריות. | להסתיר outages של DA. | דורש גישה לצינור telemetry. |

## גבולות אמון

- **Ingress boundary:** לקוח אל הרחבת DA ב-Torii. דורש אימות לכל בקשה,
  rate limiting ואימות payload.
- **Replication boundary:** צמתי אחסון מחליפים chunks ו-proofs. הצמתים מאומתים
  הדדית אך עשויים להתנהג ביזנטית.
- **Ledger boundary:** נתוני בלוק מחויבים לעומת אחסון מחוץ לשרשרת. הקונצנזוס
  שומר על שלמות, אבל availability דורש אכיפה off-chain.
- **Governance boundary:** החלטות Council/Parliament המאשרות מפעילים, תקציבים
  וסנקציות. פריצה כאן פוגעת ישירות בפריסת DA.
- **Observability boundary:** איסוף metrics/logs שמיוצא ל-dashboard/alert tooling.
  Tampering מסתיר outages או התקפות.

## תרחישי איום ובקרות

### תקיפות במסלול ingest

**תרחיש:** לקוח זדוני שולח payloads Norito פגומים או blobs גדולים מדי כדי לרוקן
משאבים או להבריח metadata לא תקינה.

**בקרות**
- אימות schema של Norito עם משא ומתן גרסאות מחמיר; דחיית flags לא מוכרים.
- Rate limiting ואימות ב-endpoint ingest של Torii.
- גבולות chunk size ו-encoding דטרמיניסטי נאכפים ע"י chunker של SoraFS.
- Pipeline admission שומר manifests רק לאחר התאמת checksum של שלמות.
- Replay cache דטרמיניסטי (`ReplayCache`) עוקב אחר חלונות `(lane, epoch,
  sequence)`, שומר high-water marks בדיסק, ודוחה duplications/replays ישנים;
  harnesses של property ו-fuzz מכסים fingerprints שונים והגשות out-of-order.
  [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**פערים שיוריים**
- Torii ingest חייב לשלב את replay cache ב-admission ולשמר cursors של sequence
  בין אתחולים.
- סכמות Norito DA כוללות כעת fuzz harness ייעודי (`fuzz/da_ingest_schema.rs`) כדי
  ללחוץ על invariants של encode/decode; dashboards כיסוי צריכים להתריע על regress.

### withholding של שכפול

**תרחיש:** מפעילי אחסון ביזנטיים מקבלים pin assignments אך מפילים chunks, ועוברים
אתגרי PDP/PoTR באמצעות תגובות מזויפות או collusion.

**בקרות**
- לוח האתגרים PDP/PoTR מורחב ל-payloads של DA עם כיסוי לכל epoch.
- שכפול multi-source עם ספי quorum; orchestrator מזהה shards חסרים ומפעיל repair.
- slashing של ממשל קשור ל-proofs כושלים ול-replicas חסרים.
- Job reconciliation אוטומטי (`cargo xtask da-commitment-reconcile`) משווה receipts
  של ingest עם commitments DA (SignedBlockWire, `.norito`, או JSON), מפיק bundle
  JSON לראיות ממשל, ונכשל על tickets חסרים/לא תואמים כדי ש-Alertmanager יזמן.

**פערים שיוריים**
- harness הסימולציה ב-`integration_tests/src/da/pdp_potr.rs` (מכוסה ע"י
  `integration_tests/tests/da/pdp_potr_simulation.rs`) מפעיל תרחישי collusion
  ו-partition, ומאמת שה-schedule של PDP/PoTR מזהה התנהגות ביזנטית באופן
  דטרמיניסטי. המשיכו להרחיב יחד עם DA-5 לכיסוי משטחי proof חדשים.
- מדיניות eviction של tier cold דורשת audit trail חתום כדי למנוע drops סמויים.

### Tampering של commitments

**תרחיש:** Sequencer שנפרץ מפרסם בלוקים שמדלגים או משנים commitments של DA, מה
שמוביל לכשלי fetch או אי-עקביות light-client.

**בקרות**
- הקונצנזוס מבצע cross-check בין הצעות בלוק לתורי submissions של DA; peers
  דוחים הצעות שחסרות commitments נחוצים.
- light clients מאמתים inclusion proofs לפני חשיפת handles ל-fetch.
- Audit trail המשווה receipts של submission עם commitments של בלוקים.
- Job reconciliation אוטומטי (`cargo xtask da-commitment-reconcile`) משווה receipts
  של ingest עם commitments DA (SignedBlockWire, `.norito`, או JSON), מפיק bundle
  JSON לראיות ממשל, ונכשל על tickets חסרים/לא תואמים כדי ש-Alertmanager יזמן.

**פערים שיוריים**
- מכוסה ע"י job reconciliation + hook של Alertmanager; חבילות ממשל צורכות כעת
  bundle JSON של ראיות כברירת מחדל.

### Partition ברשת וצנזורה

**תרחיש:** יריב מבצע partition לרשת השכפול, מונע מצמתים לקבל chunks שהוקצו או
להגיב לאתגרי PDP/PoTR.

**בקרות**
- דרישות multi-region ל-providers מבטיחות מסלולי רשת מגוונים.
- חלונות challenge כוללים jitter ו-fallback לערוצי repair out-of-band.
- Dashboards observability עוקבים אחרי עומק שכפול, הצלחת challenge, ו-latency
  של fetch עם ספי התרעה.

**פערים שיוריים**
- סימולציות partition לאירועי Taikai live עדיין חסרות; דרושים soak tests.
- מדיניות reserve של רוחב פס לתיקון עדיין לא מקודדת.

### שימוש לרעה פנימי

**תרחיש:** מפעיל עם גישת registry משנה מדיניות retention, מלבין providers
זדוניים, או משתיק התראות.

**בקרות**
- פעולות ממשל דורשות חתימות multi-party ורשומות Norito מנוטרות.
- שינויי מדיניות מפיקים events ל-monitoring ול-archive logs.
- Pipeline observability אוכף Norito logs append-only עם hash chaining.
- אוטומציה של ביקורת גישה רבעונית (`cargo xtask da-privilege-audit`) סורקת
  תיקיות manifest/replay (ובתוספת מסלולים שמפעילים מספקים), מסמנת entries
  חסרים/לא תיקייה/world-writable, ומפיקה bundle JSON חתום לדשבורדים.

**פערים שיוריים**
- Tamper-evidence עבור dashboards דורש snapshots חתומים.

## רישום סיכונים שיוריים

| סיכון | הסתברות | השפעה | Owner | תוכנית מיתון |
| --- | --- | --- | --- | --- |
| Replay של manifests DA לפני שנחת sequence cache של DA-2 | אפשרי | בינוני | Core Protocol WG | ליישם sequence cache + nonce validation ב-DA-2; להוסיף בדיקות רגרסיה. |
| Collusion PDP/PoTR כאשר >f nodes נפגעים | לא סביר | גבוה | Storage Team | לגזור schedule אתגר חדש עם sampling cross-provider; לאמת באמצעות harness סימולציה. |
| פער audit ל-eviction של cold tier | אפשרי | גבוה | SRE / Storage Team | לצרף logs חתומים ו-receipts on-chain ל-evictions; לנטר בדשבורדים. |
| Latency לזיהוי השמטת sequencer | אפשרי | גבוה | Core Protocol WG | `cargo xtask da-commitment-reconcile` לילי משווה receipts מול commitments (SignedBlockWire/`.norito`/JSON) ומדווח לממשל על tickets חסרים או לא תואמים. |
| עמידות partition לשידורי Taikai live | אפשרי | קריטי | Networking TL | להריץ drills של partition; לשמור רוחב פס לתיקון; לתעד SOP failover. |
| Drift בהרשאות ממשל | לא סביר | גבוה | Governance Council | `cargo xtask da-privilege-audit` רבעוני (dirs manifest/replay + paths נוספים) עם JSON חתום + gate בדשבורד; לעגן artefacts audit ב-chain. |

## Follow-ups נדרשים

1. לפרסם schemas Norito ל-ingest DA ודוגמאות וקטורים (נישא ל-DA-2).
2. לשלב replay cache ב-ingest של Torii DA ולשמר cursors של sequence בין אתחולים.
3. **הושלם (2026-02-05):** harness הסימולציה PDP/PoTR מפעיל כעת collusion +
   partition עם מודל backlog QoS; ראו `integration_tests/src/da/pdp_potr.rs`
   (עם tests ב-`integration_tests/tests/da/pdp_potr_simulation.rs`) עבור
   המימוש וסיכומים דטרמיניסטיים שנקלטו למטה.
4. **הושלם (2026-05-29):** `cargo xtask da-commitment-reconcile` משווה receipts
   של ingest מול commitments DA (SignedBlockWire/`.norito`/JSON), מפיק
   `artifacts/da/commitment_reconciliation.json`, ומחובר ל-Alertmanager/
   חבילות ממשל להתראות omission/tampering (`xtask/src/da.rs`).
5. **הושלם (2026-05-29):** `cargo xtask da-privilege-audit` סורק spool של
   manifest/replay (ובתוספת paths שמפעילים מספקים), מסמן entries חסרים/לא
   תיקייה/world-writable, ומפיק bundle JSON חתום לדשבורדים/סקירות ממשל
   (`artifacts/da/privilege_audit.json`), סוגר את פער האוטומציה של ביקורת הגישה.

**איפה להמשיך:**

- replay cache והתמדת cursors הוטמעו ב-DA-2. ראו את המימוש ב-
  `crates/iroha_core/src/da/replay_cache.rs` (לוגיקת cache) ואת אינטגרציית Torii
  ב-`crates/iroha_torii/src/da/ingest.rs`, שמחילה בדיקות fingerprint דרך `/v2/da/ingest`.
- סימולציות streaming של PDP/PoTR מופעלות דרך harness proof-stream ב-
  `crates/sorafs_car/tests/sorafs_cli.rs`, ומכסות זרימות בקשה PoR/PDP/PoTR
  ותרחישי כשל שמוצגים במודל האיומים.
- תוצאות capacity ו-repair soak נמצאות תחת
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, בעוד שמטריצת soak של
  Sumeragi הרחבה מתועדת ב-`docs/source/sumeragi_soak_matrix.md` (כולל גרסאות
  מתורגמות). ארטיפקטים אלו מתעדים את ה-drills הארוכים שמוזכרים ברישום הסיכונים.
- אוטומציית reconciliation + privilege-audit נמצאת ב-`docs/automation/da/README.md`
  ובפקודות החדשות `cargo xtask da-commitment-reconcile` /
  `cargo xtask da-privilege-audit`; השתמשו בפלטי ברירת המחדל תחת `artifacts/da/`
  בעת צירוף ראיות לחבילות ממשל.

## ראיות סימולציה ומידול QoS (2026-02)

כדי לסגור את follow-up DA-1 #3, קידדנו harness סימולציה דטרמיניסטי ל-PDP/PoTR
תחת `integration_tests/src/da/pdp_potr.rs` (מכוסה ע"י
`integration_tests/tests/da/pdp_potr_simulation.rs`). ה-harness מקצה nodes
לשלושה אזורים, מזריק partition/collusion לפי הסתברויות ה-roadmap, עוקב אחרי
lateness של PoTR, ומזין מודל backlog של repair שמשקף את תקציב התיקון של tier
hot. הרצת תרחיש ברירת המחדל (12 epochs, 18 אתגרי PDP + 2 חלונות PoTR לכל epoch)
הניבה את המטריקות הבאות:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Metric | Value | Notes |
| --- | --- | --- |
| PDP failures detected | 48 / 49 (98.0%) | Partitions עדיין מפעילות זיהוי; כשל אחד שלא זוהה נובע מ-jitter תקין. |
| PDP mean detection latency | 0.0 epochs | הכשלים נחשפים בתוך epoch המקורי. |
| PoTR failures detected | 28 / 77 (36.4%) | הזיהוי מופעל כאשר node מפספס >=2 חלונות PoTR, מה שמשאיר את רוב האירועים ברישום הסיכון השיורי. |
| PoTR mean detection latency | 2.0 epochs | תואם את סף האיחור של שני epochs המוטמע בהסלמת הארכוב. |
| Repair queue peak | 38 manifests | ה-backlog קופץ כאשר partitions נערמות מהר יותר מארבעה תיקונים לכל epoch. |
| Response latency p95 | 30,068 ms | משקף חלון אתגר של 30 שניות עם jitter של +/-75 ms המוחל על דגימת QoS. |
<!-- END_DA_SIM_TABLE -->

הפלטים הללו מזינים כעת פרוטוטיפים של dashboards DA ומספקים את קריטריוני
הקבלה של "simulation harness + QoS modelling" שמופיעים ב-roadmap.

האוטומציה נמצאת מאחורי
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
שקורא ל-harness המשותף ומפיק Norito JSON אל `artifacts/da/threat_model_report.json`
כברירת מחדל. Jobs ליליים צורכים קובץ זה כדי לרענן את המטריצות במסמך זה ולהתריע
על סטיות בשיעורי זיהוי, תורי repair או דגימות QoS.

כדי לרענן את הטבלה לעיל עבור docs, הריצו `make docs-da-threat-model`, שמפעיל
`cargo xtask da-threat-model-report`, מייצר מחדש
`docs/source/da/_generated/threat_model_report.json`, ומשכתב את הסעיף הזה דרך
`scripts/docs/render_da_threat_model_tables.py`. המראה `docs/portal`
(`docs/portal/docs/da/threat-model.md`) מתעדכן באותו מעבר כדי ששני העותקים
יישארו מסונכרנים.
