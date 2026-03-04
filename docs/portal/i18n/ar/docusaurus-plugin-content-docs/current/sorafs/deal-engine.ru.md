---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: محرك الصفقة
العنوان: Двидок сделок SoraFS
Sidebar_label: Двиjoк сделок
الوصف: نظرة عامة على SF-8 والتكامل Torii والقياس عن بعد.
---

:::note Канонический источник
:::

# تصميم المنتج SoraFS

تتدفق خريطة الطريق SF-8 إلى SoraFS، بشكل خاص
تحديد القرار بشأن الاستراحة والخروج منها
العملاء والتجارب. توصف الحمولات Norito،
المقترحات في `crates/sorafs_manifest/src/deal.rs`, فتح خدمات البيع,
سندات الحجب، الصفائح الدموية الدقيقة الحقيقية والمختومة.

العامل SoraFS الأساسي (`sorafs_node::NodeHandle`)
تم استخدام المثال `DealEngine` لكل عملية. صوت:

- التحقق من صحة الصفقات وتسجيلها عبر `DealTermsV1`؛
- تحصيل الرسوم في XOR عند استخدام النسخ المتماثلة؛
- تحديد الصفائح الدقيقة الحقيقية للغاية مع القدرة على تحديد الهدف
  أخذ العينات على أساس BLAKE3؛ و
- نموذج دفتر الأستاذ واللقطات والحمولات التي لتسهيل النشر في الإدارة.تقوم الاختبارات الأولية بالتحقق من صحة اختيار الصفائح الدقيقة والخيارات المتنوعة لذلك
يمكن للمشغلين التحقق بشدة من واجهة برمجة التطبيقات (API). قم بإعادة شحن حمولات الإدارة
`DealSettlementV1`، في المقام الأول، قم بالتوصيل إلى منشورات خط الأنابيب SF-12، وقم بإعادة تشغيل السلسلة
فتح القياس عن بعد `sorafs.node.deal_*`
(`deal_settlements_total`، `deal_expected_charge_nano`، `deal_client_debit_nano`،
`deal_outstanding_nano`، `deal_bond_slash_nano`، `deal_publish_total`) للوحة التحكم Torii و
إنفاذ SLO. تركز الخطوات التالية على أتمتة القطع والبدء
المدققون والدلالات المرتبطة بالحكم السياسي.

استخدام القياس عن بعد وكذلك نوع المقياس `sorafs.node.micropayment_*`:
`micropayment_charge_nano`، `micropayment_credit_generated_nano`،
`micropayment_credit_applied_nano`، `micropayment_credit_carry_nano`،
`micropayment_outstanding_nano`، وكذلك ملابس السباحة
(`micropayment_tickets_processed_total`، `micropayment_tickets_won_total`،
`micropayment_tickets_duplicate_total`). هذه المجاميع صحيحة
نقاط اليانصيب التي يمكن لمشغليها أن يتصلوا بالصفائح الدقيقة الدقيقة و
قم بتغيير الائتمان مع النتائج.

## التكامل Torii

يوفر Torii نقاط نهاية رائعة حتى يتمكن مقدمو الخدمة من التحكم في الاستخدام والإصدارات
عقد دورة حياة بدون أسلاك خاصة:- `POST /v1/sorafs/deal/usage` قم بتشغيل جهاز القياس عن بعد `DealUsageReport` وقم بالاتصال به
  تحديد نتائج البحث (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` إغلاق النافذة، شريط
  `DealSettlementRecord` الأصلي موجود مع ترميز base64 `DealSettlementV1`,
  готовым к пликации в Governance DAG.
- لينتا Torii `/v1/events/sse` تيرنر ترانسليبريت `SorafsGatewayEvent::DealUsage`,
  الاستخدام المختصر للوقت (العصر، ساعات GiB الصغيرة، بطاقات الذاكرة،
  تحديد الرسوم)، اكتب `SorafsGatewayEvent::DealSettlement`،
  بما في ذلك دفتر الأستاذ القياسي لقطة زائد BLAKE3 دايجست/حجم/base64
  عناصر الإدارة على القرص والتنبيهات `SorafsGatewayEvent::ProofHealth` عند الإرجاع
  نتيجة PDP/PoTR (المقدم، حسنًا، حالة الإضراب/التباطؤ، كل شيء). يمكن للعملاء
  قم بالتصفية من أجل إعادة الاتصال بأجهزة قياس عن بعد جديدة أو إرسال تنبيهات أو تنبيهات إثبات الصحة دون الاقتراع.

تنضم نقاط النهاية هذه إلى إطار الحصص SoraFS من خلال النقر الجديد
`torii.sorafs.quota.deal_telemetry`، يسمح للمشغل بالوصول إلى مستوى الخدمة
يتم إجراء جزء من عملية النشر.