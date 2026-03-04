---
lang: he
direction: rtl
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-11-15T09:17:21.371048+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/sns_training_template.md -->

# תבנית שקופיות אימון SNS

מתווה Markdown זה משקף את השקופיות שעל המנחים להתאים לקבוצות השפה שלהם. העתיקו את הסעיפים האלה ל-Keynote/PowerPoint/Google Slides והתאימו את הנקודות, הצילומים והתרשימים לפי הצורך.

## שקופית כותרת
- תוכנית: "Sora Name Service onboarding"
- כותרת משנה: ציינו suffix + cycle (לדוגמה: `.sora - 2026-03`)
- מציגים + שיוכים

## אוריינטציה ל-KPI
- צילום מסך או הטמעה של `docs/portal/docs/sns/kpi-dashboard.md`
- רשימת נקודות שמסבירה מסנני suffix, טבלת ARPU, ומעקב freeze
- דגשים ליצוא PDF/CSV

## מחזור חיי manifest
- תרשים: registrar -> Torii -> governance -> DNS/gateway
- שלבים עם הפניה ל-`docs/source/sns/registry_schema.md`
- דוגמת קטע manifest עם הערות

## תרגילי מחלוקת והקפאה
- תרשים זרימה להתערבות guardian
- צ'קליסט עם הפניה ל-`docs/source/sns/governance_playbook.md`
- דוגמת ציר זמן של כרטיס freeze

## לכידת annex
- מקטע פקודה שמראה `cargo xtask sns-annex ... --portal-entry ...`
- תזכורת לארכב Grafana JSON תחת `artifacts/sns/regulatory/<suffix>/<cycle>/`
- קישור ל-`docs/source/sns/reports/.<suffix>/<cycle>.md`

## צעדים הבאים
- קישור למשוב הדרכה (ראו `docs/examples/sns_training_eval_template.md`)
- כתובות ערוצי Slack/Matrix
- תאריכי אבני דרך קרובות

</div>
