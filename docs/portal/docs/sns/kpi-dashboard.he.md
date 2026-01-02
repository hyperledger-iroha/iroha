---
lang: he
direction: rtl
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3649db9b00f9be968cfeb98bc34bbc797aaf22d7ac3936698b4f562094911073
source_last_modified: "2025-11-15T05:28:18.938087+00:00"
translation_last_reviewed: 2026-01-01
---

# לוח KPI של Sora Name Service

לוח ה-KPI נותן ל-stewards, guardians ורגולטורים מקום אחד לסקור אותות אימוץ, שגיאה והכנסות לפני קצב הנספח החודשי (SN-8a). הגדרת Grafana נמצאת במאגר ב-`dashboards/grafana/sns_suffix_analytics.json`, והפורטל משקף את אותם לוחות דרך iframe מוטמע כדי שהחוויה תתאים למופע Grafana הפנימי.

## מסננים ומקורות נתונים

- **מסנן סיומת** – מניע את השאילתות `sns_registrar_status_total{suffix}` כך שניתן לבדוק את `.sora`, `.nexus` ו-`.dao` בנפרד.
- **מסנן שחרור מרוכז** – מצמצם את מדדי `sns_bulk_release_payment_*` כדי שהפיננסים יוכלו לבצע התאמה למניפסט רישום מסוים.
- **מדדים** – נמשכים מ-Torii (`sns_registrar_status_total`, `torii_request_duration_seconds`), מ-CLI של guardian (`guardian_freeze_active`), `sns_governance_activation_total`, ומדדי עוזר ה-bulk-onboarding.

## לוחות

1. **רישומים (24h אחרונות)** – מספר אירועי registrar מוצלחים עבור הסיומת שנבחרה.
2. **הפעלות ממשל (30d)** – יוזמות אמנה/תוספת שנרשמו על ידי ה-CLI.
3. **תפוקת registrar** – קצב פעולות registrar מוצלחות לכל סיומת.
4. **מצבי שגיאה של registrar** – קצב של 5 דקות של מוני `sns_registrar_status_total` שמסומנים כשגיאה.
5. **חלונות הקפאה של guardian** – סלקטורים חיים שבהם `guardian_freeze_active` מדווח על כרטיס הקפאה פתוח.
6. **יחידות תשלום נטו לפי נכס** – סכומים שדווחו ע"י `sns_bulk_release_payment_net_units` לכל נכס.
7. **בקשות מרוכזות לפי סיומת** – נפחי מניפסט לכל מזהה סיומת.
8. **יחידות נטו לבקשה** – חישוב בסגנון ARPU הנגזר ממדדי ה-release.

## צ'ק-ליסט חודשי לסקירת KPI

מוביל הכספים מבצע סקירה מחזורית בכל יום שלישי הראשון בכל חודש:

1. פתחו את דף הפורטל **Analytics → SNS KPI** (או לוח Grafana `sns-kpis`).
2. לכדו יצוא PDF/CSV של טבלאות תפוקת registrar והכנסות.
3. השוו סיומות לאיתור הפרות SLA (קפיצות בשיעור שגיאה, סלקטורים קפואים >72 h, פערי ARPU >10%).
4. רשמו סיכומים + פעולות בערך הנספח הרלוונטי תחת `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
5. צרפו את ארטיפקטי הלוח המיוצאים לקומיט הנספח וקשרו אותם בסדר היום של המועצה.

אם הסקירה מגלה הפרות SLA, פתחו אירוע PagerDuty עבור הבעלים המושפע (registrar duty manager, guardian on-call, או steward program lead) ועקבו אחר התיקון ביומן הנספח.
