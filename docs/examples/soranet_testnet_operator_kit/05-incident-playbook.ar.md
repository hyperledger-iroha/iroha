---
lang: ar
direction: rtl
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-11-04T16:28:48.303168+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md -->

# دليل الاستجابة للbrownout / downgrade

1. **الرصد**
   - تفعيل تنبيه `soranet_privacy_circuit_events_total{kind="downgrade"}` او تشغيل webhook للbrownout من الحوكمة.
   - التحقق عبر `kubectl logs soranet-relay` او systemd journal خلال 5 دقائق.

2. **التثبيت**
   - تجميد تدوير الحارس (`relay guard-rotation disable --ttl 30m`).
   - تفعيل override direct-only للعملاء المتاثرين
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - التقاط hash الحالي لاعدادات الامتثال (`sha256sum compliance.toml`).

3. **التشخيص**
   - جمع احدث snapshot للدليل وحزمة مقاييس relay:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - تدوين عمق صف PoW، وعدادات throttling، وقفزات فئات GAR.
   - تحديد ما اذا كان الحدث بسبب عجز PQ او override امتثال او فشل relay.

4. **التصعيد**
   - ابلاغ جسر الحوكمة (`#soranet-incident`) بملخص وhash الحزمة.
   - فتح تذكرة حادث تربط التنبيه، مع تضمين timestamps وخطوات التخفيف.

5. **التعافي**
   - بعد معالجة السبب الجذري، اعادة تفعيل التدوير
     (`relay guard-rotation enable`) واعادة ضبط overrides direct-only.
   - مراقبة مؤشرات KPI لمدة 30 دقيقة؛ والتاكد من عدم ظهور brownout جديدة.

6. **ما بعد الحادث**
   - تقديم تقرير الحادث خلال 48 ساعة باستخدام قالب الحوكمة.
   - تحديث runbooks اذا تم اكتشاف نمط فشل جديد.

</div>
