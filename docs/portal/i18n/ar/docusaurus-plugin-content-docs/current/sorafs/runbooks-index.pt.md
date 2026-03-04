---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: فهرس كتب التشغيل
العنوان: دليل التشغيل للمشغلين
Sidebar_label: دليل التشغيل
الوصف: نقطة الإدخال الكنسي لمشغلي التشغيل SoraFS migrados.
---

> قم بإعادة كتابة سجل الاستجابة الموجود في `docs/source/sorafs/runbooks/`.
> يجب أن يتم هزيمة كل دليل عمليات SoraFS الجديد هنا كما لم يتم نشره
> بناء البوابة.

استخدم هذه الصفحة للتحقق من انتهاء دفاتر التشغيل من عملية ترحيل المستندات
بدائل للبوابة. كل قائمة بالمسؤولية، أو الطريق الكنسي الأصلي
ونسخة على البوابة حتى تتمكن المراجعات من توجيهها إلى الدليل المطلوب أثناء الإصدار التجريبي السابق.

## مضيف النسخة التجريبية

عند قيامك بترقية DocOps إلى المضيف التجريبي السابق، وافق على بعض المراجعات فيه
`https://docs.iroha.tech/`. لتوجيه المشغلين أو المراجعين لترحيل دليل التشغيل،
قم بالإشارة إلى اسم المضيف لاستخدام لقطة البوابة المحمية من خلال المجموع الاختباري.
إجراءات النشر/التراجع موجودة
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).| كتاب التشغيل | الرد(هو) | نسخة بدون بوابة | فونتي |
|---------|-----------------|-----------------|-------|
| بداية البوابة الإلكترونية DNS | الشبكات TL، أتمتة العمليات، Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| دليل العمليات الخاص بـ SoraFS | مستندات/ديفريل | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| تسوية القدرات | الخزانة / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| عمليات تسجيل الدبابيس | الأدوات مجموعة العمل | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| قائمة التحقق من عمليات عدم | فريق التخزين، SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook de Disputas e Revogações | مجلس الحكم | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| كتاب اللعب من بيان التدريج | مستندات/ديفريل | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| إمكانية ملاحظة رقبة Taikai | منصة الوسائط WG / برنامج DA / الشبكات TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## قائمة التحقق

- [x] بناء البوابة aponta para este índice (مدخل إلى الشريط الجانبي).
- [x] كل دليل تشغيل يقوم بتغيير القائمة أو المسار الأصلي الكنسي لإدارة المراجعات
  يتم الاحتفاظ بها طوال فترة مراجعة المستندات.
- [x] يتم دمج خط الأنابيب السابق لحظر DocOps عند وجود دليل تشغيل مدرج
  ausente da saída do port.الهجرة المستقبلية (على سبيل المثال، محاكاة جديدة للجزر أو ملحقات الحكم) هي
إضافة خط إلى اللوحة الأخيرة وتحديث قائمة مرجعية لـ DocOps المُدمجة فيها
`docs/examples/docs_preview_request_template.md`.