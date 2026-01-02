---
lang: he
direction: rtl
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-11-21T14:25:28.923348+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/soranet_testnet_operator_kit/06-verification-report.md -->

## דוח אימות מפעיל (שלב T0)

- שם המפעיל: ______________________
- מזהה descriptor של relay: ______________________
- תאריך הגשה (UTC): ___________________
- דוא"ל / matrix ליצירת קשר: ___________________

### סיכום רשימת בדיקה

| פריט | הושלם (כן/לא) | הערות |
|------|-----------------|-------|
| אימות חומרה ורשת | | |
| יישום בלוק compliance | | |
| אימות מעטפת קבלה | | |
| בדיקת smoke ל-guard rotation | | |
| טלמטריה נאספה ודשבורדים פעילים | | |
| ביצוע brownout drill | | |
| הצלחת כרטיסי PoW במסגרת היעד | | |

### תמונת מצב של מדדים

- יחס PQ (`sorafs_orchestrator_pq_ratio`): ________
- מספר downgrade ב-24 השעות האחרונות: ________
- RTT ממוצע של מעגלים (p95): ________ ms
- זמן פתרון חציוני PoW: ________ ms

### מצורפים

נא לצרף:

1. hash של support bundle ל-relay (`sha256`): __________________________
2. צילומי דשבורדים (יחס PQ, הצלחת מעגלים, היסטוגרמת PoW).
3. חבילת drill חתומה (`drills-signed.json` + מפתח ציבורי של החותם בהקס ומצרפים).
4. דוח מדדים SNNet-10 (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`).

### חתימת המפעיל

אני מאשר שהמידע לעיל מדויק ושכל השלבים הנדרשים הושלמו.

חתימה: _________________________  תאריך: ___________________

</div>
