---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/nexus/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a58f511cf45f1e21a0df81381c93b0ff31cb0623f8fe4fcdc3466354913283e
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/nexus/overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-overview
title: סקירת Sora Nexus
description: סיכום ברמה גבוהה של ארכיטקטורת Iroha 3 (Sora Nexus) עם הפניות למסמכי המונו-ריפו הקנוניים.
---

Nexus (Iroha 3) מרחיב את Iroha 2 עם ביצוע multi-lane, מרחבי נתונים תחומי-ממשל וכלי עבודה משותפים בכל SDK. דף זה משקף את התקציר החדש `docs/source/nexus_overview.md` במונו-ריפו כדי שקוראי הפורטל יבינו במהירות כיצד חלקי הארכיטקטורה משתלבים.

## קווי שחרור

- **Iroha 2** - פריסות בהוסט עצמי לקונסורציומים או רשתות פרטיות.
- **Iroha 3 / Sora Nexus** - הרשת הציבורית multi-lane שבה מפעילים רושמים מרחבי נתונים (DS) ומקבלים כלי ממשל, סליקה ותצפיתיות משותפים.
- שתי השורות קומפליות מאותו workspace (IVM + toolchain Kotodama), כך שתיקוני SDK, עדכוני ABI ו-fixtures של Norito נשארים ניידים. מפעילים מורידים את החבילה `iroha3-<version>-<os>.tar.zst` כדי להצטרף ל-Nexus; עיינו ב-`docs/source/sora_nexus_operator_onboarding.md` לרשימת הבדיקה המלאה.

## אבני בניין

| רכיב | סיכום | קישורי פורטל |
|-----------|---------|--------------|
| מרחב נתונים (DS) | תחום ביצוע/אחסון מוגדר ממשל שמחזיק נתיב אחד או יותר, מגדיר סטים של מאמתים, מחלקת פרטיות ומדיניות עמלות + DA. | ראו [Nexus spec](./nexus-spec) לסכמת המניפסט. |
| Lane | שבר ביצוע דטרמיניסטי; מפיק התחייבויות שמסדרות טבעת NPoS הגלובלית. מחלקות Lane כוללות `default_public`, `public_custom`, `private_permissioned` ו-`hybrid_confidential`. | [מודל Lane](./nexus-lane-model) מתאר גיאומטריה, קידומות אחסון ושימור. |
| תוכנית מעבר | מזהי placeholder, שלבי ניתוב ואריזת פרופיל כפול עוקבים אחרי מעבר מפריסות חד-נתיביות אל Nexus. | [הערות מעבר](./nexus-transition-notes) מתעדות כל שלב הגירה. |
| Space Directory | חוזה רישום שמאחסן מניפסטים + גרסאות של DS. מפעילים משווים רשומות קטלוג מול הספריה הזו לפני הצטרפות. | מעקב diffs של מניפסט נמצא תחת `docs/source/project_tracker/nexus_config_deltas/`. |
| קטלוג נתיבים | סעיף התצורה `[nexus]` ממפה מזהי Lane לכינויים, מדיניות ניתוב וספי DA. `irohad --sora --config … --trace-config` מדפיס את הקטלוג המפוענח לצורכי ביקורת. | השתמשו ב-`docs/source/sora_nexus_operator_onboarding.md` למסלול ה-CLI. |
| נתב סליקה | מתזמר העברות XOR שמחבר נתיבי CBDC פרטיים לנתיבי נזילות ציבוריים. | `docs/source/cbdc_lane_playbook.md` מפרט כפתורי מדיניות ושערי טלמטריה. |
| טלמטריה/SLOs | לוחות מחוונים + התראות תחת `dashboards/grafana/nexus_*.json` תופסים גובה נתיבים, עומס DA, זמן סליקה ועומק תור ממשל. | [תוכנית שיקום טלמטריה](./nexus-telemetry-remediation) מפרטת את הלוחות, ההתראות וראיות הביקורת. |

## תמונת מצב של פריסה

| שלב | מיקוד | קריטריוני יציאה |
|-------|-------|---------------|
| N0 - בטא סגורה | Registrar מנוהל מועצה (`.sora`), קליטת מפעילים ידנית, קטלוג נתיבים סטטי. | מניפסטים של DS חתומים + מסירות ממשל מתורגלות. |
| N1 - השקה ציבורית | מוסיף סיומות `.nexus`, מכרזים, registrar בשירות עצמי, חיווט סליקה XOR. | בדיקות סנכרון resolver/gateway, לוחות פיוס חיובים, תרגילי מחלוקת. |
| N2 - הרחבה | מציג `.dao`, APIs למשווקים, אנליטיקה, פורטל מחלוקות, scorecards של stewards. | ארטיפקטים של תאימות בגרסאות, ערכת jury למדיניות באוויר, דוחות שקיפות אוצר. |
| שער NX-12/13/14 | מנוע תאימות, לוחות טלמטריה ותיעוד חייבים לצאת יחד לפני פיילוטים עם שותפים. | [Nexus overview](./nexus-overview) + [Nexus operations](./nexus-operations) פורסמו, לוחות חוברו, מנוע מדיניות מוזג. |

## אחריות מפעילים

1. **היגיינת תצורה** - שמרו על `config/config.toml` מסונכרן עם קטלוג הנתיבים ומרחבי הנתונים שפורסם; ארכבו פלט `--trace-config` עם כל כרטיס שחרור.
2. **מעקב מניפסטים** - השוו רשומות קטלוג עם חבילת Space Directory האחרונה לפני הצטרפות או שדרוג צמתים.
3. **כיסוי טלמטריה** - חשפו את `nexus_lanes.json`, `nexus_settlement.json` ולוחות SDK קשורים; חברו התראות ל-PagerDuty והפעילו סקירות רבעוניות לפי תוכנית שיקום הטלמטריה.
4. **דיווח תקריות** - פעלו לפי מטריצת החומרה ב-[Nexus operations](./nexus-operations) והגישו RCAs בתוך חמישה ימי עסקים.
5. **מוכנות ממשל** - השתתפו בהצבעות מועצת Nexus שמשפיעות על הנתיבים שלכם ותרגלו הוראות rollback רבעונית (נעקב ב-`docs/source/project_tracker/nexus_config_deltas/`).

## ראו גם

- סקירה קנונית: `docs/source/nexus_overview.md`
- מפרט מפורט: [./nexus-spec](./nexus-spec)
- גיאומטריית נתיבים: [./nexus-lane-model](./nexus-lane-model)
- תוכנית מעבר: [./nexus-transition-notes](./nexus-transition-notes)
- תוכנית שיקום טלמטריה: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook תפעול: [./nexus-operations](./nexus-operations)
- מדריך קליטת מפעילים: `docs/source/sora_nexus_operator_onboarding.md`
