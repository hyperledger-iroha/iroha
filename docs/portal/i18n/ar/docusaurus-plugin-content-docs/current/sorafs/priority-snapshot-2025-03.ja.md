---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/priority-snapshot-2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f8585e4acbe29c62883cee5bae6e069859cf22533a9ffed9211acd133c1fc36a
source_last_modified: "2025-11-14T04:43:22.092284+00:00"
translation_last_reviewed: 2026-01-30
---

> المصدر المعتمد: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> الحالة: **بيتا / بانتظار ACKs التوجيه** (Networking, Storage, Docs leads).

## نظرة عامة

تحافظ لقطة مارس على اتساق مبادرات docs/content-network مع مسارات تسليم SoraFS
(SF-3, SF-6b, SF-9). بمجرد إقرار جميع القادة باللقطة في قناة Nexus steering،
أزل ملاحظة “Beta” أعلاه.

### محاور التركيز

1. **تعميم لقطة الأولويات** — جمع acknowledgements وتسجيلها في محاضر المجلس بتاريخ
   2025-03-05.
2. **إغلاق kickoff Gateway/DNS** — التدرب على حزمة التيسير الجديدة (القسم 6 في
   runbook) قبل ورشة 2025-03-03.
3. **ترحيل runbook للمشغلين** — بوابة `Runbook Index` أصبحت live؛ اكشف رابط
   المعاينة beta بعد توقيع reviewer onboarding.
4. **مسارات تسليم SoraFS** — مواءمة العمل المتبقي لـ SF-3/6b/9 مع plan/roadmap:
   - عامل ingestion لـ PoR + endpoint الحالة في `sorafs-node`.
   - صقل bindings الخاصة بـ CLI/SDK عبر تكاملات orchestrator في Rust/JS/Swift.
   - توصيل runtime لمنسق PoR وأحداث GovernanceLog.

راجع الملف المصدر للجدول الكامل وقائمة التوزيع وسجلات الإدخال.
