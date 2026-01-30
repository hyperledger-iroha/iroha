---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/priority-snapshot-2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a82f7d16241718ead957a22dd1dc1ff59ae01e95ba345ea1c2d05822f1d7010f
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
id: priority-snapshot-2025-03
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
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
