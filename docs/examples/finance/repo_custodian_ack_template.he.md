---
lang: he
direction: rtl
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6609b9628186b58c0441d1ca1594a3030b7f7d81bb2df9c36af3a9d65cdd963d
source_last_modified: "2025-11-15T20:04:53.140162+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/finance/repo_custodian_ack_template.md -->

# תבנית אישור נאמן Repo

השתמשו בתבנית זו כאשר repo (דו-צדדי או tri-party) מפנה לנאמן דרך `RepoAgreement::custodian`. המטרה היא לתעד את SLA המשמורת, חשבונות הניתוב ואנשי קשר לתרגילים לפני העברת נכסים. העתיקו את התבנית לתיקיית הראיות (לדוגמה
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`), מלאו את ה-placeholders, וחשבו hash לקובץ כחלק מחבילת הממשל המתוארת ב-
`docs/source/finance/repo_ops.md` סעיף 2.8.

## 1. מטאדאטה

| שדה | ערך |
|------|------|
| מזהה הסכם | `<repo-yyMMdd-XX>` |
| מזהה חשבון נאמן | `<ih58...>` |
| הוכן על ידי / תאריך | `<custodian ops lead>` |
| אנשי קשר desk שאושרו | `<desk lead + counterparty>` |
| תיקיית ראיות | ``artifacts/finance/repo/<slug>/`` |

## 2. היקף משמורת

- **הגדרות ביטחונות שהתקבלו:** `<list of asset definition ids>`
- **מטבע cash leg / מסילת settlement:** `<xor#sora / other>`
- **חלון משמורת:** `<start/end timestamps or SLA summary>`
- **הוראות קבע:** `<hash + path to standing instruction document>`
- **דרישות אוטומציה:** `<scripts, configs, or runbooks custodian will invoke>`

## 3. ניתוב ומעקב

| פריט | ערך |
|------|------|
| ארנק משמורת / חשבון ledger | `<asset ids or ledger path>` |
| ערוץ ניטור | `<Slack/phone/on-call rotation>` |
| איש קשר drill | `<primary + backup>` |
| התראות נדרשות | `<PagerDuty service, Grafana board, etc.>` |

## 4. הצהרות

1. *Custody readiness:* "בחַנו את payload `repo initiate` המדורג עם המזהים לעיל ואנו
   מוכנים לקבל ביטחונות לפי ה-SLA המפורט בסעיף 2."
2. *Rollback commitment:* "נבצע את playbook ה-rollback הנ"ל לפי הנחיית מפקד התקרית,
   ונספק לוגי CLI ו-hashes ב-`governance/drills/<timestamp>.log`."
3. *Evidence retention:* "נשמור את האישור, הוראות הקבע ולוגי ה-CLI למשך לפחות `<duration>`
   ונספק אותם למועצת הכספים לפי בקשה."

חתמו למטה (חתימות אלקטרוניות מתקבלות כאשר הן עוברות דרך מעקב הממשל).

| שם | תפקיד | חתימה / תאריך |
|------|------|------------------|
| `<custodian ops lead>` | מפעיל נאמן | `<signature>` |
| `<desk lead>` | Desk | `<signature>` |
| `<counterparty>` | Counterparty | `<signature>` |

> לאחר חתימה, חשבו hash לקובץ (לדוגמה: `sha256sum custodian_ack_<cust>.md`) ורשמו את ה-digest בטבלת חבילת הממשל כדי שמבקרים יוכלו לאמת את הבייטים של האישור שהוזכרו בהצבעה.

</div>
