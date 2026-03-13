---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: מנוע עסקה
title: محرك الصفقات في SoraFS
sidebar_label: محرك الصفقات
description: نظرة عامة على محرك الصفقات SF-8 وتكامل Torii وسطح التليمترية.
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/deal_engine.md`. احرص على إبقاء الموقعين متوافقين ما دامت الوثائق القديمة نشطة.
:::

# محرك الصفقات في SoraFS

يعرف مسار خارطة الطريق SF-8 محرك الصفقات في SoraFS، موفراً
محاسبة حتمية لاتفاقات التخزين والاسترجاع بين
العملاء والمزوّدين. تُوصَف الاتفاقات عبر حمولات Norito
المعرّفة في `crates/sorafs_manifest/src/deal.rs`، وتشمل شروط الصفقة،
قفل السندات، المدفوعات المصغرة الاحتمالية، وسجلات التسوية.

ينشئ العامل المضمن في SoraFS (`sorafs_node::NodeHandle`) الآن
مثيلاً من `DealEngine` لكل عملية عقدة. يقوم المحرك بما يلي:

- يتحقق من الصفقات ويسجلها باستخدام `DealTermsV1`؛
- يراكم رسومًا مقومة بـ XOR عند الإبلاغ عن استخدام النسخ المتماثل؛
- يقيّم نوافذ المدفوعات المصغرة الاحتمالية باستخدام أخذ عينات حتمية
  قائمة على BLAKE3؛ ו
- ينتج لقطات ledger وحمولات تسوية مناسبة للنشر الحوكمـي.

מידע נוסף
المشغّلون من ممارسة واجهات API بثقة. تبعث التسويات الآن حمولات حوكمة `DealSettlementV1`،
وترتبط مباشرةً بخط نشر SF-12، كما تُحدّث سلسلة OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) من أجل لوحات Torii
وتطبيق SLOs. وتركّز العناصر اللاحقة على أتمتة slashing التي يبدأها المدققون
وتنسيق دلالات الإلغاء مع سياسة الحوكمة.

تغذي تليمترية الاستخدام أيضًا مجموعة المقاييس `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, وعدّادات التذاكر
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). تكشف هذه الإجماليات عن تدفق اليانصيب
الاحتمالي لتمكين المشغّلين من ربط مكاسب المدفوعات المصغرة وترحيل الرصيد
‏

## קוד Torii

تعرض Torii نقاط نهاية مخصصة كي يتمكن المزوّدون من الإبلاغ عن الاستخدام وتحريك
دورة حياة الصفقة بدون wiring مخصص:

- `POST /v2/sorafs/deal/usage` يقبل تليمترية `DealUsageReport` ويعيد
  نتائج محاسبة حتمية (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` ينهى النافذة الحالية، ويبث
  `DealSettlementRecord` الناتج إلى جانب `DealSettlementV1` مشفّرًا بـ base64
  وجاهزًا للنشر في DAG الحوكمة.
- يغذي `/v2/events/sse` في Torii الآن سجلات `SorafsGatewayEvent::DealUsage`
  التي تلخص كل إرسال استخدام (epoch، ساعات GiB المقاسة، عدّادات التذاكر،
  الرسوم الحتمية)، وسجلات `SorafsGatewayEvent::DealSettlement`
  التي تتضمن لقطة ledger المعتمدة للتسوية مع digest/الحجم/base64 من BLAKE3
  للقطعة الحوكمية على القرص، وتنبيهات `SorafsGatewayEvent::ProofHealth`
  عندما تتجاوز عتبات PDP/PoTR (المزوّد، النافذة، حالة strike/cooldown، مبلغ العقوبة).
  يمكن للمستهلكين التصفية حسب المزوّد للتفاعل مع تليمترية جديدة أو تسويات أو تنبيهات
  صحة البراهين دون polling.

يشارك كلا نقطتي النهاية في إطار حصص SoraFS عبر نافذة
`torii.sorafs.quota.deal_telemetry` الجديدة، ما يسمح للمشغّلين بضبط معدل الإرسال
المسموح لكل نشر.