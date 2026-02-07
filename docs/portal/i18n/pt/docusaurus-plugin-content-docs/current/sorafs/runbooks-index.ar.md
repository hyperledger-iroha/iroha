---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice de runbooks
título: فهرس أدلة التشغيل للمشغلين
sidebar_label: فهرس أدلة التشغيل
description: نقطة الدخول المعتمدة لأدلة تشغيل مشغلي SoraFS المُرحَّلة.
---

> يعكس سجلّ المالكين الموجود ضمن `docs/source/sorafs/runbooks/`.
> Verifique se o SoraFS está funcionando corretamente.

Faça o download do seu cartão de crédito para que você possa fazer o seu trabalho com segurança. البوابة.
يسرد كل إدخال الملكية ومسار المصدر المعتمد والنسخة في البوابة كي يتمكن المراجعون من
Você pode usar o produto para obter mais informações.

## مضيف المعاينة التجريبية

A solução DocOps para o DocOps é a solução para o problema do computador.
`https://docs.iroha.tech/`. عند توجيه المشغلين أو المراجعين إلى دليل تشغيل مُرحَّل,
A soma de verificação é definida como uma soma de verificação. إجراءات
النشر/الرجوع موجودة في
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| دليل التشغيل | المالك(ون) | نسخة البوابة | المصدر |
|------------|-----------|-------------|--------|
| Gateway Gateway e DNS | TL de rede, automação de operações, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Conjunto de ferramentas SoraFS | Documentos/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| تسوية السعة | Tesouraria / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Máquinas de lavar louça | GT Ferramentaria | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| قائمة تحقق عمليات العقد | Equipe de armazenamento, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Máquinas de lavar roupa e produtos | Conselho de Governança | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Encenação | Documentos/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| قابلية الملاحظة لمرساة Taikai | GT Plataforma de Mídia / Programa DA / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## قائمة التحقق

- [x] بناء البوابة يربط بهذا الفهرس (عنصر الشريط الجانبي).
- [x] كل دليل تشغيل مُرحَّل يذكر مسار المصدر المعتمد لإبقاء المراجعين على اتساق أثناء مراجعات
  Não.
- [x] خط أنابيب معاينة DocOps يحظر الدمج عندما يكون دليل تشغيل مُدرج مفقودًا من مخرجات البوابة.

Não há nada que você possa fazer sobre o dinheiro (o que significa que você pode usar o dinheiro ou o dinheiro)
Você pode usar o aplicativo DocOps para fazer isso no site do DocOps
`docs/examples/docs_preview_request_template.md`.