---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/devportal/preview-invite-tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c984866497c2c0379f868629cb58ece2de199452bc734e043bc9ea873b4d482e
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-invite-tracker.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# מעקב הזמנות preview

מעקב זה רושם כל גל preview של פורטל התיעוד כדי שבעלי DOCS-SORA וסוקרי ממשל יראו איזו קבוצה פעילה, מי אישר את ההזמנות, ואילו ארטיפקטים עדיין דורשים טיפול. עדכנו אותו בכל פעם שהזמנות נשלחות, נשללות או נדחות כדי שהמסלול הביקורתי ישאר במאגר.

## סטטוס גלים

| גל | קוהורט | Issue מעקב | מאשרים | סטטוס | חלון יעד | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Core maintainers** | Maintainers של Docs + SDK שמאמתים את זרימת ה-checksum | `DOCS-SORA-Preview-W0` (tracker GitHub/ops) | Lead Docs/DevRel + Portal TL | הושלם | Q2 2025 שבועות 1-2 | ההזמנות נשלחו 2025-03-25, הטלמטריה נשארה ירוקה, סיכום יציאה פורסם 2025-04-08. |
| **W1 - Partners** | מפעילי SoraFS, אינטגרטורים Torii תחת NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + נציג ממשל | הושלם | Q2 2025 שבוע 3 | ההזמנות 2025-04-12 -> 2025-04-26 עם אישור כל שמונת השותפים; הראיות ב-[`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) וסיכום היציאה ב-[`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Community** | רשימת המתנה קהילתית אוצרת (<=25 בכל פעם) | `DOCS-SORA-Preview-W2` | Lead Docs/DevRel + community manager | הושלם | Q3 2025 שבוע 1 (משוער) | ההזמנות 2025-06-15 -> 2025-06-29 עם טלמטריה ירוקה לאורך התקופה; ראיות + ממצאים ב-[`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Cohortes beta** | בטא פיננס/אובזרווביליטי + שותף SDK + advocate אקוסיסטם | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + נציג ממשל | הושלם | Q1 2026 שבוע 8 | ההזמנות 2026-02-18 -> 2026-02-28; סיכום + נתוני פורטל נוצרו בגל `preview-20260218` (ראו [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> הערה: קשרו כל issue במעקב לכרטיסי הבקשה המתאימים וארכיבו אותם תחת הפרויקט `docs-portal-preview` כדי שהאישורים יישארו ניתנים לאיתור.

## משימות פעילות (W0)

- רענון ארטיפקטי preflight (הרצת GitHub Actions `docs-portal-preview` ב-2025-03-24, בדיקת descriptor דרך `scripts/preview_verify.sh` עם התג `preview-2025-03-24`).
- בסיסי טלמטריה תועדו (`docs.preview.integrity`, snapshot של `TryItProxyErrors` נשמר ב-issue W0).
- טקסט outreach ננעל באמצעות [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) עם תג preview `preview-2025-03-24`.
- בקשות כניסה נרשמו עבור חמשת ה-maintainers הראשונים (כרטיסים `DOCS-SORA-Preview-REQ-01` ... `-05`).
- חמש ההזמנות הראשונות נשלחו 2025-03-25 10:00-10:20 UTC אחרי שבעה ימים רצופים של טלמטריה ירוקה; אישורים נשמרו ב-`DOCS-SORA-Preview-W0`.
- ניטור טלמטריה + office hours של המארח (בדיקות יומיות עד 2025-03-31; יומן checkpoints למטה).
- משוב אמצע גל / issues נאספו ותויגו `docs-preview/w0` (ראו [W0 digest](./preview-feedback/w0/summary.md)).
- סיכום גל פורסם + אישורי יציאה (bundle יציאה מתאריך 2025-04-08; ראו [W0 digest](./preview-feedback/w0/summary.md)).
- גל beta W3 במעקב; גלים עתידיים יתוזמנו לפי סקירת ממשל.

## סיכום גל W1 partners

- אישורים משפטיים וממשל. נספח שותפים נחתם 2025-04-05; אישורים הועלו ל-`DOCS-SORA-Preview-W1`.
- טלמטריה + Try it staging. כרטיס שינוי `OPS-TRYIT-147` בוצע 2025-04-06 עם snapshots Grafana ל-`docs.preview.integrity`, `TryItProxyErrors`, ו-`DocsPortal/GatewayRefusals`.
- הכנת artefact + checksum. bundle `preview-2025-04-12` אומת; לוגים של descriptor/checksum/probe נשמרו ב-`artifacts/docs_preview/W1/preview-2025-04-12/`.
- רשימת הזמנות + שליחה. שמונה בקשות שותפים (`DOCS-SORA-Preview-REQ-P01...P08`) אושרו; ההזמנות נשלחו 2025-04-12 15:00-15:21 UTC עם אישורים לכל סוקר.
- אינסטרומנטציה של משוב. office hours יומיים + checkpoints טלמטריה תועדו; ראו [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) לסיכום.
- רשימה סופית / יומן יציאה. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) מתעד כעת זמני הזמנה/ack, ראיות טלמטריה, exports של quiz, ומצביעי artefact ל-2025-04-26 כדי לאפשר שחזור על ידי הממשל.

## יומן הזמנות - W0 core maintainers

| מזהה סוקר | תפקיד | כרטיס בקשה | הזמנה נשלחה (UTC) | יציאה צפויה (UTC) | סטטוס | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Portal maintainer | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | פעיל | אישר בדיקת checksum; מתמקד בבדיקת nav/sidebar. |
| sdk-rust-01 | Rust SDK lead | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | פעיל | בודק מתכוני SDK + quickstarts של Norito. |
| sdk-js-01 | JS SDK maintainer | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | פעיל | מאמת קונסולת Try it + תהליכי ISO. |
| sorafs-ops-01 | SoraFS operator liaison | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | פעיל | מבצע audit ל-runbooks של SoraFS + תיעוד orchestration. |
| observability-01 | Observability TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | פעיל | בודק נספחי טלמטריה/אירועים; אחראי על כיסוי Alertmanager. |

כל ההזמנות מתייחסות לאותו artefact `docs-portal-preview` (הרצה 2025-03-24, תג `preview-2025-03-24`) ולוג האימות שנשמר ב-`DOCS-SORA-Preview-W0`. כל הוספה/השהיה חייבת להירשם גם בטבלה וגם ב-issue של המעקב לפני מעבר לגל הבא.

## יומן checkpoints - W0

| תאריך (UTC) | פעילות | הערות |
| --- | --- | --- |
| 2025-03-26 | סקירת טלמטריה בסיסית + office hours | `docs.preview.integrity` ו-`TryItProxyErrors` נשארו ירוקים; office hours אישרו השלמת בדיקת checksum. |
| 2025-03-27 | פרסום digest משוב ביניים | סיכום נשמר ב-[`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); שתי בעיות nav קטנות תויגו `docs-preview/w0`, ללא אירועים. |
| 2025-03-31 | בדיקת טלמטריה לשבוע האחרון | office hours אחרונות לפני יציאה; הסוקרים אישרו שהמשימות הנותרות בתהליך, ללא התראות. |
| 2025-04-08 | סיכום יציאה + סגירת הזמנות | ביקורות הושלמו, גישה זמנית בוטלה, ממצאים נשמרו ב-[`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); המעקב עודכן לפני הכנת W1. |

## יומן הזמנות - W1 partners

| מזהה סוקר | תפקיד | כרטיס בקשה | הזמנה נשלחה (UTC) | יציאה צפויה (UTC) | סטטוס | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | SoraFS operator (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | הושלם | משוב ops על orchestrator נמסר 2025-04-20; ack יציאה 15:05 UTC. |
| sorafs-op-02 | SoraFS operator (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | הושלם | הערות rollout נרשמו ב-`docs-preview/w1`; ack 15:10 UTC. |
| sorafs-op-03 | SoraFS operator (US) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | הושלם | עריכות dispute/blacklist נרשמו; ack 15:12 UTC. |
| torii-int-01 | Torii integrator | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | הושלם | walkthrough של Try it auth אושר; ack 15:14 UTC. |
| torii-int-02 | Torii integrator | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | הושלם | הערות RPC/OAuth נרשמו; ack 15:16 UTC. |
| sdk-partner-01 | SDK partner (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | הושלם | משוב integrity preview הוטמע; ack 15:18 UTC. |
| sdk-partner-02 | SDK partner (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | הושלם | סקירת telemetria/redaction הושלמה; ack 15:22 UTC. |
| gateway-ops-01 | Gateway operator | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | הושלם | הערות runbook DNS gateway נרשמו; ack 15:24 UTC. |

## יומן checkpoints - W1

| תאריך (UTC) | פעילות | הערות |
| --- | --- | --- |
| 2025-04-12 | שליחת הזמנות + אימות artefacts | שמונת השותפים קיבלו דוא"ל עם descriptor/archive `preview-2025-04-12`; אישורים נשמרו במעקב. |
| 2025-04-13 | סקירת טלמטריה בסיסית | `docs.preview.integrity`, `TryItProxyErrors`, ו-`DocsPortal/GatewayRefusals` ירוקים; office hours אישרו השלמת בדיקת checksum. |
| 2025-04-18 | office hours באמצע הגל | `docs.preview.integrity` נשאר ירוק; שני ניטים תועדו כ-`docs-preview/w1` (ניסוח nav + צילום Try it). |
| 2025-04-22 | בדיקת טלמטריה סופית | הפרוקסי והדשבורדים תקינים; אין issues חדשות, נרשם במעקב לפני יציאה. |
| 2025-04-26 | סיכום יציאה + סגירת הזמנות | כל השותפים אישרו סיום סקירה, הזמנות בוטלו, ראיות נשמרו ב-[`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## תקציר קוהורט beta W3

- ההזמנות נשלחו 2026-02-18 עם אימות checksum ואישורים באותו יום.
- משוב נאסף תחת `docs-preview/20260218` עם issue ממשל `DOCS-SORA-Preview-20260218`; digest + סיכום נוצרו דרך `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- הגישה בוטלה 2026-02-28 לאחר בדיקת טלמטריה סופית; המעקב וטבלאות הפורטל עודכנו לציון השלמת W3.

## יומן הזמנות - W2 community

| מזהה סוקר | תפקיד | כרטיס בקשה | הזמנה נשלחה (UTC) | יציאה צפויה (UTC) | סטטוס | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Community reviewer (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | הושלם | ack 16:06 UTC; מתמקד ב-quickstarts של SDK; יציאה אושרה 2025-06-29. |
| comm-vol-02 | Community reviewer (Governance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | הושלם | סקירת governance/SNS הושלמה; יציאה אושרה 2025-06-29. |
| comm-vol-03 | Community reviewer (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | הושלם | משוב walkthrough Norito נרשם; ack 2025-06-29. |
| comm-vol-04 | Community reviewer (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | הושלם | סקירת runbooks SoraFS הושלמה; ack 2025-06-29. |
| comm-vol-05 | Community reviewer (Accessibility) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | הושלם | הערות accessibility/UX שותפו; ack 2025-06-29. |
| comm-vol-06 | Community reviewer (Localization) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | הושלם | משוב localization נרשם; ack 2025-06-29. |
| comm-vol-07 | Community reviewer (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | הושלם | בדיקות docs של SDK mobile נמסרו; ack 2025-06-29. |
| comm-vol-08 | Community reviewer (Observability) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | הושלם | סקירת נספח observability הושלמה; ack 2025-06-29. |

## יומן checkpoints - W2

| תאריך (UTC) | פעילות | הערות |
| --- | --- | --- |
| 2025-06-15 | שליחת הזמנות + אימות artefacts | descriptor/archive `preview-2025-06-15` שותף עם 8 סוקרים; אישורים נשמרו במעקב. |
| 2025-06-16 | סקירת טלמטריה בסיסית | `docs.preview.integrity`, `TryItProxyErrors`, ו-`DocsPortal/GatewayRefusals` ירוקים; לוגי Try it מציגים טוקנים קהילתיים פעילים. |
| 2025-06-18 | office hours ו-triage של issues | שתי הצעות (`docs-preview/w2 #1` ניסוח tooltip, `#2` sidebar localization) - שתיהן הוקצו ל-Docs. |
| 2025-06-21 | בדיקת טלמטריה + תיקוני docs | Docs פתרו `docs-preview/w2 #1/#2`; הדשבורדים ירוקים, ללא אירועים. |
| 2025-06-24 | office hours של השבוע האחרון | הסוקרים אישרו השלמות נותרות; ללא התראות. |
| 2025-06-29 | סיכום יציאה + סגירת הזמנות | אישורים נרשמו, גישת preview בוטלה, snapshots + artefacts נשמרו (ראו [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | office hours ו-triage של issues | שתי הצעות תיעוד נרשמו תחת `docs-preview/w1`; ללא אירועים או התראות. |

## hooks לדיווח

- בכל יום רביעי, עדכנו את הטבלה למעלה ואת issue ההזמנות הפעיל עם הערה קצרה (הזמנות שנשלחו, סוקרים פעילים, אירועים).
- כאשר גל נסגר, הוסיפו את נתיב סיכום המשוב (למשל `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) וקשרו אותו מ-`status.md`.
- אם קריטריון pause מה-[preview invite flow](./preview-invite-flow.md) מופעל, הוסיפו כאן את צעדי ה-remediation לפני חידוש ההזמנות.
