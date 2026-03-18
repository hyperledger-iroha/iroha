---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : moteur de transaction
titre : محرك الصفقات في SoraFS
sidebar_label : محرك الصفقات
description: نظرة عامة على محرك الصفقات SF-8 et Torii et التليمترية.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/deal_engine.md`. احرص على إبقاء الموقعين متوافقين ما دامت الوثائق القديمة نشطة.
:::

# محرك الصفقات pour SoraFS

يعرف مسار خارطة الطريق SF-8 محرك الصفقات في SoraFS, موفراً
محاسبة حتمية لاتفاقات التخزين والاسترجاع بين
العملاء والمزوّدين. تُوصَف الاتفاقات عبر حمولات Norito
المعرّفة في `crates/sorafs_manifest/src/deal.rs`, وتشمل شروط الصفقة،
قفل السندات، المدفوعات المصغرة الاحتمالية، وسجلات التسوية.

ينشئ العامل المضمن في SoraFS (`sorafs_node::NodeHandle`)
مثيلاً من `DealEngine` لكل عملية عقدة. يقوم المحرك بما يلي:

- يتحقق من الصفقات ويسجلها باستخدام `DealTermsV1`؛
- يراكم رسومًا مقومة بـ XOR عند الإبلاغ عن استخدام النسخ المتماثل؛
- يقيّم نوافذ المدفوعات المصغرة الاحتمالية باستخدام أخذ عينات حتمية
  قائمة على BLAKE3؛ et
- ينتج لقطات ledger وحمولات تسوية مناسبة للنشر الحوكمـي.تغطي الاختبارات الوحدوية التحقق، واختيار المدفوعات المصغرة، وتدفقات التسوية ليتمكن
Il s'agit d'une API disponible. تبعث التسويات الآن حمولات حوكمة `DealSettlementV1`,
Il s'agit d'une connexion pour le SF-12, qui est également compatible avec OpenTelemetry `sorafs.node.deal_*`.
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) et Torii
وتطبيق SLO. وتركّز العناصر اللاحقة على أتمتة slashing التي يبدأها المدققون
وتنسيق دلالات الإلغاء مع سياسة الحوكمة.

تغذي تليمترية الاستخدام أيضًا مجموعة المقاييس `sorafs.node.micropayment_*` :
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, وعدّادات التذاكر
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). تكشف هذه الإجماليات عن تدفق اليانصيب
الاحتمالي لتمكين المشغّلين من ربط مكاسب المدفوعات المصغرة وترحيل الرصيد
بنتائج التسوية.

## تكامل Torii

تعرض Torii نقاط نهاية مخصصة كي يتمكن المزوّدون من الإبلاغ عن الاستخدام وتحريك
دورة حياة الصفقة بدون câblage مخصص:- `POST /v1/sorafs/deal/usage` يقبل تليمترية `DealUsageReport` ويعيد
  نتائج محاسبة حتمية (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` ينهى النافذة الحالية، ويبث
  `DealSettlementRecord` Version `DealSettlementV1` en base64
  وجاهزًا للنشر في DAG الحوكمة.
- `/v1/events/sse` à Torii pour `SorafsGatewayEvent::DealUsage`
  التي تلخص كل إرسال استخدام (epoch، ساعات GiB المقاسة، عدّادات التذاكر،
  الرسوم الحتمية) et `SorafsGatewayEvent::DealSettlement`
  Le grand livre est également disponible dans digest/الحجم/base64 par BLAKE3
  للقطعة الحوكمية على القرص، وتنبيهات `SorafsGatewayEvent::ProofHealth`
  Il s'agit d'un PDP/PoTR (pour Strike/Cooldown, ou plus).
  يمكن للمستهلكين التصفية حسب المزوّد للتفاعل مع تليمترية جديدة أو تسويات أو تنبيهات
  صحة البراهين دون sondage.

يشارك كلا نقطتي النهاية في إطار حصص SoraFS عبر نافذة
`torii.sorafs.quota.deal_telemetry` Dispositif pour l'alimentation en eau potable
المسموح لكل نشر.