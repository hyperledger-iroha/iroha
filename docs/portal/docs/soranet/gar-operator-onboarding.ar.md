---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 74b0ef4843c441003cd6630f35e0deac4a736adad450270047a739c1b1d0a6fc
source_last_modified: "2025-11-21T13:08:42.404970+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: انضمام مشغلي GAR
sidebar_label: انضمام GAR
description: قائمة تحقق لتفعيل سياسات امتثال SNNet-9 مع digests الاتستة والتقاط الأدلة.
---

استخدم هذا الملخص لنشر إعدادات الامتثال SNNet-9 عبر عملية قابلة للتكرار وصديقة للتدقيق. اربطه بمراجعة الاختصاصات حتى يستخدم كل مشغل نفس digests ونفس تخطيط الأدلة.

## الخطوات

1. **تجميع الاعداد**
   - استورد `governance/compliance/soranet_opt_outs.json`.
   - ادمج `operator_jurisdictions` مع digests الاتستة المنشورة
     في [مراجعة الاختصاصات](gar-jurisdictional-review).
2. **التحقق**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - اختياري: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **التقاط الأدلة**
   - خزّن تحت `artifacts/soranet/compliance/<YYYYMMDD>/`:
     - `config.json` (كتلة الامتثال النهائية)
     - `attestations.json` (URIs + digests)
     - سجلات التحقق
     - مراجع لملفات PDF/Norito الموقعة
4. **التفعيل**
   - ضع وسم rollout (`gar-opt-out-<date>`)، وأعد نشر إعدادات orchestrator/SDK،
     وتأكد من ظهور أحداث `compliance_*` في السجلات المطلوبة.
5. **الاغلاق**
   - اودع حزمة الأدلة لدى Governance Council.
   - سجّل نافذة التفعيل والموافقين في GAR logbook.
   - جدولة تواريخ المراجعة التالية من جدول مراجعة الاختصاصات.

## قائمة تحقق سريعة

- [ ] `jurisdiction_opt_outs` يطابق الكتالوج القياسي.
- [ ] تم نسخ digests الاتستة بدقة.
- [ ] تم تشغيل أوامر التحقق وارشفتها.
- [ ] تم حفظ حزمة الأدلة في `artifacts/soranet/compliance/<date>/`.
- [ ] تم تحديث وسم rollout وGAR logbook.
- [ ] تم ضبط تذكيرات المراجعة التالية.

## راجع ايضا

- [GAR Jurisdictional Review](gar-jurisdictional-review)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)
