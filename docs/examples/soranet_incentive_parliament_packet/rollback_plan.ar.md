---
lang: ar
direction: rtl
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1add138352dabf2433c36c9abac24085af8c7e53dca8bc579d73b37680e470cf
source_last_modified: "2025-11-22T04:59:33.125102+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/soranet_incentive_parliament_packet/rollback_plan.md -->

# خطة rollback لحوافز Relay

استخدم هذا الدليل لتعطيل المدفوعات التلقائية للrelay اذا طلبت الحوكمة ايقافا او اذا اطلقت حدود التليمترى تنبيهات.

1. **تجميد الاتمتة.** اوقف daemon الحوافز على كل مضيف orchestrator
   (`systemctl stop soranet-incentives.service` او نشر الحاويات المكافئ) وتاكد من توقف العملية.
2. **تصريف التعليمات المعلقة.** شغّل
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   لضمان عدم وجود تعليمات دفع عالقة. ارشف payloads Norito الناتجة للتدقيق.
3. **الغاء موافقة الحوكمة.** عدّل `reward_config.json`، واضبط
   `"budget_approval_id": null`، ثم اعد نشر الاعدادات عبر
   `iroha app sorafs incentives service init` (او `update-config` اذا كان daemon يعمل بشكل دائم). محرك الدفع الآن يفشل مغلقا مع
   `MissingBudgetApprovalId`، لذلك يرفض daemon سك المدفوعات حتى تتم استعادة هاش موافقة جديد. سجل git commit و SHA-256
   للاعدادات المعدلة في سجل الحادث.
4. **اخطار برلمان Sora.** ارفق سجل المدفوعات المصرف، وتقرير shadow-run، وملخص حادث قصير. يجب ان تشير محاضر البرلمان
   الى هاش الاعدادات الملغاة ووقت ايقاف daemon.
5. **التحقق من rollback.** ابق daemon معطلا حتى:
   - تكون تنبيهات التليمترى (`soranet_incentives_rules.yml`) خضراء لمدة >=24 ساعة،
   - يبين تقرير تسوية الخزانة صفر تحويلات مفقودة، و
   - يوافق البرلمان على هاش ميزانية جديد.

عند اعادة اصدار هاش موافقة ميزانية جديد من الحوكمة، حدث `reward_config.json`
بالdigest الجديد، اعد تشغيل امر `shadow-run` على اخر تليمترى، واعِد تشغيل daemon الحوافز.

</div>
