---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: محرك الصفقة
العنوان: محرك acuerdos de SoraFS
Sidebar_label: محرك الأقراص
الوصف: استئناف محرك الأقراص SF-8، التكامل مع Torii وأسطح القياس عن بعد.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/deal_engine.md`. حافظ على مواقعك المتواجدة أثناء تنشيط المستندات المخزنة.
:::

# موتور دي أكويردوس دي SoraFS

يقدم خط المسار SF-8 محرك الأقراص SoraFS الذي يفتح
ضوابط التحديد لحالات التخزين والاسترداد بين
العملاء والموردون. تم وصف الروابط مع الحمولات الصافية Norito
تم تعريفها في `crates/sorafs_manifest/src/deal.rs`، والتي تستخدم محطات المعالجة،
حظر المكافآت والمدفوعات الصغيرة المحتملة وسجلات التصفية.

العامل المضمن في SoraFS (`sorafs_node::NodeHandle`) الآن على الفور
`DealEngine` لكل عملية عقدة. المحرك:

- التحقق من صحة وتسجيل Acuerdos Usando `DealTermsV1`؛
- تجميع البضائع المقدرة في XOR عند الإبلاغ عن استخدام النسخ المتماثل؛
- تقييم نوافذ الميكروباغو الاحتمالية باستخدام محدد محدد
  مبني على BLAKE3؛ ذ
- إنتاج لقطات من دفتر الأستاذ وحمولات التصفية المناسبة للنشر
  دي غوبرنانزا.تشمل الاختبارات الوحدوية التحقق من صحة واختيار الميكروباجات وتدفقات المياه
تصفية حتى يتمكن المشغلون من تشغيل واجهات برمجة التطبيقات بثقة.
تُصدر عمليات التصفية الآن حمولات حكومية `DealSettlementV1`,
الاتصال مباشرة بخط أنابيب النشر SF-12، وتحديث السلسلة
فتح القياس عن بعد `sorafs.node.deal_*`
(`deal_settlements_total`، `deal_expected_charge_nano`، `deal_client_debit_nano`،
`deal_outstanding_nano`، `deal_bond_slash_nano`، `deal_publish_total`) للوحات المعلومات Torii y
تطبيق SLOs. يتم التركيز على الخطوات التالية في أتمتة القطع
تم البدء بها من قبل المدققين وتنسيق معنى الإلغاء مع سياسة الحكومة.

قياس الاستخدام عن بعد أيضًا لمجموعة المقاييس `sorafs.node.micropayment_*`:
`micropayment_charge_nano`، `micropayment_credit_generated_nano`،
`micropayment_credit_applied_nano`، `micropayment_credit_carry_nano`،
`micropayment_outstanding_nano`، ومحاسبو التذاكر
(`micropayment_tickets_processed_total`، `micropayment_tickets_won_total`،
`micropayment_tickets_duplicate_total`). توضح هذه الإجماليات تدفق اليانصيب
احتمال أن يتمكن المشغلون من ربط انتصارات الميكروباجات و
ترحيل الائتمان مع نتائج التصفية.

## التكامل مع Torii

يعرض Torii نقاط النهاية المخصصة لكي يقوم الموردون بالإبلاغ عن الاستخدام وتوصيل الدائرة
de vida del acuerdo sin الأسلاك الشخصية:- `POST /v1/sorafs/deal/usage` يقبل القياس عن بعد `DealUsageReport` ويعود
  نتائج تحديد البيانات (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` الانتهاء من النافذة الفعلية، وإرسالها
  نتيجة `DealSettlementRecord` مع `DealSettlementV1` في base64
  قائمة للنشر في DAG de gobernanza.
- El Feed `/v1/events/sse` de Torii الآن سجلات الإرسال `SorafsGatewayEvent::DealUsage`
  يمكنك استئناف كل إرسال من الاستخدام (العصر، وسائط GiB-hora، محاسبي التذاكر،
  تحديد الشحنات)، السجلات `SorafsGatewayEvent::DealSettlement`
  يتضمن اللقطة الأساسية لدفتر الأستاذ للتصفية الأكثر ملخصًا/الحجم/base64
  BLAKE3 قطعة أثرية من إدارة الديسكو والتنبيهات `SorafsGatewayEvent::ProofHealth`
  كل مرة يتم فيها تجاوز مظلات PDP/PoTR (المزود، النافذة، حالة الضربة/التهدئة،
  مونتو دي العقوبات). يمكن للمستهلكين ترشيح المنتج من أجل الحصول على أ
  جديد القياس عن بعد، تصفية أو تنبيهات الصحة الاختبار دون إجراء الاقتراع.

جميع نقاط النهاية المشاركة في إطار عمل SoraFS عبر النافذة الجديدة
`torii.sorafs.quota.deal_telemetry`، السماح للمشغلين بضبط حجم الإرسال
مسموح به من أجل التصميم.