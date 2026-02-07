---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mecanismo de negociação
título: محرك الصفقات em SoraFS
sidebar_label: Nome da barra lateral
description: O arquivo SF-8 é compatível com Torii e com o nome Torii.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/deal_engine.md`. Certifique-se de que o produto esteja funcionando corretamente.
:::

# محرك الصفقات em SoraFS

Você pode usar o SF-8 para obter mais informações em SoraFS, mas
محاسبة حتمية لاتفاقات التخزين e الاسترجاع بين
العملاء والمزوّدين. Solução de problemas Norito
O nome de `crates/sorafs_manifest/src/deal.rs`, e o código de segurança
قفل السندات, المدفوعات المصغرة الاحتمالية, وسجلات التسوية.

O código de barras é SoraFS (`sorafs_node::NodeHandle`)
Use `DealEngine` para remover o problema. يقوم المحرك بما يلي:

- يتحقق من الصفقات ويسجلها باستخدام `DealTermsV1`;
- يراكم رسومًا مقومة بـ XOR عند الإبلاغ عن استخدام النسخ المتماثل؛
- يقيّم نوافذ المدفوعات المصغرة الاحتمالية باستخدام أخذ عينات حتمية
  قائمة على BLAKE3; e
- ينتج لقطات razão e حمولات تسوية مناسبة للنشر الحوكمـي.

تغطي الاختبارات الوحدوية التحقق, واختيار المدفوعات المصغرة, وتدفقات التسوية ليتمكن
A API da API está disponível para download. تبعث التسويات الآن حمولات حوكمة `DealSettlementV1`,
وترتبط مباشرةً بخط نشر SF-12, كما تُحدّث سلسلة OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) por meio do Torii
Definir SLOs. وتركّز العناصر اللاحقة على أتمتة slashing التي يبدأها المدققون
وتنسيق دلالات الإلغاء مع سياسة الحوكمة.

Verifique o valor do código de barras `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, código de identificação
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). تكشف هذه الإجماليات عن تدفق اليانصيب
A melhor maneira de fazer isso é com a ajuda de uma pessoa.
بنتائج التسوية.

## Torii

A solução Torii é uma ferramenta que pode ser usada para evitar problemas de segurança.
A melhor maneira de conectar a fiação é:

- `POST /v1/sorafs/deal/usage` يقبل تليمترية `DealUsageReport` ويعيد
  Verifique se há algum problema (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle`
  `DealSettlementRecord` é um dispositivo `DealSettlementV1` baseado em base64
  وجاهزًا للنشر no DAG الحوكمة.
- O `/v1/events/sse` é o Torii do modelo `SorafsGatewayEvent::DealUsage`
  التي تلخص كل إرسال استخدام (época, ساعات GiB المقاسة, عدّادات التذاكر,
  Nome do usuário), e `SorafsGatewayEvent::DealSettlement`
  Como criar o ledger para o digest/الحجم/base64 com BLAKE3
  للقطعة الحوكمية على القرص, وتنبيهات `SorafsGatewayEvent::ProofHealth`
  Você pode usar PDP/PoTR (المزوّد, النافذة, حالة strike/cooldown, مبلغ العقوبة).
  يمكن للمستهلكين التصفية حسب المزوّد للتفاعل مع تليمترية جديدة, أو تسويات, أو تنبيهات
  صحة البراهين دون votação.

A chave de segurança do SoraFS é a mesma do SoraFS.
`torii.sorafs.quota.deal_telemetry` é um dispositivo de armazenamento de dados de alta qualidade
Não há nada.