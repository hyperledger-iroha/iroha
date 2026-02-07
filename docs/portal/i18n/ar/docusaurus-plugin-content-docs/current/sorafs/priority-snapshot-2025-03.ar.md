---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: لقطة الأولوية-2025-03
العنوان: اللقطة الأولويات — مارس 2025 (بيتا)
description: نسخة مرآة توجيه Nexus 2025-03؛ بانتظار ACKs قبل طرح العام.
---

> المصدر مؤهل: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> الحالة: **بيتا / بانتظار ACKs توجيه** (الشبكات، التخزين، المستندات).

## نظرة عامة

حافظ على لقطة مارس على اتساق وثيق docs/content-network مع حافلات المقربين SoraFS
(SF-3، SF-6b، SF-9). بمجرد أن يتم تحقيق جميع أهداف اللقطة في قناة Nexus Steering،
أزل ملحوظة "بيتا" أعلاه.

### الأهداف المهمة

1. **تعميم لقطة الأولويات** — جمع الشكر والتقدير وتسجيلها في محاضرة المجلس بتاريخ
   2025-03-05.
2. ** إغلاق Kickstart Gateway/DNS** — التدرب على الحزمة التيسير الجديدة (القسم 6 في
   runbook) قبل ورشة 2025-03-03.
3. **ترحيل runbook للمشغلين** — أصبحت البوابة `Runbook Index` مباشرة؛ اكشف الرابط
   المعاينة بيتا بعد توقيع المراجع onboarding.
4. **مسارات إعادة التسليم SoraFS** — موامة العمل بنجاح لـ SF-3/6b/9 مع خطة/خارطة طريق:
   - عامل ابتلاع لـ PoR + endpoint Status في `sorafs-node`.
   - صقل الروابط الخاصة بـ CLI/SDK عبر تكاملات الأوركسترا في Rust/JS/Swift.
   - توصيل وقت التشغيل لمنسق PoR و الأحداث GovernanceLog.

راجع الملف المصدر للجدول الكامل وقائمة توزيعات الاشتراكات.