---
lang: he
direction: rtl
source: docs/portal/docs/da/threat-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
ہونے تک دونوں ورژنز کو sync رکھیں۔
:::

# Sora Nexus מודל איום זמינות נתונים

_آخری جائزہ: 2026-01-19 -- اگلا شیڈول شدہ جائزہ: 2026-04-19_

קצב תחזוקה: קבוצת עבודה זמינות נתונים (<=90 ימים). ہر ریویژن
`status.md` میں لازمی درج ہو، اور اس میں active mitigation tickets اور
simulation artefacts کے روابط شامل ہوں۔

## مقصد اور دائرہ کار

זמינות נתונים (DA) שידורי Taikai, Nexus כתמי נתיב, או ממשל
artefacts کو Byzantine، نیٹ ورک، اور آپریٹر کی خرابیوں میں بھی قابل بازیافت
رکھتا ہے۔ یہ threat model DA-1 (architecture اور threat model) کیلئے انجینئرنگ
کام کی بنیاد ہے اور downstream DA tasks (DA-2 تا DA-10) کیلئے baseline ہے۔

רכיבים בהיקף:
- Torii DA הרחבת הטמעה או Norito כותבי מטא נתונים.
- SoraFS בגיבוי עצי אחסון כתמים (שכבות חמות/קרה) או מדיניות שכפול.
- התחייבויות לחסום Nexus (פורמטים של חוטים, הוכחות, ממשקי API של לקוח קל).
- PDP/PoTR enforcement hooks جو DA payloads کیلئے مخصوص ہیں.
- זרימות עבודה של מפעילים (הצמדה, פינוי, חיתוך) או צינורות צפייה.
- Governance approvals جو DA operators اور content کو admit یا evict کرتے ہیں۔

מחוץ לתחום של מסמך זה:
- مکمل economics modelling (DA-7 workstream میں).
- SoraFS base protocols جو پہلے سے SoraFS threat model میں شامل ہیں.
- ארגונומיה של לקוח SDK מעבר לשיקולי משטח האיום.

## סקירה אדריכלית

1. **Submission:** Clients blobs کو Torii DA ingest API کے ذریعے submit کرتے ہیں۔
   Node blobs کو chunks میں بانٹتا ہے، Norito manifests encode کرتا ہے (blob type,
   lane, epoch, codec flags)، اور chunks کو hot SoraFS tier میں محفوظ کرتا ہے۔
2. **פרסומת:** כוונות סיכות או רמזים לשכפול רישום (שוק SoraFS)
   کے ذریعے storage providers تک جاتے ہیں، policy tags کے ساتھ جو hot/cold retention
   targets بتاتے ہیں۔
3. **מחויבות:** Nexus רצפי רצף התחייבויות (CID + שורשי KZG אופציונליים)
   canonical block میں شامل کرتے ہیں۔ התחייבות קלה של לקוחות פורסם
   metadata کی بنیاد پر availability verify کرتے ہیں۔
4. **Replication:** Storage nodes assigned shares/chunks کھینچتے ہیں، PDP/PoTR
   challenges مکمل کرتے ہیں، اور policy کے مطابق hot/cold tiers کے درمیان data
   promote کرتے ہیں۔
5. **Fetch:** Consumers SoraFS یا DA-aware gateways کے ذریعے data fetch کرتے ہیں،
   proofs verify کرتے ہیں، اور replicas غائب ہونے پر repair requests اٹھاتے ہیں۔
6. **ממשל:** מפעילי ועדת הפיקוח של הפרלמנט או דא, לוחות זמנים להשכרה,
   اور enforcement escalations کو approve کرتی ہے۔ Governance artefacts اسی DA
   path سے گزرتے ہیں تاکہ transparency برقرار رہے۔

## נכסים ובעלים

Impact scale: **Critical** ledger safety/liveness توڑتا ہے؛ **גבוה** DA
یا clients کو بلاک کرتا ہے؛ **Moderate** quality کم کرتا ہے مگر recoverable؛
**Low** محدود اثر۔| נכס | תיאור | יושרה | זמינות | סודיות | בעלים |
| --- | --- | --- | --- | --- | --- |
| כתמי DA (נתחים + מניפסטים) | Taikai, ליין, כתמי ממשל ב-SoraFS | קריטי | קריטי | מתון | DA WG / צוות אחסון |
| מניפסטים של Norito DA | מטא נתונים מוקלדים המתארים כתמים | קריטי | גבוה | מתון | Core Protocol WG |
| חסימת התחייבויות | CID + שורשי KZG בתוך בלוקים Nexus | קריטי | גבוה | נמוך | Core Protocol WG |
| לוחות זמנים של PDP/PoTR | קצב אכיפה עבור העתקים של DA | גבוה | גבוה | נמוך | צוות אחסון |
| רישום מפעילים | ספקי אחסון ומדיניות מאושרים | גבוה | גבוה | נמוך | מועצת ממשל |
| רישומי שכירות ותמריצים | רישומי פנקס עבור דמי שכירות וקנסות | גבוה | מתון | נמוך | משרד האוצר |
| לוחות מחוונים של צפייה | SLOs DA, עומק שכפול, התראות | מתון | גבוה | נמוך | SRE / צפיות |
| כוונות תיקון | בקשות להחדרת נוזלים של נתחים חסרים | מתון | מתון | נמוך | צוות אחסון |

## יריבים ויכולות

| שחקן | יכולות | מניעים | הערות |
| --- | --- | --- | --- |
| לקוח זדוני | Malformed blobs submit کرنا، stale manifests replay کرنا، ingest پر DoS کی کوشش۔ | Taikai broadcasts disrupt کرنا، invalid data inject کرنا۔ | Privileged keys نہیں۔ |
| צומת אחסון ביזנטי | Assigned replicas drop کرنا، PDP/PoTR proofs forge کرنا، collude کرنا۔ | DA retention کم کرنا، rent سے بچنا، data hostage بنانا۔ | Valid operator credentials رکھتا ہے۔ |
| סיקוונסר שנפגע | Commitments omit کرنا، blocks پر equivocate کرنا، metadata reorder کرنا۔ | DA submissions چھپانا، inconsistency پیدا کرنا۔ | Consensus majority سے محدود۔ |
| מפעיל פנימי | Governance access abuse کرنا، retention policies tamper کرنا، credentials leak کرنا۔ | רווח כלכלי, חבלה. | Hot/cold tier infra تک رسائی۔ |
| יריב רשת | Nodes partition کرنا، replication delay کرنا، MITM traffic inject کرنا۔ | Availability کم کرنا، SLOs degrade کرنا۔ | TLS نہیں توڑ سکتا مگر links slow/drop کر سکتا ہے۔ |
| תוקף צפיות | Dashboards/alerts tamper کرنا، incidents suppress کرنا۔ | DA outages چھپانا۔ | Telemetry pipeline تک رسائی درکار۔ |

## גבולות אמון

- **Ingress boundary:** Client سے Torii DA extension۔ אישור ברמת הבקשה, הגבלת שיעור,
  اور payload validation درکار ہیں۔
- **Replication boundary:** Storage nodes chunks اور proofs exchange کرتے ہیں۔ צמתים
  باہمی authenticated ہیں مگر Byzantine برتاؤ ممکن ہے۔
- **Ledger boundary:** Committed block data بمقابلہ off-chain storage۔ קונצנזוס
  integrity guard کرتا ہے، مگر availability کیلئے off-chain enforcement ضروری ہے۔
- **Governance boundary:** Council/Parliament کے فیصلے operators، budgets، slashing
  approve کرتے ہیں۔ یہاں خرابی DA deployment کو براہ راست متاثر کرتی ہے۔
- **Observability boundary:** Metrics/log collection کا dashboards/alerts tooling
  کو export ہونا۔ Tampering outages یا attacks چھپا سکتا ہے۔

## תרחישי איום ובקרות

### הטמעת התקפות נתיב**Scenario:** Malicious client malformed Norito payloads یا oversized blobs submit
کرتا ہے تاکہ resources exhaust ہوں یا invalid metadata شامل ہو۔

**בקרות**
- אימות סכימה Norito עם משא ומתן קפדני על גרסאות; דגלים לא ידועים דוחים.
- Torii ingest endpoint پر rate limiting اور authentication۔
- SoraFS chunker کے ذریعے chunk size bounds اور deterministic encoding۔
- Admission pipeline صرف integrity checksum match ہونے کے بعد manifests persist کرے۔
- מטמון שידור חוזר דטרמיניסטי (`ReplayCache`) `(lane, epoch, sequence)` מסלול Windows
  کرتا ہے، high-water marks disk پر persist کرتا ہے، اور duplicates/stale replays
  reject کرتا ہے؛ property اور fuzz harnesses divergent fingerprints اور out-of-order
  submissions cover کرتے ہیں۔ [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**פערים שיוריים**
- Torii להטמיע צילום חוזר זיכרון מטמון סמני רצף
  restarts کے پار persist کرنا ضروری ہے۔
- Norito DA schemas کے لئے dedicated fuzz harness (`fuzz/da_ingest_schema.rs`) موجود
  20 coverage dashboards کو regression پر alert کرنا چاہئے۔

### מניעת שכפול

**Scenario:** Byzantine storage operators pin assignments قبول کرتے ہیں مگر chunks





drop کرتے ہیں، forged responses یا collusion سے PDP/PoTR challenges pass کرتے ہیں۔

**בקרות**
- PDP/PoTR challenge schedule DA payloads تک extend ہے اور per-epoch coverage دیتا ہے۔
- שכפול ריבוי מקורות עם ספי מניין; להביא רסיסי מתזמר חסרים
  detect کر کے repair trigger کرتا ہے۔
- Governance slashing failed proofs اور missing replicas سے linked ہے۔
- משימת התאמה אוטומטית (`cargo xtask da-commitment-reconcile`) הטמעת קבלות
  کو DA commitments (SignedBlockWire/`.norito`/JSON) سے compare کرتا ہے، governance
  کیلئے JSON evidence bundle emit کرتا ہے، اور missing/mismatched tickets پر fail
  ہو کر Alertmanager کو page کرنے دیتا ہے۔

**פערים שיוריים**
- `integration_tests/src/da/pdp_potr.rs` רתמת סימולציה (בדיקות:
  `integration_tests/tests/da/pdp_potr_simulation.rs`) קנוניה או מחיצה
  scenarios چلاتا ہے؛ DA-5 کے ساتھ اسے نئی proof surfaces کیلئے مزید بڑھائیں۔
- Cold-tier eviction policy کیلئے signed audit trail درکار ہے تاکہ covert drops
  روکے جا سکیں۔

### שיבוש מחויבות

**Scenario:** Compromised sequencer DA commitments omit یا alter کرتا ہے، جس سے
fetch failures یا light-client inconsistencies پیدا ہوتی ہیں۔

**בקרות**
- Consensus block proposals کو DA submission queues سے cross-check کرتا ہے؛ עמיתים
  missing commitments والی proposals reject کرتے ہیں۔
- Light clients inclusion proofs verify کرتے ہیں قبل از fetch handles۔
- Submission receipts اور block commitments کا audit trail۔
- עבודת התאמה אוטומטית (`cargo xtask da-commitment-reconcile`) הטמעת קבלות
  کو commitments سے compare کرتا ہے، governance کیلئے JSON evidence bundle emit کرتا ہے،
  اور missing/mismatched tickets پر Alertmanager page ہوتا ہے۔**פערים שיוריים**
- Reconciliation job + Alertmanager hook سے cover؛ מנות משילות ברירת מחדל
  JSON evidence bundle ingest کرتے ہیں۔

### מחיצת רשת וצנזורה

**Scenario:** Adversary replication network partition کرتا ہے، جس سے nodes assigned
chunks حاصل نہیں کر پاتے یا PDP/PoTR challenges کا جواب نہیں دے پاتے۔

**בקרות**
- Multi-region provider requirements diverse network paths یقینی بناتے ہیں۔
- Challenge windows میں jitter اور out-of-band repair channel fallback شامل ہے۔
- עומק שכפול של לוחות תצפית, הצלחה באתגר, אחזור זמן אחזור
  alert thresholds کے ساتھ monitor کرتے ہیں۔

**פערים שיוריים**
- Taikai live events کیلئے partition simulations ابھی نہیں؛ soak tests ضروری ہیں۔
- Repair bandwidth reservation policy ابھی codified نہیں۔

### התעללות מבפנים

**תרחיש:** גישה לרישום ומדיניות שימור מפעיל מפעילה מניפולציה
malicious providers کو whitelist کرتا ہے، یا alerts suppress کرتا ہے۔

**בקרות**
- Governance actions multi-party signatures اور Norito-notarised records مانگتی ہیں۔
- Policy changes monitoring اور archival logs کو events بھیجتی ہیں۔
- Observability pipeline append-only Norito logs with hash chaining enforce کرتا ہے۔
- מניפסט/שידור חוזר של סקירת גישה רבעונית (`cargo xtask da-privilege-audit`).
  dirs (اور operator-supplied paths) scan کرتا ہے، missing/non-directory/world-writable
  entries flag کرتا ہے، اور signed JSON bundle dashboards کیلئے emit کرتا ہے۔

**פערים שיוריים**
- Dashboard tamper-evidence کیلئے signed snapshots درکار ہیں۔

## רישום סיכונים שיוריים

| סיכון | סבירות | השפעה | בעלים | תוכנית הפחתה |
| --- | --- | --- | --- | --- |
| DA-2 sequence cache سے پہلے DA manifests replay | אפשרי | מתון | Core Protocol WG | DA-2 میں sequence cache + nonce validation implement کریں؛ regression tests شامل کریں۔ |
| > צמתים f מתפשרים על שיתוף פעולה של PDP/PoTR | לא סביר | גבוה | צוות אחסון | Cross-provider sampling کے ساتھ نیا challenge schedule derive کریں؛ simulation harness سے validate کریں۔ |
| פער ביקורת פינוי בשכבה קרה | אפשרי | גבוה | SRE / צוות אחסון | Evictions کیلئے signed audit logs + on-chain receipts attach کریں؛ dashboards سے monitor کریں۔ |
| השהיה לגילוי השמטת רצף | אפשרי | גבוה | Core Protocol WG | Nightly `cargo xtask da-commitment-reconcile` receipts vs commitments (SignedBlockWire/`.norito`/JSON) compare کر کے governance کو page کرے۔ |
| Taikai live streams کیلئے partition resilience | אפשרי | קריטי | רשתות TL | Partition drills چلائیں؛ repair bandwidth reserve کریں؛ failover SOP document کریں۔ |
| סחף פריבילגיות ממשל | לא סביר | גבוה | מועצת ממשל | רבעוני `cargo xtask da-privilege-audit` (מניפסט/שידור חוזר + נתיבים נוספים) עם JSON חתום + שער לוח המחוונים; audit artefacts کو on-chain anchor کریں۔ |

## מעקבים נדרשים1. DA ingest Norito schemas اور example vectors publish کریں (DA-2 میں لے جائیں)۔
2. Replay cache کو Torii DA ingest میں thread کریں اور sequence cursors restarts
   کے پار persist کریں۔
3. **Completed (2026-02-05):** PDP/PoTR simulation harness اب collusion + partition
   scenarios اور QoS backlog modelling exercises کرتا ہے؛ دیکھیں
   `integration_tests/src/da/pdp_potr.rs` (בדיקות: `integration_tests/tests/da/pdp_potr_simulation.rs`)
4. **הושלם (2026-05-29):** `cargo xtask da-commitment-reconcile` קבלות
   کو DA commitments (SignedBlockWire/`.norito`/JSON) سے compare کر کے
   `artifacts/da/commitment_reconciliation.json` emit کرتا ہے اور Alertmanager/
   governance packets کیلئے wired ہے (`xtask/src/da.rs`)۔
5. **הושלם (2026-05-29):** `cargo xtask da-privilege-audit` מניפסט/משחק חוזר
   (اور operator-supplied paths) walk کرتا ہے، missing/non-directory/world-writable
   entries flag کرتا ہے، اور signed JSON bundle governance dashboards کیلئے بناتا
   ہے (`artifacts/da/privilege_audit.json`)۔

**היכן לחפש אחר כך:**

- DA-2 میں replay cache اور cursor persistence آچکی ہے۔ Implementation دیکھیں
  שילוב `crates/iroha_core/src/da/replay_cache.rs` (לוגיקת מטמון) אור Torii
  `crates/iroha_torii/src/da/ingest.rs` میں، جو `/v1/da/ingest` کے ذریعے fingerprint
  checks thread کرتا ہے۔
- רתמת הוכחת זרם של PDP/PoTR זרימת סימולציות.
  `crates/sorafs_car/tests/sorafs_cli.rs`. یہ PoR/PDP/PoTR request flows اور
  failure scenarios cover کرتا ہے جو threat model میں بیان ہیں۔
- Capacity اور repair soak کے نتائج `docs/source/sorafs/reports/sf2c_capacity_soak.md`
  میں ہیں، جبکہ Sumeragi soak matrix `docs/source/sumeragi_soak_matrix.md` میں ہے
  (localized variants شامل ہیں)۔ یہ artefacts residual risk register کے drills
  کو capture کرتے ہیں۔
- פיוס + אוטומציה של הרשאות ביקורת `docs/automation/da/README.md` אוט
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit` میں ہے؛
  governance packets کیلئے evidence attach کرتے وقت `artifacts/da/` کی default
  outputs استعمال کریں۔

## עדויות סימולציה ומודלים של QoS (2026-02)

DA-1 follow-up #3 مکمل کرنے کیلئے ہم نے `integration_tests/src/da/pdp_potr.rs`
( `integration_tests/tests/da/pdp_potr_simulation.rs` سے covered ) میں


deterministic PDP/PoTR simulation harness شامل کیا۔ یہ harness nodes کو تین regions
میں allocate کرتا ہے، roadmap probabilities کے مطابق partitions/collusion inject
کرتا ہے، PoTR lateness track کرتا ہے، اور repair-backlog model کو feed کرتا ہے جو
hot-tier repair budget کی عکاسی کرتا ہے۔ תרחיש ברירת מחדל (12 עידנים, 18 PDP
challenges + 2 PoTR windows per epoch) سے یہ metrics نکلے:<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| מדד | ערך | הערות |
| --- | --- | --- |
| זוהו כשלים ב-PDP | 48 / 49 (98.0%) | Partitions اب بھی detection trigger کرتے ہیں؛ ایک undetected failure honest jitter سے ہے۔ |
| PDP ממוצע זמן זיהוי | 0.0 עידנים | Failures originating epoch کے اندر ظاہر ہوتے ہیں۔ |
| זוהו כשלים ב-PoTR | 28 / 77 (36.4%) | Detection تب trigger ہوتی ہے جب node >=2 PoTR windows miss کرے، زیادہ تر واقعات residual-risk register میں رہتے ہیں۔ |
| PoTR ממוצע חביון זיהוי | 2.0 עידנים | Archival escalation میں شامل دو-epoch lateness threshold سے match کرتا ہے۔ |
| שיא תור לתיקון | 38 מניפסטים | Backlog تب بڑھتا ہے جب partitions چار repairs/epoch سے تیز جمع ہوں۔ |
| השהיית תגובה p95 | 30,068 אלפיות השנייה | 30 s challenge window اور QoS sampling کیلئے +/-75 ms jitter کو reflect کرتا ہے۔ |
<!-- END_DA_SIM_TABLE -->

یہ outputs اب DA dashboard prototypes کو drive کرتے ہیں اور roadmap میں حوالہ
دئے گئے "simulation harness + QoS modelling" acceptance criteria پورے کرتے ہیں۔

אוטומציה של `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
کے پیچھے ہے، جو shared harness کو call کرتا ہے اور default طور پر Norito JSON
`artifacts/da/threat_model_report.json` میں emit کرتا ہے۔ עבודות לילה 20 קובץ
consume کر کے document matrices refresh کرتی ہیں اور detection rates، repair
queues، یا QoS samples میں drift پر alert دیتی ہیں۔

Docs کیلئے اوپر کی table refresh کرنے کو `make docs-da-threat-model` چلائیں، جو
`cargo xtask da-threat-model-report` invoke کرتا ہے،
`docs/source/da/_generated/threat_model_report.json` regenerate کرتا ہے، اور
`scripts/docs/render_da_threat_model_tables.py` کے ذریعے یہ section rewrite کرتا
ہے۔ `docs/portal` mirror (`docs/portal/docs/da/threat-model.md`) بھی اسی pass میں
update ہوتا ہے تاکہ دونوں copies sync رہیں۔