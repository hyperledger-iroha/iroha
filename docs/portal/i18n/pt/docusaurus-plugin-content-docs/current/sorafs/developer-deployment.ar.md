---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implantação do desenvolvedor
título: Nome do arquivo SoraFS
sidebar_label: Nome da barra lateral
description: Você pode usar o SoraFS no CI para obter mais detalhes.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/developer/deployment.md`. Certifique-se de que o produto esteja mais limpo do que o normal.
:::

#ملاحظات النشر

A solução SoraFS é uma ferramenta que pode ser usada no CI para obter mais informações. Obrigado. Verifique se o seu produto está funcionando corretamente e sem problemas.

## قبل التشغيل

- **مواءمة السجل** — أكد أن ملفات chunker والمانيفستات تشير إلى نفس ثلاثية `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **سياسة القبول** — راجع إعلانات المزوّدين الموقّعة وأدلة alias المطلوبة لـ `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **دليل تشغيل سجل التثبيت** — احتفظ بـ `docs/source/sorafs/runbooks/pin_registry_ops.md` للسيناريوهات الاستردادية (alias, تدوير, إخفاقات النسخ المتماثل).

## إعدادات البيئة

- يجب على البوابات تفعيل نقطة نهاية بث الأدلة (`POST /v1/sorafs/proof/stream`) حتى يتمكن CLI من إصدار ملخصات التليمترية.
- Altere o valor do `sorafs_alias_cache` para o `iroha_config` ou use o CLI. (`sorafs_cli manifest submit --alias-*`).
- وفّر رموز البث (أو بيانات اعتماد Torii) عبر مدير أسرار آمن.
- فعّل مصدّرات التليمترية (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) e أرسلها إلى حزمة Prometheus/OTel الخاصة بك.

## استراتيجية الإطلاق

1. **مانيفستات azul/verde**
   - Use `manifest submit --summary-out` para remover o problema.
   - A chave `torii_sorafs_gateway_refusals_total` pode ser usada para remover o excesso de água.
2. **التحقق من الأدلة**
   - اعتبر إخفاقات `sorafs_cli proof stream` عوائق للنشر؛ Verifique se o produto está funcionando corretamente e se você não tiver tempo suficiente.
   - يجب أن يكون `proof verify` جزءًا من اختبار smoke بعد التثبيت لضمان أن CAR المستضاف لدى المزوّدين لا Não é um resumo do arquivo.
3. **لوحات التليمترية**
   - Use `docs/examples/sorafs_proof_streaming_dashboard.json` ou Grafana.
   - أضف لوحات لصحة سجل التثبيت (`docs/source/sorafs/runbooks/pin_registry_ops.md`) e intervalo de pedaços.
4. **تمكين متعدد المصادر**
   - Verifique o valor do arquivo em `docs/source/sorafs/runbooks/multi_source_rollout.md` para obter mais informações sobre o produto e a solução de problemas placar/التليمترية لأغراض التدقيق.

## التعامل مع الحوادث

- Verifique o valor do arquivo em `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` é um token de fluxo e um token de fluxo.
  - `dispute_revocation_runbook.md` é uma opção que não funciona.
  - `sorafs_node_ops.md` não funciona.
  - `multi_source_rollout.md` é um dispositivo de segurança que pode ser instalado em qualquer lugar do mundo.
- سجّل إخفاقات الأدلة وشذوذات الكمون في GovernanceLog عبر واجهات PoR tracker الحالية حتى تتمكن الحوكمة من تقييم أداء المزوّدين.

## الخطوات التالية

- ادمج أتمتة المُنسِّق (`sorafs_car::multi_fetch`) عند توفر مُنسِّق الجلب متعدد المصادر (SF-6b).
- تتبع ترقيات PDP/PoTR ضمن SF-13/SF-14; Clique em CLI para configurar o software de gerenciamento de arquivos.

من خلال الجمع بين ملاحظات النشر هذه وبين دليل البدء السريع ووصفات CI, يمكن للفرق الانتقال من التجارب A chave SoraFS pode ser usada para remover o excesso de água.