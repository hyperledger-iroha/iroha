---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: محرك الصفقة
العنوان: Motor de acordos da SoraFS
Sidebar_label: محرك الحبال
الوصف: فيزاو جيرال دو موتور دي أكوردوس SF-8، متكامل مع Torii وأسطح القياس عن بعد.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/sorafs/deal_engine.md`. الحفاظ على المواقع الممتعة أثناء التوثيق البديل الدائم.
:::

# موتور دي أكوردوس دا SoraFS

تم تقديم مسار خريطة الطريق SF-8 لمحرك الاتفاق SoraFS، مما يعني
حتمية المراقبة من أجل اتفاقات التخزين والاسترداد بين
العملاء والبروفيسور. يتم وصف المستندات مع الحمولات الصافية Norito
محدد في `crates/sorafs_manifest/src/deal.rs`، مصطلحات كوبريندو متوافقة،
حجب السندات، والمدفوعات الصغيرة المحتملة، وسجلات التصفية.

أيها العامل الذي تم تفعيله من SoraFS (`sorafs_node::NodeHandle`) الآن على سبيل المثال
`DealEngine` لكل عقدة من المعالجات. يا محرك:

- التحقق من صحة تسجيل الدخول باستخدام `DealTermsV1`؛
- تجميع الكوبرانكاس المقدرة في XOR عند استخدام النسخ المتماثل والإبلاغ عنه؛
- Avalia Janelas de Micropagamento Probabilistico Usando amostragem Deterministica
  قاعدة على BLAKE3؛ ه
- إنتاج لقطات من دفتر الأستاذ وحمولات التصفية المناسبة للجمهور
  دي الحاكم.الخصيتين الوحدويتين صالحتان تمامًا، وسلسة من الحزم الصغيرة وتدفقات السائلة من أجل
يمكن للمشغلين ممارسة التمارين كواجهات برمجة التطبيقات (APIs) مع الثقة. ليكويداكويس أغورا إيميتم
الحمولات الصافية للإدارة `DealSettlementV1`، متصلة مباشرة بخط الأنابيب
نشر SF-12، وتحديث سلسلة OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`، `deal_expected_charge_nano`، `deal_client_debit_nano`،
`deal_outstanding_nano`، `deal_bond_slash_nano`، `deal_publish_total`) للوحات المعلومات الخاصة بـ Torii e
تطبيق SLOs. تركز العناصر التالية على بدء القطع تلقائيًا
المدققون وتنسيق دلالات الإلغاء مع سياسة الإدارة.

يتضمن قياس الاستخدام الحالي عن بعد الطعام أو مجموعة المقاييس `sorafs.node.micropayment_*`:
`micropayment_charge_nano`، `micropayment_credit_generated_nano`،
`micropayment_credit_applied_nano`، `micropayment_credit_carry_nano`،
`micropayment_outstanding_nano`، ومحاسبو التذاكر
(`micropayment_tickets_processed_total`، `micropayment_tickets_won_total`،
`micropayment_tickets_duplicate_total`). هذه كلها تعرض تدفق اليانصيب
الاحتمالية التي تجعل المشغلين يمتلكون مكافآت صغرى مرتبطة
ترحيل الائتمان مع نتائج التصفية.

## Integracao com Torii

يعرض Torii نقاط النهاية المخصصة ليبلغ المحققون عن استخدامها وتواصلها
ciclo de vida do acordo sem الأسلاك تنهد متوسط:- `POST /v1/sorafs/deal/usage` جهاز القياس عن بعد `DealUsageReport` والإرجاع
  نتائج تحديد المراقبة (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` تم الانتهاء من الإدخال التلقائي، الإرسال
  نتيجة `DealSettlementRecord` مرتبطة بـ `DealSettlementV1` في base64
  Pronto para publicacao no DAG de Goveranca.
- تغذية `/v1/events/sse` إلى Torii قبل إرسال السجلات `SorafsGatewayEvent::DealUsage`
  استئناف كل إرسالية من الاستخدام (العصر، ومتوسطات GiB-hours، وعدادات التذاكر،
  تحديد الكوبرانكا)، السجلات `SorafsGatewayEvent::DealSettlement`
  تتضمن لقطة Canonico do Ledger de Liquidacao أكثر من Digest/tamanho/base64
  BLAKE3 أداة التحكم في القرص والتنبيهات `SorafsGatewayEvent::ProofHealth`
  دائمًا ما يتم تحديد PDP/PoTR بشكل متفوق (المزود، يناير، حالة الإضراب/التهدئة،
  الشجاعة دا penalidade). يمكن للمستهلكين التصفية من أجل إثبات إعادة تجديد جديدة
  القياس عن بعد، والتصفية، أو تنبيهات التحقق من الأدلة دون الاقتراع.

تشارك جميع نقاط النهاية في إطار العمل الخاص بـ SoraFS عبر nova janela
`torii.sorafs.quota.deal_telemetry`، للسماح للمشغلين بضبط نوع البيئة
مسموح للنشر.