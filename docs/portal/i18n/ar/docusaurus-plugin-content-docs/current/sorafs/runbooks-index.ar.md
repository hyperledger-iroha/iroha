---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: فهرس كتب التشغيل
العنوان: فهرس عامل التشغيل للمشغلين
Sidebar_label: فهرس دليل الفساد
الوصف: نقطة الدخول المعتمدة لأدلة مشغلي SoraFS المُرحَّلة.
---

> يعكس سجلّ المالكين الموجود ضمن `docs/source/sorafs/runbooks/`.
> يجب ربط أي دليل تشغيل جديد لـ SoraFS بمجرد نشره في بناء البوابة.

استخدم هذه الصفحة التي تعمل من أي شيء وأكملت الانتقال من شجرة الوثائق القديمة إلى البوابة.
يسرد كل الملفات الملكية ومسار المصدر المعتمد والنسخة في البوابة كيشاء المراجعون منها
انتقل إلى الدليل المطلوب خلال التجربة التجريبية.

## المضيف التجربة التجريبية

لقد قامت موجة DocOps بترقية المقرر التجريبي المعتمد من المراجعين إلى
`https://docs.iroha.tech/`. عند توجيه التشغيل أو المراجعين إلى دليل مُرحَّل،
أشِر إلى هذا الاسم المضيف كي يستخدموا لقطة البوابة المحمية بالـ المجموع الاختباري. إجراءات
النشر/الرجوع موجود في
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).| دليل التشغيل | المالك(ون) | نسخة البوابة | المصدر |
|-------------|------------------|-------|-------|
| بوابة القفزة وDNS | الشبكات TL، أتمتة العمليات، Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| دليل تشغيل العمليات SoraFS | مستندات/ديفريل | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| اتفاقية السعة | الخزانة / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| عمليات تسجيل التثبيت | الأدوات مجموعة العمل | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| قم بإنجاز عمليات العقود | فريق التخزين، SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| دليل تشغيل والإلغاء | مجلس الحكم | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| دليل تشغيل مانيفيستات الـstaging | مستندات/ديفريل | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| فرصة ذهبية لمرساة Taikai | منصة الوسائط WG / برنامج DA / الشبكات TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## قائمة التحقق

- [x] بناء الرابطة بهذا الفهرس (عنصر الشريط الجانبي).
- [x] كل تشغيل الدليل مُرحَّل يتولى الإشراف على المصدر المعتمد لإبقاء المراجعين على اتساق أثناء المراجعة
  التوثيق.
- [x] خط أنابيب معاينة DocOps يحظر المدمج عندما يكون دليل تشغيل مُدرج مفقودًا من مخرجات البوابة.

يجب أن يتم تحميل عمليات الرحيل المستقبلية (مثل اضطرابات الفوضى أو المجالات الجديدة للاشتراك) صفًا
إلى الأعلى وما هو مطلوب من قائمة التحقق الخاصة بـ DocOps المضمنة في
`docs/examples/docs_preview_request_template.md`.