---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: priority-snapshot-2025-03
title: ترجیحات اسنیپ شاٹ — مارچ 2025 (بیٹا)
description: Nexus 2025-03 steering snapshot کا عکس؛ عوامی rollout سے پہلے ACKs کا انتظار۔
---

> מוצר מוצר: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> اسٹیٹس: **بیٹا / steering ACKs کا انتظار** (Networking, Storage, Docs leads).

## جائزہ

مارچ اسنیپ شاٹ docs/content-network کی initiatives کو SoraFS delivery tracks
(SF-3, SF-6b, SF-9) کے ساتھ aligned رکھتا ہے۔ جیسے ہی تمام leads Nexus steering
چینل میں snapshot کی تصدیق کر دیں، اوپر والی “Beta” نوٹ ہٹا دیں۔

### فوکس تھریڈز

1. **ترجیحات اسنیپ شاٹ circulate کریں** — acknowledgements جمع کریں اور انہیں
   2025-03-05 council minutes میں لاگ کریں۔
2. **סגירת בעיטת שער/DNS** — סדנת 2025-03-03 הנחייה
   kit (runbook کا Section 6) rehearse کریں۔
3. **Operator runbook migration** — portal `Runbook Index` live ہے؛ מבקר
   onboarding sign-off کے بعد beta preview URL ظاہر کریں۔
4. **SoraFS delivery threads** — SF-3/6b/9 کا باقی کام plan/roadmap کے ساتھ align کریں:
   - `sorafs-node` עובד בליעת PoR + נקודת קצה סטטוס.
   - שילובי חלודה/JS/Swift מתזמר עם כריכות CLI/SDK ופוליש.
   - חיווט זמן ריצה של מתאם PoR ואירועי GovernanceLog.

مکمل جدول، distribution checklist اور log entries کے لیے source فائل دیکھیں۔