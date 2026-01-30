---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: priority-snapshot-2025-03
title: لقطة الأولويات — مارس 2025 (بيتا)
description: نسخة مرآة من لقطة توجيه Nexus 2025-03؛ بانتظار ACKs قبل الطرح العام.
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
