---
lang: he
direction: rtl
source: docs/examples/sns/arbitration_transparency_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 305a3f3b253a013825d4dd798d2282e111913ec777fe0fbf5b02a92c7172b92a
source_last_modified: "2025-11-15T07:34:14.070551+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/sns/arbitration_transparency_report_template.md -->

# דוח שקיפות בוררות SNS - <Month YYYY>

- **סיומת:** `<.sora / .nexus / .dao>`
- **חלון דיווח:** `<ISO start>` -> `<ISO end>`
- **הוכן על ידי:** `<Council liaison>`
- **Artefacts מקוריים:** `cases.ndjson` SHA256 `<hash>`, ייצוא דשבורד `<filename>.json`

## 1. תקציר מנהלים

- סך תיקים חדשים: `<count>`
- תיקים שנסגרו בתקופה: `<count>`
- עמידה ב-SLA: `<ack %>` acknowledge / `<resolution %>` decision
- overrides של guardian שהונפקו: `<count>`
- העברות/החזרים שבוצעו: `<count>`

## 2. חלוקת תיקים

| סוג מחלוקת | תיקים חדשים | תיקים שנסגרו | חציון פתרון (ימים) |
|------------|-------------|---------------|--------------------|
| בעלות | 0 | 0 | 0 |
| הפרת מדיניות | 0 | 0 | 0 |
| ניצול לרעה | 0 | 0 | 0 |
| חיוב | 0 | 0 | 0 |
| אחר | 0 | 0 | 0 |

## 3. ביצועי SLA

| עדיפות | SLA לאישור | הושג | SLA לפתרון | הושג | חריגות |
|--------|-----------|------|-----------|------|--------|
| דחוף | <= 2 h | 0% | <= 72 h | 0% | 0 |
| גבוה | <= 8 h | 0% | <= 10 d | 0% | 0 |
| סטנדרטי | <= 24 h | 0% | <= 21 d | 0% | 0 |
| Info | <= 3 d | 0% | <= 30 d | 0% | 0 |

תארו את גורמי השורש לכל חריגה וקשרו לכרטיסי remediation.

## 4. רישום תיקים

| מזהה תיק | סלקטור | עדיפות | סטטוס | תוצאה | הערות |
|----------|--------|--------|-------|-------|-------|
| SNS-YYYY-NNNNN | `label.suffix` | Standard | Closed | Upheld | `<summary>` |

ספקו הערות של שורה אחת המפנות לעובדות אנונימיות או לקישורי הצבעה פומביים. חתמו היכן שנדרש
והזכירו את ההשחרות שבוצעו.

## 5. פעולות וסעדים

- **הקפאות / שחרורים:** `<counts + case ids>`
- **העברות:** `<counts + assets moved>`
- **התאמות חיוב:** `<credits/debits>`
- **מעקבי מדיניות:** `<tickets or RFCs opened>`

## 6. ערעורים ו overrides של guardian

סכמו ערעורים שהוסלמו ל-guardian board, כולל timestamps והחלטות
(approve/deny). קשרו לרשומות `sns governance appeal` או להצבעות council.

## 7. פריטים פתוחים

- `<Action item>` - Owner `<name>`, ETA `<date>`
- `<Action item>` - Owner `<name>`, ETA `<date>`

צרפו NDJSON, יצואי Grafana ולוגים של CLI המוזכרים בדוח זה.

</div>
