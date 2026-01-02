---
lang: he
direction: rtl
source: docs/examples/android_partner_sla_notes_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f1d3e5d2b42f7e6f9c2de4f2be56b6994b4b88b109f70edc7e6f04ec0f3465ac
source_last_modified: "2025-11-12T08:32:28.349523+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/android_partner_sla_notes_template.md -->

# הערות גילוי SLA לשותף Android - תבנית

השתמשו בתבנית זו לכל פגישת גילוי SLA של AND8. שמרו את העותק המלא תחת
`docs/source/sdk/android/partner_sla_sessions/<partner>/<date>/minutes.md`
וצרפו artefacts תומכים (תשובות שאלון, אישורים, קבצים מצורפים) באותה תיקיה.

```
Partner: <Name>                      Date: <YYYY-MM-DD>  Time: <UTC>
Primary contact(s): <names, roles, email>
Android attendees: <Program Lead / Partner Eng / Support Eng / Compliance>
Meeting link / ticket: <URL or ID>
```

## 1. אג'נדה והקשר

- מטרת הפגישה (היקף פיילוט, חלון ריליס, ציפיות טלמטריה).
- מסמכי ייחוס ששיתפנו לפני השיחה (support playbook, לוח ריליסים,
  דשבורדי טלמטריה).

## 2. סקירת עומס

| נושא | הערות |
|------|-------|
| עומסים יעד / רשתות | |
| נפח עסקאות צפוי | |
| חלונות עסקיים קריטיים / תקופות blackout | |
| משטרים רגולטוריים (GDPR, MAS, FISC וכו') | |
| שפות נדרשות / לוקליזציה | |

## 3. דיון SLA

| מחלקת SLA | ציפיית השותף | חריגה מה-baseline? | פעולה נדרשת |
|-----------|---------------|--------------------|-------------|
| תיקון קריטי (48 h) | | Yes/No | |
| חומרה גבוהה (5 business days) | | Yes/No | |
| תחזוקה (30 days) | | Yes/No | |
| הודעת cutover (60 days) | | Yes/No | |
| קצב תקשורת תקריות | | Yes/No | |

תעדו סעיפי SLA נוספים שהשותף מבקש (למשל גשר טלפוני ייעודי, ייצואי טלמטריה נוספים).

## 4. דרישות טלמטריה וגישה

- צרכי גישה ל-Grafana / Prometheus:
- דרישות יצוא logs/traces:
- ציפיות לראיות offline או dossier:

## 5. הערות ציות ומשפט

- דרישות התראה שיפוטית (חוק + תזמון).
- אנשי קשר משפטיים נדרשים לעדכוני תקריות.
- מגבלות תושבות נתונים / דרישות אחסון.

## 6. החלטות ופעולות

| פריט | Owner | תאריך יעד | הערות |
|------|-------|-----------|-------|
| | | | |

## 7. אישור

- השותף אישר SLA בסיסי? (Y/N)
- שיטת אישור המשך (email / ticket / signature):
- צרפו אימייל אישור או פרוטוקול פגישה לתיקיה זו לפני סגירה.

</div>
