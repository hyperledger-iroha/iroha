---
lang: he
direction: rtl
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2026-01-03T18:07:59.200257+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# תבנית תזכיר חתימה משפטית של האיחוד האירופי AND6

תזכיר זה מתעד את הביקורת המשפטית הנדרשת לפי פריט מפת הדרכים **AND6** לפני ה
חבילת חפצים של האיחוד האירופי (ETSI/GDPR) מוגשת לרגולטורים. היועץ צריך לשכפל
תבנית זו לכל מהדורה, מלא את השדות למטה ואחסן את העותק החתום
לצד החפצים הבלתי ניתנים לשינוי המוזכרים בתזכיר.

## סיכום

- **שחרור / רכבת:** `<e.g., 2026.1 GA>`
- **תאריך סקירה:** `<YYYY-MM-DD>`
- **יועץ / מבקר:** `<name + organisation>`
- **היקף:** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- **כרטיסים נלווים:** `<governance or legal issue IDs>`

## רשימת חפצים

| חפץ | SHA-256 | מיקום / קישור | הערות |
|--------|--------|----------------|-------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + ארכיון ממשל | אשר מזהי שחרור והתאמות של מודל איומים. |
| `gdpr_dpia_summary.md` | `<hash>` | אותה ספרייה / מראות לוקליזציה | ודא שהפניות למדיניות הסריקה תואמות ל-`sdk/android/telemetry_redaction.md`. |
| `sbom_attestation.md` | `<hash>` | אותה ספרייה + חבילת cosign בדלי ראיות | אמת חתימות CycloneDX + מקור. |
| שורת יומן הוכחות | `<hash>` | `docs/source/compliance/android/evidence_log.csv` | מספר שורה `<n>` |
| חבילת מגירה של מכשירי מעבדה | `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` | מאשרת חזרת כשל הקשורה לגרסה זו. |

> צרף שורות נוספות אם החבילה מכילה יותר קבצים (לדוגמה, פרטיות
> נספחים או תרגומי DPIA). כל חפץ חייב להתייחס לבלתי משתנה שלו
> יעד העלאה ועבודת Buildkite שיצרה אותו.

## ממצאים וחריגים

- `None.` *(החלף ברשימת תבליטים המכסה סיכונים שיוריים, מפצה
  בקרות, או פעולות מעקב נדרשות.)*

## אישור

- **החלטה:** `<Approved / Approved with conditions / Blocked>`
- **חתימה / חותמת זמן:** `<digital signature or email reference>`
- **בעלי מעקב:** `<team + due date for any conditions>`

העלה את התזכיר הסופי לדלי הראיות הממשל, העתק את ה-SHA-256 לתוך
`docs/source/compliance/android/evidence_log.csv`, וקשר את נתיב ההעלאה פנימה
`status.md`. אם ההחלטה היא "חסומה", הסלמה להיגוי AND6
שלבי תיקון הוועדה והמסמכים הן ברשימת החמים של מפת הדרכים והן
יומן מגירה של מעבדת מכשיר.