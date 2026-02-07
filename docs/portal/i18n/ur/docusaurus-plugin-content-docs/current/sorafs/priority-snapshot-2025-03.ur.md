---
lang: ur
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

> مستند ماخذ: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> اسٹیٹس: **بیٹا / steering ACKs کا انتظار** (Networking, Storage, Docs leads).

## جائزہ

مارچ اسنیپ شاٹ docs/content-network کی initiatives کو SoraFS delivery tracks
(SF-3, SF-6b, SF-9) کے ساتھ aligned رکھتا ہے۔ جیسے ہی تمام leads Nexus steering
چینل میں snapshot کی تصدیق کر دیں، اوپر والی “Beta” نوٹ ہٹا دیں۔

### فوکس تھریڈز

1. **ترجیحات اسنیپ شاٹ circulate کریں** — acknowledgements جمع کریں اور انہیں
   2025-03-05 council minutes میں لاگ کریں۔
2. **Gateway/DNS kickoff close-out** — 2025-03-03 workshop سے پہلے نیا facilitation
   kit (runbook کا Section 6) rehearse کریں۔
3. **Operator runbook migration** — portal `Runbook Index` live ہے؛ reviewer
   onboarding sign-off کے بعد beta preview URL ظاہر کریں۔
4. **SoraFS delivery threads** — SF-3/6b/9 کا باقی کام plan/roadmap کے ساتھ align کریں:
   - `sorafs-node` میں PoR ingestion worker + status endpoint۔
   - Rust/JS/Swift orchestrator integrations میں CLI/SDK bindings کا polish۔
   - PoR coordinator runtime wiring اور GovernanceLog events۔

مکمل جدول، distribution checklist اور log entries کے لیے source فائل دیکھیں۔
