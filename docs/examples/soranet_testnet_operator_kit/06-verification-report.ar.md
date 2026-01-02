---
lang: ar
direction: rtl
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-11-21T14:25:28.923348+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/soranet_testnet_operator_kit/06-verification-report.md -->

## تقرير تحقق المشغل (المرحلة T0)

- اسم المشغل: ______________________
- معرف واصف relay: ______________________
- تاريخ التقديم (UTC): ___________________
- بريد التواصل / matrix: ___________________

### ملخص قائمة التحقق

| البند | مكتمل (نعم/لا) | ملاحظات |
|------|-----------------|---------|
| تم التحقق من العتاد والشبكة | | |
| تطبيق كتلة الامتثال | | |
| التحقق من ظرف القبول | | |
| اختبار smoke لتدوير guard | | |
| تليمترى مفعلة ولوحات تعمل | | |
| تنفيذ drill للbrownout | | |
| نجاح تذاكر PoW ضمن الهدف | | |

### لقطة المقاييس

- نسبة PQ (`sorafs_orchestrator_pq_ratio`): ________
- عدد downgrade خلال اخر 24 ساعة: ________
- متوسط RTT للدوائر (p95): ________ ms
- زمن حل PoW الوسطي: ________ ms

### المرفقات

يرجى ارفاق:

1. hash حزمة دعم relay (`sha256`): __________________________
2. لقطات لوحات (نسبة PQ، نجاح الدوائر، هيستوغرام PoW).
3. حزمة drill الموقعة (`drills-signed.json` + المفتاح العام للموقّع بالهيكس والمرفقات).
4. تقرير مقاييس SNNet-10 (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`).

### توقيع المشغل

اقر ان المعلومات اعلاه دقيقة وان جميع الخطوات المطلوبة قد اكتملت.

التوقيع: _________________________  التاريخ: ___________________

</div>
