---
id: priority-snapshot-2025-03
lang: he
direction: rtl
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

> מקור קנוני: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> סטטוס: **בטא / ממתין ל-ACKs של steering** (Networking, Storage, Docs leads).

## סקירה

תמונת מרץ שומרת על יוזמות docs/content-network מיושרות עם מסלולי המסירה של
SoraFS (SF-3, SF-6b, SF-9). לאחר שכל ה-leads יאשרו את ה-snapshot בערוץ Nexus
steering, הסירו את הערת “Beta” שמעל.

### נושאי מיקוד

1. **הפצת snapshot העדיפויות** — איסוף acknowledgements ורישומם בפרוטוקול
   council בתאריך 2025-03-05.
2. **סגירת kickoff של Gateway/DNS** — לתרגל את ערכת ההנחיה החדשה (סעיף 6 ב-runbook)
   לפני ה-workshop ב-2025-03-03.
3. **הגירת runbook למפעילים** — הפורטל `Runbook Index` פעיל; חשפו את URL של
   beta preview אחרי sign-off של onboarding ל-reviewers.
4. **חוטי מסירה של SoraFS** — ליישר את העבודה שנותרה ב-SF-3/6b/9 עם plan/roadmap:
   - עובד ingestion של PoR + endpoint סטטוס ב-`sorafs-node`.
   - polishing של bindings ל-CLI/SDK באינטגרציות orchestrator Rust/JS/Swift.
   - חיווט runtime של מתאם PoR ואירועי GovernanceLog.

ראו את קובץ המקור לטבלה המלאה, checklist להפצה ורישומי לוג.
