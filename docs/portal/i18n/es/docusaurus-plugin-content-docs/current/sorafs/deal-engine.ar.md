---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: motor de acuerdos
título: محرك الصفقات في SoraFS
sidebar_label: محرك الصفقات
descripción: نظرة عامة على محرك الصفقات SF-8 وتكامل Torii وسطح التليمترية.
---

:::nota المصدر المعتمد
Utilice el código `docs/source/sorafs/deal_engine.md`. احرص على إبقاء الموقعين متوافقين ما دامت الوثائق القديمة نشطة.
:::

# محرك الصفقات في SoraFS

يعرف مسار خارطة الطريق SF-8 محرك الصفقات في SoraFS, موفراً
محاسبة حتمية لاتفاقات التخزين والاسترجاع بين
العملاء والمزوّدين. تُوصَف الاتفاقات عبر حمولات Norito
المعرّفة في `crates/sorafs_manifest/src/deal.rs`, y تشمل شروط الصفقة,
قفل السندات، المدفوعات المصغرة الاحتمالية, وسجلات التسوية.

Nombre del producto: SoraFS (`sorafs_node::NodeHandle`)
Utilice el `DealEngine` para conectarlo. يقوم المحرك بما يلي:

- يتحقق من الصفقات ويسجلها باستخدام `DealTermsV1`؛
- يراكم رسومًا مقومة بـ XOR عند الإبلاغ عن استخدام النسخ المتماثل؛
- يقيّم نوافذ المدفوعات المصغرة الاحتمالية باستخدام أخذ عينات حتمية
  قائمة على BLAKE3؛ y
- ينتج لقطات libro mayor وحمولات تسوية مناسبة للنشر الحوكمـي.تغطي الاختبارات الوحدوية التحقق، واختيار المدفوعات المصغرة، وتدفقات التسوية ليتمكن
المشغّلون ممارسة y API بثقة. تبعث التسويات الآن حمولات حوكمة `DealSettlementV1`،
Instalación de software SF-12, instalación de OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) desde el teclado Torii
Otros SLO. وتركّز العناصر اللاحقة على أتمتة recortando التي يبدأها المدققون
وتنسيق دلالات الإلغاء مع سياسة الحوكمة.

Nombre del producto: `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, وعدّادات التذاكر
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). تكشف هذه الإجماليات عن تدفق اليانصيب
الاحتمالي لتمكين المشغّلين من ربط مكاسب المدفوعات المصغرة وترحيل الرصيد
بنتائج التسوية.

## Mensaje Torii

تعرض Torii نقاط نهاية مخصصة كي يتمكن المزوّدون من الإبلاغ عن الاستخدام وتحريك
دورة حياة الصفقة بدون cableado:- `POST /v1/sorafs/deal/usage` يقبل تليمترية `DealUsageReport` ويعيد
  نتائج محاسبة حتمية (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` ينهى النافذة الحالية، ويبث
  `DealSettlementRecord` Adaptador de base `DealSettlementV1` basado en base64
  وجاهزًا للنشر في DAG الحوكمة.
- يغذي `/v1/events/sse` في Torii آن سجلات `SorafsGatewayEvent::DealUsage`
  التي تلخص كل إرسال استخدام (época, ساعات GiB المقاسة, عدّادات التذاكر،
  الرسوم الحتمية) ، سجلات `SorafsGatewayEvent::DealSettlement`
  El libro mayor se basa en digest/الحجم/base64 de BLAKE3
  للقطعة الحوكمية على القرص، وتنبيهات `SorafsGatewayEvent::ProofHealth`
  عندما تتجاوز عتبات PDP/PoTR (المزوّد، النافذة، حالة strike/cooldown, مبلغ العقوبة).
  يمكن للمستهلكين التصفية حسب المزوّد للتفاعل مع تليمترية جديدة أو تسويات أو تنبيهات
  صحة البراهين دون encuestas.

يشارك كلا نقطتي النهاية في إطار حصص SoraFS عبر نافذة
`torii.sorafs.quota.deal_telemetry` الجديدة، ما يسمح للمشغّلين بضبط معدل الإرسال
المسموح لكل نشر.