---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: runbooks-index
title: فهرس أدلة التشغيل للمشغلين
sidebar_label: فهرس أدلة التشغيل
description: نقطة الدخول المعتمدة لأدلة تشغيل مشغلي SoraFS المُرحَّلة.
---

> يعكس سجلّ المالكين الموجود ضمن `docs/source/sorafs/runbooks/`.
> يجب ربط أي دليل تشغيل جديد لـ SoraFS هنا بمجرد نشره في بناء البوابة.

استخدم هذه الصفحة للتحقق من أي أدلة تشغيل أكملت الانتقال من شجرة الوثائق القديمة إلى البوابة.
يسرد كل إدخال الملكية ومسار المصدر المعتمد والنسخة في البوابة كي يتمكن المراجعون من
الانتقال مباشرةً إلى الدليل المطلوب خلال المعاينة التجريبية.

## مضيف المعاينة التجريبية

لقد قامت موجة DocOps بترقية مضيف المعاينة التجريبية المعتمد من المراجعين إلى
`https://docs.iroha.tech/`. عند توجيه المشغلين أو المراجعين إلى دليل تشغيل مُرحَّل،
أشِر إلى هذا الاسم المضيف كي يستخدموا لقطة البوابة المحمية بالـ checksum. إجراءات
النشر/الرجوع موجودة في
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| دليل التشغيل | المالك(ون) | نسخة البوابة | المصدر |
|-------------|-----------|-------------|--------|
| انطلاقة Gateway وDNS | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| دليل تشغيل عمليات SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| تسوية السعة | Treasury / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| عمليات سجل التثبيت | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| قائمة تحقق عمليات العقد | Storage Team, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| دليل تشغيل النزاعات والإلغاءات | Governance Council | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| دليل تشغيل مانيفستات الـ staging | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| قابلية الملاحظة لمرساة Taikai | Media Platform WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## قائمة التحقق

- [x] بناء البوابة يربط بهذا الفهرس (عنصر الشريط الجانبي).
- [x] كل دليل تشغيل مُرحَّل يذكر مسار المصدر المعتمد لإبقاء المراجعين على اتساق أثناء مراجعات
  التوثيق.
- [x] خط أنابيب معاينة DocOps يحظر الدمج عندما يكون دليل تشغيل مُدرج مفقودًا من مخرجات البوابة.

يجب أن تضيف عمليات الترحيل المستقبلية (مثل تمارين الفوضى الجديدة أو ملاحق الحوكمة) صفًا
إلى الجدول أعلاه وتحدّث قائمة التحقق الخاصة بـ DocOps المضمنة في
`docs/examples/docs_preview_request_template.md`.
