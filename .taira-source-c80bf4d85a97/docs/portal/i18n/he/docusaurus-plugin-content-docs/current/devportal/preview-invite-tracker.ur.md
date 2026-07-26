---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-invite-tracker.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو دعوت ٹریکر

یہ ٹریکر docs پورٹل کی ہر پریویو ویو ریکارڈ کرتا ہے تاکہ DOCS-SORA کے مالکان اور governance reviewers دیکھ سکیں کہ کون سی cohort فعال ہے، کس نے دعوتیں منظور کیں، اور کون سے artifacts ابھی توجہ چاہتے ہیں۔ ‏ ‏ تاکہ audit trail ریپوزٹری کے اندر رہے۔

## ویو اسٹیٹس

| ויו | קבוצה | ٹریکر ایشو | מאשר/ים | اسٹیٹس | ہدف ونڈو | نوٹس |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - מתחזקי ליבה** | Docs + SDK maintainers جو checksum فلو validate کرتے ہیں | `DOCS-SORA-Preview-W0` (GitHub/ops tracker) | Docs/DevRel lead + Portal TL | מקל | Q2 2025 ہفتے 1-2 | תאריך 2025-03-25 תקציר יציאה 2025-04-08 תקציר יציאה 2025-04-08 |
| **W1 - שותפים** | SoraFS آپریٹرز، Torii integrators تحت NDA | `DOCS-SORA-Preview-W1` | מוביל Docs/DevRel + קשר ממשל | מקל | Q2 2025 ہفتہ 3 | دعوتیں 2025-04-12 -> 2025-04-26، تمام آٹھ پارٹنرز کی تصدیق؛ evidence [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) میں اور exit digest [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) میں۔ |
| **W2 - קהילה** | منتخب community waitlist (<=25 ایک وقت میں) | `DOCS-SORA-Preview-W2` | Docs/DevRel lead + מנהל קהילה | מקל | Q3 2025 ہفتہ 1 (عارضی) | دعوتیں 2025-06-15 -> 2025-06-29، پوری مدت میں ٹیلی میٹری سبز؛ evidence + findings [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md) میں۔ |
| **W3 - קבוצות ביטא** | בטא פיננסים/צפיות + שותף SDK + עו"ד | `DOCS-SORA-Preview-W3` | מוביל Docs/DevRel + קשר ממשל | מקל | Q1 2026 ہفتہ 8 | תאריך 2026-02-18 -> 2026-02-28; digest + portal data `preview-20260218` ویو سے تیار (دیکھیں [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> نوٹ: ہر ٹریکر ایشو کو متعلقہ preview request tickets سے لنک کریں اور انہیں `docs-portal-preview` پروجیکٹ میں archive کریں تاکہ approvals قابلِ دریافت رہیں۔

## فعال کام (W0)

- preflight artifacts ریفریش (GitHub Actions `docs-portal-preview` رن 2025-03-24، descriptor `scripts/preview_verify.sh` کے ذریعے `preview-2025-03-24` ٹیگ سے verify)۔
- ٹیلی میٹری baselines محفوظ (`docs.preview.integrity`, `TryItProxyErrors` dashboard snapshot W0 issue میں محفوظ)۔
- outreach متن [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) کے ساتھ lock، preview tag `preview-2025-03-24`۔
- پہلے پانچ maintainers کے لئے intake requests لاگ (tickets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- 2025-03-25 10:00-10:20 UTC בתאריך 2025-03-20 UTC کے بعد؛ acknowledgements `DOCS-SORA-Preview-W0` میں محفوظ۔
- ٹیلی میٹری مانیٹرنگ + host office hours (2025-03-31 تک روزانہ check-ins; checkpoint log نیچے)۔
- midpoint feedback / issues اکٹھے کر کے `docs-preview/w0` ٹیگ (دیکھیں [W0 digest](./preview-feedback/w0/summary.md))۔
- ویو summary شائع + invite exit confirmations (exit bundle تاریخ 2025-04-08; دیکھیں [W0 digest](./preview-feedback/w0/summary.md))۔
- W3 beta wave ٹریک; مستقبل کی ویوز governance review کے بعد شیڈول۔

## שותפים של W1 או טלפונים- قانونی اور governance approvals۔ Partner addendum 2025-04-05 کو سائن؛ approvals `DOCS-SORA-Preview-W1` میں اپ لوڈ۔
- טלמטריה + נסה זאת בימוי. Change ticket `OPS-TRYIT-147` 2025-04-06 کو اجرا؛ `docs.preview.integrity`, `TryItProxyErrors`, اور `DocsPortal/GatewayRefusals` کے Grafana snapshots آرکائیو۔
- הכנת חפץ + סכום בדיקה. אימות חבילה `preview-2025-04-12`; descriptor/checksum/probe logs `artifacts/docs_preview/W1/preview-2025-04-12/` میں محفوظ۔
- הזמנה של סגל + שליחה. آٹھ partner requests (`DOCS-SORA-Preview-REQ-P01...P08`) منظور؛ دعوتیں 2025-04-12 15:00-15:21 UTC کو بھیجیں، ہر reviewer کا ack ریکارڈ۔
- מכשור משוב. روزانہ office hours + telemetry checkpoints ریکارڈ؛ digest کے لئے [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) دیکھیں۔
- סגל/יומן יציאה סופי. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) או חותמות זמן להזמנה/אקח, עדויות טלמטריה, ייצוא חידון, או מצביעי חפצים 2025-04-26 reproduce کر سکے۔

## יומן הזמנות - מנהלי ליבה של W0

| מזהה מבקר | תפקיד | בקש כרטיס | ההזמנה נשלחה (UTC) | יציאה צפויה (UTC) | סטטוס | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | מתחזק פורטל | `DOCS-SORA-Preview-REQ-01` | 25-03-2025 10:05 | 2025-04-08 10:00 | פעיל | checksum verification کی تصدیق؛ nav/sidebar review پر توجہ۔ |
| sdk-rust-01 | עופרת SDK חלודה | `DOCS-SORA-Preview-REQ-02` | 25-03-2025 10:08 | 2025-04-08 10:00 | פעיל | SDK recipes + Norito quickstarts ٹیسٹ کر رہا ہے۔ |
| sdk-js-01 | מתחזק JS SDK | `DOCS-SORA-Preview-REQ-03` | 25-03-2025 10:12 | 2025-04-08 10:00 | פעיל | Try it console + ISO flows ویلیڈیٹ۔ |
| sorafs-ops-01 | SoraFS קשר מפעיל | `DOCS-SORA-Preview-REQ-04` | 25-03-2025 10:15 | 2025-04-08 10:00 | פעיל | SoraFS runbooks + orchestration docs آڈٹ۔ |
| observability-01 | צפיות TL | `DOCS-SORA-Preview-REQ-05` | 25-03-2025 10:18 | 2025-04-08 10:00 | פעיל | telemetry/incident appendices کا جائزہ؛ Alertmanager coverage کے ذمہ دار۔ |

تمام دعوتیں ایک ہی `docs-portal-preview` artefact (run 2025-03-24، tag `preview-2025-03-24`) اور `DOCS-SORA-Preview-W0` میں محفوظ verification transcript کو ریفرنس کرتی ہیں۔ کسی بھی اضافے/وقفے کو اگلی ویو پر جانے سے پہلے اوپر والی ٹیبل اور tracker issue دونوں میں لاگ کریں۔

## יומן מחסום - W0

| תאריך (UTC) | פעילות | הערות |
| --- | --- | --- |
| 26-03-2025 | סקירת בסיס טלמטריה + שעות עבודה | `docs.preview.integrity` + `TryItProxyErrors` سبز رہے؛ office hours نے checksum verification مکمل ہونے کی تصدیق کی۔ |
| 27-03-2025 | תקציר משוב נקודת אמצע פורסם | Summary [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md) میں محفوظ؛ دو چھوٹے nav issues `docs-preview/w0` کے تحت، کوئی incident نہیں۔ |
| 31-03-2025 | בדיקת נקודתית של טלמטריה בשבוע האחרון | آخری office hours؛ reviewers نے باقی کام تصدیق کیا، کوئی alert نہیں۔ |
| 2025-04-08 | סיכום יציאה + סגירות הזמנות | reviews مکمل، عارضی رسائی منسوخ، findings [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08) میں آرکائیو؛ W1 سے پہلے tracker اپ ڈیٹ۔ |

## יומן הזמנות - שותפים W1| מזהה מבקר | תפקיד | בקש כרטיס | ההזמנה נשלחה (UTC) | יציאה צפויה (UTC) | סטטוס | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| soraps-op-01 | מפעיל SoraFS (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 26/04/2025 15:00 | הושלם | orchestrator ops feedback 2025-04-20 کو دیا؛ יציאה ack 15:05 UTC. |
| soraps-op-02 | מפעיל SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 12-04-2025 15:03 | 26/04/2025 15:00 | הושלם | rollout comments `docs-preview/w1` میں لاگ؛ יציאה ack 15:10 UTC. |
| soraps-op-03 | מפעיל SoraFS (ארה"ב) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 26/04/2025 15:00 | הושלם | dispute/blacklist edits فائل؛ יציאה ack 15:12 UTC. |
| torii-int-01 | אינטגרטור Torii | `DOCS-SORA-Preview-REQ-P04` | 12-04-2025 15:09 | 26/04/2025 15:00 | הושלם | Try it auth walkthrough قبول؛ יציאה ack 15:14 UTC. |
| torii-int-02 | אינטגרטור Torii | `DOCS-SORA-Preview-REQ-P05` | 12-04-2025 15:12 | 26/04/2025 15:00 | הושלם | הערות RPC/OAuth יציאה ack 15:16 UTC. |
| sdk-partner-01 | שותף SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 12-04-2025 15:15 | 26/04/2025 15:00 | הושלם | מיזוג משוב שלמות בתצוגה מקדימה; יציאה ack 15:18 UTC. |
| sdk-partner-02 | שותף SDK (אנדרואיד) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 26/04/2025 15:00 | הושלם | סקירת טלמטריה/עריכה יציאה ack 15:22 UTC. |
| gateway-ops-01 | מפעיל שער | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 26/04/2025 15:00 | הושלם | gateway DNS runbook comments فائل؛ יציאה ack 15:24 UTC. |

## יומן מחסום - W1

| תאריך (UTC) | פעילות | הערות |
| --- | --- | --- |
| 2025-04-12 | שליחת הזמנה + אימות חפץ | `preview-2025-04-12` descriptor/archive کے ساتھ آٹھ partners کو ای میل؛ acknowledgements tracker میں محفوظ۔ |
| 2025-04-13 | סקירת בסיס טלמטריה | `docs.preview.integrity`, `TryItProxyErrors`, اور `DocsPortal/GatewayRefusals` سبز؛ office hours نے checksum verification مکمل ہونے کی تصدیق کی۔ |
| 2025-04-18 | שעות עבודה באמצע גל | `docs.preview.integrity` سبز رہا؛ دو doc نٹس `docs-preview/w1` کے تحت لاگ (nav wording + Try it screenshot)۔ |
| 22-04-2025 | בדיקת נקודתית סופית בטלמטריה | proxy + dashboards صحت مند؛ کوئی نئی issues نہیں، exit سے پہلے tracker میں نوٹ۔ |
| 26-04-2025 | סיכום יציאה + סגירות הזמנות | تمام partners نے review completion کی تصدیق کی، invites revoke، evidence [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26) میں آرکائیو۔ |

## סיכום קבוצת הביטא של W3

- 2026-02-18 کو invites بھیجی گئیں، checksum verification + acknowledgements اسی دن لاگ۔
- משוב `docs-preview/20260218` בנושא ממשל `DOCS-SORA-Preview-20260218`; digest + summary `npm run --prefix docs/portal preview:wave -- --wave preview-20260218` سے تیار۔
- 2026-02-28 کو final telemetry check کے بعد access revoke؛ tracker + portal tables اپ ڈیٹ کر کے W3 مکمل دکھایا۔

## יומן הזמנות - קהילת W2| מזהה מבקר | תפקיד | בקש כרטיס | ההזמנה נשלחה (UTC) | יציאה צפויה (UTC) | סטטוס | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | מבקר קהילה (SDK) | `DOCS-SORA-Preview-REQ-C01` | 15-06-2025 16:00 | 29/06/2025 16:00 | הושלם | אק 16:06 UTC; SDK quickstarts پر فوکس؛ exit 2025-06-29 کو کنفرم۔ |
| comm-vol-02 | מבקר קהילה (ממשל) | `REQ-C02` | 15/06/2025 16:03 | 29/06/2025 16:00 | הושלם | סקירת ממשל/SNS exit 2025-06-29 کو کنفرم۔ |
| comm-vol-03 | מבקר קהילה (Norito) | `REQ-C03` | 15/06/2025 16:06 | 29/06/2025 16:00 | הושלם | Norito walkthrough feedback لاگ؛ יציאה ack 2025-06-29. |
| comm-vol-04 | מבקר קהילה (SoraFS) | `REQ-C04` | 15/06/2025 16:09 | 29/06/2025 16:00 | הושלם | SoraFS runbook review مکمل؛ יציאה ack 2025-06-29. |
| comm-vol-05 | מבקר קהילה (נגישות) | `REQ-C05` | 15/06/2025 16:12 | 29/06/2025 16:00 | הושלם | הערות נגישות/UX יציאה ack 2025-06-29. |
| comm-vol-06 | מבקר קהילה (לוקליזציה) | `REQ-C06` | 15/06/2025 16:15 | 29/06/2025 16:00 | הושלם | משוב על לוקליזציה יציאה ack 2025-06-29. |
| comm-vol-07 | מבקר קהילה (נייד) | `REQ-C07` | 15/06/2025 16:18 | 29/06/2025 16:00 | הושלם | Mobile SDK doc checks مکمل؛ יציאה ack 2025-06-29. |
| comm-vol-08 | מבקר קהילה (צפיות) | `REQ-C08` | 15/06/2025 16:21 | 29/06/2025 16:00 | הושלם | Observability appendix review مکمل؛ יציאה ack 2025-06-29. |

## יומן מחסום - W2

| תאריך (UTC) | פעילות | הערות |
| --- | --- | --- |
| 2025-06-15 | שליחת הזמנה + אימות חפץ | `preview-2025-06-15` descriptor/archive 8 community reviewers کے ساتھ شیئر؛ acknowledgements tracker میں محفوظ۔ |
| 2025-06-16 | סקירת בסיס טלמטריה | לוחות מחוונים `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` Try it proxy logs میں community tokens فعال۔ |
| 2025-06-18 | שעות משרד ובחינת גיליון | או הצעות (ניסוח תיאור כלי `docs-preview/w2 #1`, סרגל צד לוקליזציה של `#2`) - מסמכים או מנותבים. |
| 21-06-2025 | בדיקת טלמטריה + תיקוני מסמכים | Docs نے `docs-preview/w2 #1/#2` حل کیا؛ dashboards سبز، کوئی incident نہیں۔ |
| 24-06-2025 | שעות עבודה בשבוע האחרון | reviewers نے باقی feedback submissions کنفرم کیے؛ کوئی alert نہیں۔ |
| 29-06-2025 | סיכום יציאה + סגירות הזמנות | acks ریکارڈ، preview access revoke، telemetry snapshots + artefacts آرکائیو (دیکھیں [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | שעות משרד ובחינת גיליון | دو documentation suggestions `docs-preview/w1` کے تحت لاگ؛ کوئی incidents یا alerts نہیں۔ |

## ווי דיווח- ہر بدھ، اوپر والی table اور active invite issue کو مختصر status note سے اپ ڈیٹ کریں (invites sent, active reviewers, incidents).
- جب کوئی ویو بند ہو، feedback summary path شامل کریں (مثال: `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) اور اسے `status.md` سے لنک کریں۔
- اگر [preview invite flow](./preview-invite-flow.md) کے pause criteria ٹرگر ہوں تو invites دوبارہ شروع کرنے سے پہلے remediation steps یہاں شامل کریں۔