---
lang: he
direction: rtl
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: training-collateral
title: חומרי הדרכה SNS
description: תכנית לימודים, זרימת לוקליזציה ואיסוף הוכחות נספחים כנדרש ב‑SN-8.
---

> משקף `docs/source/sns/training_collateral.md`. השתמשו בדף זה בעת תדרוך צוותי רשם, DNS, guardians ופיננסים לפני כל השקת סיומת.

## 1. תמונת מצב של התכנית

| מסלול | יעדים | קריאה מוקדמת |
|-------|------------|-----------|
| תפעול רשם | הגשת manifests, ניטור לוחות KPI, הסלמת שגיאות. | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS ושער | יישום skeletons של resolver, תרגול הקפאה/rollback. | `sorafs/gateway-dns-runbook`, direct-mode policy samples. |
| Guardians ומועצה | ביצוע סכסוכים, עדכון נספחי ממשל, רישום נספחים. | `sns/governance-playbook`, steward scorecards. |
| פיננסים ואנליטיקה | איסוף מדדי ARPU/bulk, פרסום חבילות נספחים. | `finance/settlement-iso-mapping`, KPI dashboard JSON. |

### זרימת המודולים

1. **M1 — אוריינטציית KPI (30 ד׳):** מעבר על מסנני סיומת, ייצוא ומוני הקפאה. תוצר: snapshots ב‑PDF/CSV עם digest SHA‑256.
2. **M2 — מחזור חיי manifest (45 ד׳):** בנייה ואימות manifests של הרשם, יצירת skeletons של resolver באמצעות `scripts/sns_zonefile_skeleton.py`. תוצר: diff של git המציג את ה-skeleton + הוכחת GAR.
3. **M3 — תרגילי סכסוך (40 ד׳):** סימולציית הקפאה + ערעור של guardian, שמירת לוגי CLI תחת `artifacts/sns/training/<suffix>/<cycle>/logs/`.
4. **M4 — איסוף נספחים (25 ד׳):** ייצוא JSON של הדשבורד והרצה:

   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   תוצר: Markdown נספח מעודכן + מזכר רגולטורי + בלוקים לפורטל.

## 2. זרימת לוקליזציה

- שפות: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, `ur`.
- כל תרגום נמצא לצד קובץ המקור (`docs/source/sns/training_collateral.<lang>.md`). עדכנו `status` + `translation_last_reviewed` לאחר רענון.
- נכסים לכל שפה נמצאים תחת `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (slides/, workbooks/, recordings/, logs/).
- הריצו `python3 scripts/sync_docs_i18n.py --lang <code>` לאחר עריכת מקור האנגלית כדי שהמתרגמים יראו את ה-hash החדש.

### רשימת מסירה

1. עדכנו את stub התרגום (`status: complete`) לאחר לוקליזציה.
2. ייצאו שקופיות ל‑PDF והעלו לתיקיית `slides/` לפי שפה.
3. הקליטו walkthrough KPI באורך ≤10 דקות; קשרו מה-stub של השפה.
4. פתחו כרטיס ממשל עם התג `sns-training` המכיל digests של שקופיות/workbook, קישורי הקלטה והוכחות נספחים.

## 3. נכסי הדרכה

- תבנית שקופיות: `docs/examples/sns_training_template.md`.
- תבנית workbook: `docs/examples/sns_training_workbook.md` (אחד לכל משתתף).
- הזמנות + תזכורות: `docs/examples/sns_training_invite_email.md`.
- טופס הערכה: `docs/examples/sns_training_eval_template.md` (תגובות מאוחסנות תחת `artifacts/sns/training/<suffix>/<cycle>/feedback/`).

## 4. תזמון ומדדים

| מחזור | חלון | מדדים | הערות |
|-------|--------|---------|-------|
| 2026‑03 | לאחר סקירת KPI | נוכחות %, digest נספח נרשם | `.sora` + `.nexus` cohorts |
| 2026‑06 | לפני GA של `.dao` | מוכנות פיננסית ≥90 % | כולל ריענון מדיניות |
| 2026‑09 | התרחבות | תרגיל סכסוך <20 ד׳, SLA נספח ≤2 ימים | יישור עם תמריצי SN‑7 |

אספו משוב אנונימי ב‑`docs/source/sns/reports/sns_training_feedback.md` כדי שקבוצות הבאות ישפרו את הלוקליזציה והמעבדות.

