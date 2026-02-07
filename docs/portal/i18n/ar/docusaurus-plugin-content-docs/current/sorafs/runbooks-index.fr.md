---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: فهرس كتب التشغيل
العنوان: فهرس مشغلي كتب التشغيل
Sidebar_label: فهرس كتب التشغيل
الوصف: نقطة الدخول الأساسية لمشغلي كتب التشغيل SoraFS المهاجرة.
---

> قم بإعادة تسجيل المسؤولين الذين تم العثور عليهم في `docs/source/sorafs/runbooks/`.
> Chaque nouveau دليل الاستغلال SoraFS doit être lié ici dès qu'il est publié dans
> بناء البوابة.

استخدم هذه الصفحة للتحقق من أن كتب التشغيل قد أنهت ترحيل الشجرة
de docs héritée vers le portail. Chaque entrée indique la responsabilité, le chemin source
Canonique ونسخة البوابة تمكن القراء من الوصول مباشرة إلى الدليل
souhaité hanging l’aperçu bêta.

## Hôte d’aperçu bêta

إن DocOps الغامض المضطرب قد أدى إلى موافقة القراء على مستوى الفتحة التجريبية
`https://docs.iroha.tech/`. عندما تقوم بتوجيه المشغلين أو المراجعين إلى أحد
دليل التشغيل المهاجر، راجع اسم الفندق الذي يمكنك من خلاله استشارة لحظة الباب
تحت حماية المجموع الاختباري. يتم تنفيذ إجراءات النشر/التراجع هنا
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).| كتاب التشغيل | المسؤول (المسؤولون) | نسخ البوابة | المصدر |
|---------|----------------|------------|--------|
| بوابة لانسنت وDNS | الشبكات TL، أتمتة العمليات، Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| قواعد اللعب للاستغلال SoraFS | مستندات/ديفريل | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| التوفيق بين القدرات | الخزانة / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| عملية تسجيل الدبابيس | الأدوات مجموعة العمل | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| القائمة المرجعية لاستغلال البيانات | فريق التخزين، SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook الدعاوى والإبطالات | مجلس الحكم | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| كتاب اللعب من بيان التدريج | مستندات/ديفريل | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| إمكانية ملاحظة قضيب Taikai | منصة الوسائط WG / برنامج DA / الشبكات TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## قائمة التحقق

- [x] La build du portail renvoie vers cet Index (entrée de la barre latérale).
- [x] يتم ترحيل كل دليل تشغيل إلى قائمة المصدر الكنسي لحماية المتابعين
  تتماشى مع مراجعات الوثائق.
- [x] يحظر خط أنابيب فتح DocOps عمليات الدمج عند قائمة دليل التشغيل في لا
  طلعة على الباب.الهجرة المستقبلية (على سبيل المثال، تمارين الفوضى الجديدة أو ملحقات الحكم)
يجب إضافة سطر إلى اللوحة من خلاله والمتابعة اليومية لقائمة التحقق DocOps المتكاملة في
`docs/examples/docs_preview_request_template.md`.