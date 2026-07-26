---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: فهرس كتب التشغيل
العنوان: دليل التشغيل للمشغلين
Sidebar_label: دليل التشغيل
الوصف: نقطة الإدخال القانوني لمشغلي SoraFS المهاجرة.
---

> راجع سجل المسؤولين الذين يعيشون في `docs/source/sorafs/runbooks/`.
> يجب توسيع كل دليل جديد لعمليات SoraFS هنا مرة واحدة يتم نشره فيها
> بناء البوابة.

تُستخدم هذه الصفحة للتحقق مما إذا كانت كتب التشغيل قد أكملت عملية الترحيل من
شجرة التوثيق المنقولة على البوابة. كل ما أدخلت تعداد لا اللقب، لا
مسار الأصل الكنسي والنسخ على البوابة حتى تتمكن من المراجعة
انتقل مباشرة إلى الدليل المطلوب أثناء المشاهدة التجريبية التجريبية.

## مضيف فيستا بريفيا بيتا

يؤدي استخدام DocOps إلى ترقية مضيف المشاهدة إلى الإصدار التجريبي المعتمد من خلال المراجعات
`https://docs.iroha.tech/`. لتوجيه المشغلين أو المراجعين لترحيل دليل التشغيل،
المرجع هو اسم المضيف لاستخدام لحظة البوابة المحمية من خلال المجموع الاختباري.
إجراءات النشر/التراجع حية
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).| كتاب التشغيل | بروبيتاريو (ق) | نسخة على البوابة | فوينتي |
|---------|----------------|-------------------|--------|
| ترتيب البوابة و DNS | الشبكات TL، أتمتة العمليات، Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| دليل العمليات SoraFS | مستندات/ديفريل | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| التوفيق بين القدرات | الخزانة / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| عمليات تسجيل الدبابيس | الأدوات مجموعة العمل | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| قائمة التحقق من عمليات العقدة | فريق التخزين، SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook de Disputas y Revocaciones | مجلس الحكم | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Playbook de Manificos en Stage | مستندات/ديفريل | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| ملاحظة إضافة Taikai | منصة الوسائط WG / برنامج DA / الشبكات TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## قائمة التحقق

- [x] إن بناء البوابة المضمنة هو المؤشر (مدخل إلى الشريط الجانبي).
- [x] يقوم كل كتاب تشغيل بتعداد مسار الأصل الكنسي للحفاظ على المراجعة
  تمت إزالته خلال مراجعات المستندات.
- [x] يحظر مسار العرض المسبق لـ DocOps عمليات الدمج عند فشل دليل التشغيل
  قائمة الخروج من البوابة.Las migraciones futuras (ص. على سبيل المثال، محاكاة جديدة من Caos أو Appéndices de Gobernanza)
يجب إضافة ملف إلى الجدول السابق وتحديث قائمة التحقق من DocOps المدمجة في
`docs/examples/docs_preview_request_template.md`.