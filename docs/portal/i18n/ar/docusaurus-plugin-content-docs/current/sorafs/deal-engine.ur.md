---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: محرك الصفقة
العنوان: محرك الصفقات SoraFS
Sidebar_label: محرك الصفقات
الوصف: محرك صفقة SF-8، تكامل Torii وأسطح القياس عن بعد کا جائزہ۔
---

:::ملاحظة مستند ماخذ
:::

# محرك الصفقة SoraFS

مسار خارطة الطريق SF-8 SoraFS Deal Engine متعارف کراتا ہے، جو
العملاء ومقدمو الخدمات هم اتفاقيات تخزين واسترجاع الأدوية
المحاسبة الحتمية اتفاقيات الحمولات Norito
سعر هذا المنتج `crates/sorafs_manifest/src/deal.rs` مرتفع جدًا، و
شروط الصفقة، وتأمين السندات، والمدفوعات الصغيرة المحتملة، وسجلات التسوية.

عامل SoraFS المضمن (`sorafs_node::NodeHandle`) عملية العقدة کے لیے
ایک `DealEngine` مثيل لنا ہے۔ ها المحرك:

- `DealTermsV1` صفقات التحقق من صحة وتسجيل كرتا ہے؛
- تقرير استخدام النسخ المتماثل ہونے پر XOR تتراكم الرسوم المقومة کرتا ہے؛
- أخذ العينات الحتمية المستندة إلى BLAKE3 کے ذریعے نوافذ الدفع الجزئي الاحتمالية تقييم کرتا ہے؛ و
- لقطات دفتر الأستاذ ونشر الحوكمة لحمولات التسوية بناتا ہے۔التحقق من صحة اختبارات الوحدة واختيار الدفعات الصغيرة وتدفقات التسوية التي تغطي كل شيء
يعتمد المشغلون على تمرين واجهات برمجة التطبيقات الثابتة. المستوطنات اب `DealSettlementV1` الحكم
تنبعث الحمولات الصافية من خط أنابيب النشر SF-12 الذي يمتد إلى سلك راسخ، و
القياس عن بعد المفتوح لسلسلة `sorafs.node.deal_*`
(`deal_settlements_total`، `deal_expected_charge_nano`، `deal_client_debit_nano`،
`deal_outstanding_nano`، `deal_bond_slash_nano`، `deal_publish_total`) وTorii لوحات المعلومات و
تم تحديث تطبيق SLO. عناصر المتابعة التي بدأها المدقق في أتمتة القطع و
دلالات الإلغاء وسياسة الحوكمة تنسق في المركز.

تم تعيين قياس الاستخدام عن بعد باستخدام `sorafs.node.micropayment_*` من خلال خلاصة التغذية:
`micropayment_charge_nano`، `micropayment_credit_generated_nano`،
`micropayment_credit_applied_nano`، `micropayment_credit_carry_nano`،
`micropayment_outstanding_nano`، وعدادات التذاكر
(`micropayment_tickets_processed_total`، `micropayment_tickets_won_total`،
`micropayment_tickets_duplicate_total`). أحد إجماليات تدفق اليانصيب الاحتمالي يوضح لك الأمر
يفوز المشغلون بالدفعات الصغيرة ويرتبط ترحيل الائتمان ونتائج التسوية بالسداد.

## تكامل Torii

تعرض نقاط النهاية المخصصة Torii المعلومات وتقرير استخدام موفري الخدمة
بدون أسلاك مخصصة، محرك دورة حياة الصفقة:- `POST /v2/sorafs/deal/usage` `DealUsageReport` القياس عن بعد قبول کرتا ہے و
  نتائج المحاسبة الحتمية (`UsageOutcome`) تعود إلى کرتا ہے۔
- `POST /v2/sorafs/deal/settle` إنهاء النافذة الحالية کرتا ہے، و
  تم تنزيل `DealSettlementRecord` وتشفير base64 `DealSettlementV1` وبث البث المباشر
  تم نشر منشور DAG لحوكمة جو.
- Torii کا `/v2/events/sse` تغذية اب `SorafsGatewayEvent::DealUsage` يسجل بث کرتا ہے
  ملخص تقديم الاستخدام (العصر، ساعات GiB المقيسة، عدادات التذاكر،
  الرسوم الحتمية)، يسجل `SorafsGatewayEvent::DealSettlement` لقطة دفتر الأستاذ للتسوية الأساسية
  قطعة أثرية للحوكمة على القرص BLAKE3 Digest/size/base64 تتضمن القرص، و
  تنبيهات `SorafsGatewayEvent::ProofHealth` عندما تتجاوز حدود PDP/PoTR 5 (المزود، النافذة، حالة الإضراب/التباطؤ، مبلغ العقوبة).
  موفر خدمة المستهلكين عامل تصفية للقياس الجديد عن بعد أو المستوطنات أو تنبيهات إثبات الصحة
  الاقتراع هو رد فعل قوي.

دون نقاط النهاية SoraFS إطار عمل الحصص `torii.sorafs.quota.deal_telemetry` نافذة جديدة
يحتوي مشغلو GS على النشر الشامل لضبط معدل الإرسال المسموح به.