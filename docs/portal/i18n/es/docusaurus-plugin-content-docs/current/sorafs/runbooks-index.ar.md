---
lang: es
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
descripción: نقطة الدخول المعتمدة لأدلة تشغيل مشغلي SoraFS المُرحَّلة.
---

> يعكس سجلّ المالكين الموجود ضمن `docs/source/sorafs/runbooks/`.
> يجب ربط أي دليل تشغيل جديد لـ SoraFS هنا بمجرد نشره في بناء البوابة.

Asegúrese de que el aparato esté limpio y esté limpio.
يسرد كل إدخال الملكية ومسار المصدر المعتمد والنسخة في البوابة كي يتمكن المراجعون من
الانتقال مباشرةً إلى الدليل المطلوب خلال المعاينة التجريبية.

## مضيف المعاينة التجريبية

لقد قامت موجة DocOps بترقية مضيف المعاينة التجريبية المعتمد من المراجعين إلل
`https://docs.iroha.tech/`. عند توجيه المشغلين أو المراجعين إلى دليل تشغيل مُرحَّل،
أشِر إلى هذا الاسم المضيف كي يستخدموا لقطة البوابة المحمية بالـ suma de comprobación. إجراءات
النشر/الرجوع موجودة في
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).| دليل التشغيل | المالك(ون) | نسخة البوابة | المصدر |
|-------------|-----------|-------------|--------|
| Puerta de enlace y DNS | TL de redes, automatización de operaciones, documentos/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| دليل تشغيل عمليات SoraFS | Documentos/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| تسوية السعة | Tesorería/SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| عمليات سجل التثبيت | Grupo de Trabajo sobre Herramientas | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| قائمة تحقق عمليات العقد | Equipo de Almacenamiento, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| دليل تشغيل النزاعات Y الإلغاءات | Consejo de Gobierno | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| دليل تشغيل مانيفستات الـ puesta en escena | Documentos/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| قابلية الملاحظة لمرساة Taikai | Media Platform WG / Programa DA / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## قائمة التحقق

- [x] بناء البوابة يربط بهذا الفهرس (عنصر الشريط الجانبي).
- [x] كل دليل تشغيل مُرحَّل يذكر مسار المصدر المعتمد لإبقاء المراجعين على اتساق أثناء مراجعات
  التوثيق.
- [x] خط أنابيب معاينة DocOps يحظر الدمج عندما يكون دليل تشغيل مُدرج مفقودًا من مخرجات البوابة.

يجب أن تضيف عمليات الترحيل المستقبلية (مثل تمارين الفوضى الجديدة أو ملاحق الحوكمة) صفًا
إلى الجدول أعلاه وتحدّث قائمة التحقق الخاصة بـ DocOps المضمنة في
`docs/examples/docs_preview_request_template.md`.