---
lang: he
direction: rtl
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1add138352dabf2433c36c9abac24085af8c7e53dca8bc579d73b37680e470cf
source_last_modified: "2025-11-22T04:59:33.125102+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/soranet_incentive_parliament_packet/rollback_plan.md -->

# תוכנית rollback לתמריצי Relay

השתמשו ב-playbook זה כדי להשבית תשלומי relay אוטומטיים אם הממשל מבקש עצירה
או אם ספי הטלמטריה מופעלים.

1. **הקפאת אוטומציה.** עצרו את daemon התמריצים בכל host של orchestrator
   (`systemctl stop soranet-incentives.service` או פריסת container שקולה) ואשרו שהתהליך אינו רץ.
2. **ריקון הוראות ממתינות.** הריצו
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   כדי לוודא שאין הוראות payout ממתינות. ארכבו את ה-payloads של Norito לביקורת.
3. **ביטול אישור ממשל.** ערכו את `reward_config.json`, הגדירו
   `"budget_approval_id": null`, ופרסו מחדש את התצורה עם
   `iroha app sorafs incentives service init` (או `update-config` אם daemon ארוך טווח פועל). מנוע התשלומים נכשל כעת סגור עם
   `MissingBudgetApprovalId`, ולכן ה-daemon מסרב להטביע תשלומים עד שחוזר hash אישור חדש. רשמו את commit ה-git ואת SHA-256
   של התצורה המותאמת ביומן התקרית.
4. **הודעה לפרלמנט Sora.** צרפו את ledger התשלומים שנוקז, את דוח ה-shadow-run, וסיכום תקרית קצר. פרוטוקולי הפרלמנט חייבים לציין את hash התצורה שבוטלה ואת שעת עצירת ה-daemon.
5. **אימות rollback.** השאירו את ה-daemon מושבת עד ש:
   - התראות הטלמטריה (`soranet_incentives_rules.yml`) ירוקות למשך >=24 שעות,
   - דוח התאמת האוצר מציג אפס העברות חסרות, ו-
   - הפרלמנט מאשר hash תקציב חדש.

לאחר שהממשל מנפיק מחדש hash אישור תקציב, עדכנו את `reward_config.json`
עם ה-digest החדש, הריצו שוב את הפקודה `shadow-run` על הטלמטריה העדכנית,
והפעילו מחדש את daemon התמריצים.

</div>
