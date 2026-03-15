---
lang: he
direction: rtl
source: docs/portal/docs/sns/governance-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sns/governance_playbook.md` کی عکاسی کرتا ہے اور اب پورٹل
کی کینونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

# Sora Name Service گورننس پلی بک (SN-6)

**حالت:** 2026-03-24 مسودہ - SN-1/SN-6 تیاری کے لئے زندہ حوالہ  
**روڈمیپ لنکس:** SN-6 "Compliance & Dispute Resolution", SN-7 "Resolver & Gateway Sync", ADDR-1/ADDR-5 ایڈریس پالیسی  
**پیشگی شرائط:** رجسٹری اسکیمہ [`registry-schema.md`](./registry-schema.md) میں، رجسٹرار API معاہدہ [`registrar-api.md`](./registrar-api.md) میں، ایڈریس UX رہنمائی [`address-display-guidelines.md`](./address-display-guidelines.md) میں، اور اکاؤنٹ اسٹرکچر قواعد [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md) میں۔

یہ پلی بک بتاتی ہے کہ Sora Name Service (SNS) کی گورننس باڈیز کیسے چارٹر اپناتی
ہیں، رجسٹریشن منظور کرتی ہیں، تنازعات کو escalate کرتی ہیں، اور ثابت کرتی ہیں کہ
resolver اور gateway کی حالتیں ہم آہنگ رہتی ہیں۔ یہ روڈمیپ کی اس ضرورت کو پورا
کرتی ہے کہ `sns governance ...` CLI، Norito manifests اور audit artefacts N1
(عوامی لانچ) سے پہلے ایک ہی آپریٹر ریفرنس شیئر کریں۔

## 1. دائرہ کار اور سامعین

یہ دستاویز ان کے لئے ہے:

- Governance Council کے اراکین جو چارٹرز، suffix پالیسیوں اور تنازعہ نتائج پر ووٹ دیتے ہیں۔
- Guardian board کے اراکین جو ہنگامی freezes جاری کرتے ہیں اور reversals کا جائزہ لیتے ہیں۔
- סיומת דיילים ורשם חשבונות מכירות פומביות מכירות פומביות או פיצולי הכנסות
- Resolver/gateway آپریٹرز جو SoraDNS پھیلاؤ، GAR اپڈیٹس اور telemetry guardrails کے ذمہ دار ہیں۔
- תאימות, תמיכה אוצר אוצר. artefacts چھوڑے۔

یہ `roadmap.md` میں درج بند-بیٹا (N0)، عوامی لانچ (N1) اور توسیع (N2) مراحل کو
کور کرتی ہے، ہر ورک فلو کو درکار شواہد، dashboards اور escalation راستوں سے
جوڑتی ہے۔

## 2. کردار اور رابطہ نقشہ| کردار | بنیادی ذمہ داریاں | بنیادی artefacts اور telemetry | הסלמה |
|------|----------------|--------------------------------|------|
| מועצת ממשל | چارٹرز، suffix پالیسیوں، تنازعہ فیصلوں اور steward rotations کی تدوین و توثیق۔ | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, council ballots جو `sns governance charter submit` سے محفوظ ہوتے ہیں۔ | יו"ר מועצה + מעקב אחר מסמכי ממשל. |
| לוח האפוטרופוסים | soft/hard freezes، ہنگامی canons، اور 72 h reviews جاری کرتا ہے۔ | Guardian tickets جو `sns governance freeze` سے نکلتے ہیں، override manifests جو `artifacts/sns/guardian/*` میں لاگ ہوتے ہیں۔ | סיבוב אפוטרופוס (<=15 דקות ACK) . |
| סיומת דיילים | registrar queues، auctions، pricing tiers اور customer comms چلاتے ہیں؛ compliance acknowledgements دیتے ہیں۔ | Steward policies `SuffixPolicyV1` میں، pricing reference sheets، اور steward acknowledgements جو regulatory memos کے ساتھ محفوظ ہوتے ہیں۔ | Steward program lead + suffix مخصوص PagerDuty۔ |
| רשם ואופציות חיוב | `/v2/sns/*` endpoints چلاتے ہیں، payments reconcile کرتے ہیں، telemetry emit کرتے ہیں، اور CLI snapshots برقرار رکھتے ہیں۔ | Registrar API ([`registrar-api.md`](./registrar-api.md)), `sns_registrar_status_total` metrics، payment proofs جو `artifacts/sns/payments/*` میں محفوظ ہیں۔ | מנהל תורן רשם או קשר אוצר. |
| Resolver ו-gateway מפעילי | SoraDNS، GAR اور gateway state کو registrar events کے ساتھ aligned رکھتے ہیں؛ transparency metrics stream کرتے ہیں۔ | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Resolver SRE כוננות + גשר מבצעי שער. |
| אוצר ופיננסים | 70/30 revenue split، referral carve-outs، tax/treasury filings اور SLA attestations نافذ کرتے ہیں۔ | Revenue accrual manifests، Stripe/treasury exports، اور سہ ماہی KPI appendices `docs/source/sns/regulatory/` میں۔ | בקר כספים + קצין ציות. |
| ציות וקישור רגולטורי | عالمی ذمہ داریوں (EU DSA وغیرہ) کو ٹریک کرتا ہے، KPI covenants اپڈیٹ کرتا ہے، اور disclosures فائل کرتا ہے۔ | Regulatory memos `docs/source/sns/regulatory/` میں، reference decks، اور tabletop rehearsals کے لئے `ops/drill-log.md` entries۔ | מוביל תוכנית ציות. |
| תמיכה / SRE כוננות | תקריות (התנגשויות, סחיפה של חשבונות, הפסקות פתרון) | Incident templates، `ops/drill-log.md`, staged lab evidence، Slack/war-room transcripts جو `incident/` میں محفوظ ہیں۔ | סיבוב SNS כוננות + ניהול SRE. |

## 3. کینونیکل artefacts اور ڈیٹا ذرائع| חפץ | מאת | مقصد |
|--------|------|------|
| אמנה + תוספות KPI | `docs/source/sns/governance_addenda/` | קולות של זכויות יוצרים, התחייבויות של KPI, אומנות של קולות וקולות CLI בכמה מונים ہیں۔ |
| סכימת רישום | [`registry-schema.md`](./registry-schema.md) | کینونیکل Norito ساختیں (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`)۔ |
| חוזה רשם | [`registrar-api.md`](./registrar-api.md) | REST/gRPC payloads، `sns_registrar_status_total` metrics، اور governance hook توقعات۔ |
| כתובת UX מדריך | [`address-display-guidelines.md`](./address-display-guidelines.md) | کینونیکل I105 (ترجیحی) / I105 (`sora`) renderings جو wallets/explorers میں جھلکتے ہیں۔ |
| SoraDNS / GAR מסמכים | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | גזירת מארח דטרמיניסטית, זרימת עבודה עם שקיפות, או כללי התראה. |
| תזכירי רגולציה | `docs/source/sns/regulatory/` | Jurisdictional intake notes (مثلا EU DSA)، steward acknowledgements، template annexes۔ |
| יומן מקדחה | `ops/drill-log.md` | Chaos اور IR rehearsals کا ریکارڈ جو phase exits سے پہلے لازم ہیں۔ |
| אחסון חפצים | `artifacts/sns/` | Payment proofs، guardian tickets، resolver diffs، KPI exports، اور `sns governance ...` سے تیار شدہ signed CLI output۔ |

تمام گورننس اقدامات کو اوپر کی جدول میں سے کم از کم ایک artefact کا حوالہ دینا
چاہیے تاکہ auditors 24 گھنٹوں میں decision trail دوبارہ بنا سکیں۔

## 4. لائف سائیکل پلی بکس

### 4.1 אמנה או הצעות דיילים

| مرحلہ | مالک | CLI / עדות | نوٹس |
|-------|------|----------------|------|
| טיוטת נספח או דלתות KPI | כתב המועצה + ראש דייל | Markdown template `docs/source/sns/governance_addenda/YY/` میں محفوظ | KPI covenant IDs، telemetry hooks اور activation conditions شامل کریں۔ |
| הגשת ההצעה | ראש המועצה | `sns governance charter submit --input SN-CH-YYYY-NN.md` (`CharterMotionV1` بناتا ہے) | CLI Norito manifest `artifacts/sns/governance/<id>/charter_motion.json` میں محفوظ کرتی ہے۔ |
| הצביעו אישור אפוטרופוס | מועצה + אפוטרופוסים | `sns governance ballot cast --proposal <id>` אור `sns governance guardian-ack --proposal <id>` | Hashed minutes اور quorum proofs منسلک کریں۔ |
| קבלת דייל | תוכנית דיילים | `sns governance steward-ack --proposal <id> --signature <file>` | Suffix policies تبدیل کرنے سے پہلے لازم؛ `artifacts/sns/governance/<id>/steward_ack.json` میں envelope ریکارڈ کریں۔ |
| הפעלה | רשם אופס | `SuffixPolicyV1` اپڈیٹ کریں، registrar caches ریفریش کریں، `status.md` میں نوٹ شائع کریں۔ | Activation timestamp `sns_governance_activation_total` میں لاگ ہوتا ہے۔ |
| יומן ביקורת | ציות | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` اور drill log میں اندراج کریں اگر tabletop ہوا ہو۔ | Telemetry dashboards اور policy diffs کی حوالہ جات شامل کریں۔ |

### 4.2 רישום, מכירה פומבית או אישורי תמחור1. **Preflight:** Registrar `SuffixPolicyV1` کو دیکھ کر pricing tier، دستیاب terms اور
   grace/redemption windows کی تصدیق کرتا ہے۔ قیمت کی شیٹس کو roadmap میں درج
   3/4/5/6-9/10+ tier ٹیبل (base tier + suffix coefficients) کے مطابق synced رکھیں۔
2. **Sealed-bid auctions:** Premium pools کے لئے 72 h commit / 24 h reveal cycle
   `sns governance auction commit` / `... reveal` کے ذریعے چلائیں۔ commit فہرست
   (صرف hashes) `artifacts/sns/auctions/<name>/commit.json` میں شائع کریں تاکہ
   auditors randomness verify کر سکیں۔
3. **Payment verification:** Registrars `PaymentProofV1` کو treasury splits کے خلاف
   validate کرتے ہیں (70% treasury / 30% steward، referral carve-out <=10%). Norito JSON
   `artifacts/sns/payments/<tx>.json` תגובת רשם
   (`RevenueAccrualEventV1`)
4. **Governance hook:** Premium/guarded ناموں کے لئے `GovernanceHookV1` لگائیں جس میں
   council proposal ids اور steward signatures ہوں۔ ווים חסרים
   `sns_err_governance_missing` آتا ہے۔
5. **Activation + resolver sync:** جیسے ہی Torii registry event بھیجے، resolver transparency
   tailer چلائیں تاکہ نیا GAR/zone state پھیلنے کی تصدیق ہو (4.5 دیکھیں)۔
6. **Customer disclosure:** Customer-facing ledger (wallet/explorer) کو
   [`address-display-guidelines.md`](./address-display-guidelines.md) کی shared fixtures کے
   עיבודי I105 דחוסים (`sora`) העתק/QR
   guidance سے میل کھاتے ہیں۔

### 4.3 חידושים, חיוב או התאמה בין האוצר

- **Renewal workflow:** Registrars `SuffixPolicyV1` میں دی گئی 30 دن grace + 60 دن
  redemption windows نافذ کرتے ہیں۔ רצף פתיחה מחדש של 60 שנה בהולנדית (7 שנים, פי 10 בתשלום או
  15%/دن کم ہوتی ہے) خودکار طور پر `sns governance reopen` سے چلتی ہے۔
- **Revenue split:** ہر renewal یا transfer `RevenueAccrualEventV1` بناتا ہے۔ יצוא האוצר
  (CSV/Parquet) کو روزانہ ان events کے ساتھ reconcile کرنا چاہیے؛ הוכחות
  `artifacts/sns/treasury/<date>.json` מאגר מידע
- **Referral carve-outs:** اختیاری referral فیصد suffix کے حساب سے `referral_share` کو
  steward policy میں شامل کر کے ٹریک کیے جاتے ہیں۔ Registrars final split emit کرتے ہیں
  اور referral manifests کو payment proof کے ساتھ رکھتے ہیں۔
- **קצב דיווח:** נספחי KPI פיננסיים (רישומים, חידושים, ARPU, מחלוקת/ערובה
  utilization) `docs/source/sns/regulatory/<suffix>/YYYY-MM.md` میں پوسٹ کرتا ہے۔ לוחות מחוונים
  انہی export tables سے چلیں تاکہ Grafana نمبرز ledger evidence سے میچ کریں۔
- **Monthly KPI review:** پہلے منگل کا checkpoint finance lead، duty steward اور program PM کو جوڑتا ہے۔
  [SNS KPI dashboard](./kpi-dashboard.md) کھولیں (portal embed `sns-kpis` /
  `dashboards/grafana/sns_suffix_analytics.json`), תפוקת רשם + טבלאות הכנסות
  export کریں، deltas annex میں لاگ کریں، اور artefacts memo کے ساتھ لگائیں۔ סקירה
  SLA breaches (freeze windows >72 h، registrar error spikes، ARPU drift) پائے تو incident
  trigger کریں۔

### 4.4 הקפאה, חולק על ערעורים| مرحلہ | مالک | פעולה אוור ראיות | SLA |
|-------|------|------------------------|-----|
| בקשה להקפאה רכה | דייל / תמיכה | Ticket `SNS-DF-<id>` فائل کریں جس میں payment proofs، dispute bond reference، اور affected selector(s) ہوں۔ | <=4 h intake سے۔ |
| כרטיס שומר | לוח שומר | `sns governance freeze --selector <I105> --reason <text> --until <ts>` `GuardianFreezeTicketV1` بناتا ہے۔ Ticket JSON `artifacts/sns/guardian/<id>.json` میں رکھیں۔ | <=30 דקות ACK, <=2 שעות ביצוע. |
| אשרור המועצה | מועצת ממשל | Freeze approve/reject کریں، guardian ticket اور dispute bond digest کے ساتھ فیصلہ ڈاکیومنٹ کریں۔ | اگلا council session یا async vote۔ |
| פאנל בוררות | ציות + דייל | 7 juror panel (roadmap کے مطابق) بلائیں، hashed ballots `sns governance dispute ballot` کے ذریعے جمع کریں۔ Anonymous vote receipts incident packet میں لگائیں۔ | Verdict <=7 days بعد bond deposit۔ |
| ערעור | אפוטרופוס + מועצה | Appeals bond کو دوگنا کرتی ہیں اور juror process دہراتی ہیں؛ Norito manifest `DisputeAppealV1` ریکارڈ کریں اور primary ticket ریفرنس کریں۔ | <=10 ימים. |
| ביטול הקפאה ותיקון | רשם + פותר פעולות | `sns governance unfreeze --selector <I105> --ticket <id>` چلائیں، registrar status اپڈیٹ کریں، اور GAR/resolver diffs propagate کریں۔ | Verdict کے فوراً بعد۔ |

Emergency canons (guardian-triggered freezes <=72 h) بھی اسی flow کو فالو کرتے ہیں
لیکن retroactive council review اور `docs/source/sns/regulatory/` میں transparency
note درکار ہے۔

### 4.5 Resolver או הפצת שער

1. **Event hook:** ہر registry event resolver event stream (`tools/soradns-resolver` SSE) پر
   emit ہوتا ہے۔ Resolver ops subscribe کر کے transparency tailer
   (`scripts/telemetry/run_soradns_transparency_tail.sh`) کے ذریعے diffs ریکارڈ کرتے ہیں۔
2. **GAR template update:** Gateways کو `canonical_gateway_suffix()` سے refer ہونے والے GAR templates
   اپڈیٹ کرنے ہوں گے اور `host_pattern` list کو دوبارہ sign کرنا ہوگا۔ הבדלים
   `artifacts/sns/gar/<date>.patch` میں رکھیں۔
3. **Zonefile publication:** `roadmap.md` میں بیان کردہ zonefile skeleton (name, ttl, cid, proof)
   استعمال کریں اور اسے Torii/SoraFS پر push کریں۔ Norito JSON
   `artifacts/sns/zonefiles/<name>/<version>.json` میں archive کریں۔
4. **בדיקת שקיפות:** `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   چلائیں تاکہ alerts green رہیں۔ Prometheus text output کو weekly transparency report کے ساتھ منسلک کریں۔
5. **ביקורת שער:** `Sora-*` דוגמאות כותרות (מדיניות מטמון, CSP, תקציר GAR)
   governance log کے ساتھ attach کریں تاکہ آپریٹرز ثابت کر سکیں کہ gateway نے نیا نام
   مطلوبہ guardrails کے ساتھ serve کیا۔

## 5. דיווח על טלמטריה| אות | מקור | תיאור / פעולה |
|--------|--------|---------------------|
| `sns_registrar_status_total{result,suffix}` | מטפלי רשם Torii | رجسٹریشن، renewals، freezes، transfers کے لئے success/error counter؛ جب `result="error"` suffix کے حساب سے بڑھ جائے تو alert۔ |
| `torii_request_duration_seconds{route="/v2/sns/*"}` | מדדי Torii | API handlers کے لئے latency SLOs؛ `torii_norito_rpc_observability.json` سے بنے dashboards کو feed کرتا ہے۔ |
| `soradns_bundle_proof_age_seconds` אור `soradns_bundle_cid_drift_total` | התאמה לשקיפות פותר | پرانے proofs یا GAR drift کو detect کرتا ہے؛ guardrails `dashboards/alerts/soradns_transparency_rules.yml` میں define ہیں۔ |
| `sns_governance_activation_total` | ממשל CLI | ہر charter/addendum activation پر بڑھنے والا counter؛ council decisions کو published addenda کے ساتھ reconcile کرنے میں استعمال۔ |
| מד `guardian_freeze_active` | גרדיאן CLI | ہر selector کے لئے soft/hard freeze windows track کرتا ہے؛ اگر `1` SLA سے زیادہ رہے تو SRE کو page کریں۔ |
| לוחות מחוונים נספח KPI | כספים / מסמכים | ماہانہ rollups جو regulatory memos کے ساتھ شائع ہوتے ہیں؛ portal انہیں [SNS KPI dashboard](./kpi-dashboard.md) کے ذریعے embed کرتا ہے تاکہ stewards اور regulators ایک ہی Grafana view تک پہنچیں۔ |

## 6. ראיות או דרישות ביקורת

| פעולה | עדות לארכיון | אחסון |
|--------|------------------------|--------|
| אמנה / שינוי מדיניות | מניפסט Norito חתום, תמליל CLI, הבדל KPI, אישור דייל. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| הרשמה / חידוש | מטען `RegisterNameRequestV1`, `RevenueAccrualEventV1`, הוכחת תשלום. | `artifacts/sns/payments/<tx>.json`, יומני API של רשם. |
| מכירה פומבית | התחייבות/חשוף מניפסטים, סיד אקראיות, גיליון אלקטרוני של חישוב מנצח. | `artifacts/sns/auctions/<name>/`. |
| להקפיא / לבטל את ההקפאה | כרטיס אפוטרופוס, חשיש להצבעה במועצה, כתובת אתר של יומן אירועים, תבנית הודעות לקוח. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| הפצת פותר | Zonefile/GAR diff, קטע JSONL tailer, Prometheus תמונת מצב. | `artifacts/sns/resolver/<date>/` + דוחות שקיפות. |
| צריכה רגולטורית | תזכיר הכנסה, מעקב אחר מועדים, אישור דייל, סיכום שינוי KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. רשימת תיוג שער שלב

| שלב | קריטריוני יציאה | צרור ראיות |
|-------|---------------|----------------|
| N0 - בטא סגורה | SN-1/SN-2 registry schema، manual registrar CLI، guardian drill مکمل۔ | תנועת אמנה + דייל ACK, יומני ריצה יבשה של רשם, דוח שקיפות פותר, ערך `ops/drill-log.md`. |
| N1 - השקה פומבית | מכירות פומביות + רמות מחיר קבועות בזמן אמת עבור `.sora`/`.nexus`, רשם בשירות עצמי, סנכרון אוטומטי של פותר, לוחות מחוונים לחיוב. | הבדל בגיליון התמחור, תוצאות CI של הרשם, נספח תשלום/KPI, פלט התאמה לשקיפות, הערות חזרות על תקריות. |
| N2 - הרחבה | `.dao`, ממשקי API של מפיצים, פורטל מחלוקות, כרטיסי ניקוד של דיילים, לוחות מחוונים לניתוח. | צילומי מסך של פורטל, מדדי SLA למחלוקות, ייצוא כרטיסי ניקוד של מנהלים, אמנת ממשל מעודכנת המתייחסת למדיניות המפיצים. |

Phase exits کو ریکارڈ شدہ tabletop drills (registration happy path، freeze، resolver outage) درکار ہیں جن کے
artefacts `ops/drill-log.md` میں منسلک ہوں۔## 8. תגובה לאירוע או הסלמה

| טריגר | חומרה | בעלים מיידי | פעולות חובה |
|--------|--------|----------------|------------------------|
| Resolver/GAR drift یا stale proofs | סב 1 | Resolver SRE + לוח אפוטרופוס | resolver on-call کو page کریں، tailer output capture کریں، متاثرہ نام freeze کرنے کا فیصلہ کریں، ہر 30 min پر status update دیں۔ |
| הפסקת רשם, כשל בחיוב, 1 שגיאות API נפוצות | סב 1 | מנהל תורן רשם | نئی auctions روکیں، manual CLI پر سوئچ کریں، stewards/treasury کو اطلاع دیں، Torii logs کو incident doc میں attach کریں۔ |
| מחלוקת על שם יחיד, אי התאמה בתשלום, 1 הסלמה של לקוחות | סב' 2 | דייל + מוביל תמיכה | payment proofs جمع کریں، soft freeze کی ضرورت طے کریں، SLA کے اندر requester کو جواب دیں، dispute tracker میں نتیجہ لاگ کریں۔ |
| ממצא ביקורת ציות | סב' 2 | קשר ציות | remediation plan تیار کریں، `docs/source/sns/regulatory/` میں memo فائل کریں، follow-up council session شیڈول کریں۔ |
| Drill یا rehearsal | סוו 3 | ראש הממשלה של התוכנית | `ops/drill-log.md` کے scripted scenario کو چلائیں، artefacts archive کریں، gaps کو roadmap tasks کے طور پر لیبل کریں۔ |

تمام incidents کو `incident/YYYY-MM-DD-sns-<slug>.md` بنانا ہوگا جس میں ownership tables،
command logs، اور اس پلی بک میں تیار ہونے والے evidence کے references ہوں۔

## 9. حوالہ جات

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (קטעי SNS, DG, ADDR)

جب بھی charter wording، CLI surfaces یا telemetry contracts بدلیں تو اس پلی بک کو
اپڈیٹ رکھیں؛ roadmap entries جو `docs/source/sns/governance_playbook.md` کو
ریفرنس کرتی ہیں انہیں ہمیشہ تازہ ترین ورژن سے میل کھانا چاہیے۔