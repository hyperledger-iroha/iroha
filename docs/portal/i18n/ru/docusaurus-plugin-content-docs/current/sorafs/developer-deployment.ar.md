---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: developer-deployment
title: ملاحظات نشر SoraFS
sidebar_label: ملاحظات النشر
description: قائمة تحقق لترقية خط أنابيب SoraFS من CI إلى الإنتاج.
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/developer/deployment.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف الوثائق القديمة.
:::

# ملاحظات النشر

يعزز مسار تغليف SoraFS الحتمية، لذا فإن الانتقال من CI إلى الإنتاج يتطلب أساسًا ضوابط تشغيلية. استخدم هذه القائمة عند نشر الأدوات على بوابات ومزوّدي تخزين حقيقيين.

## قبل التشغيل

- **مواءمة السجل** — أكد أن ملفات chunker والمانيفستات تشير إلى نفس ثلاثية `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **سياسة القبول** — راجع إعلانات المزوّدين الموقّعة وأدلة alias المطلوبة لـ `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **دليل تشغيل سجل التثبيت** — احتفظ بـ `docs/source/sorafs/runbooks/pin_registry_ops.md` للسيناريوهات الاستردادية (تدوير alias، إخفاقات النسخ المتماثل).

## إعدادات البيئة

- يجب على البوابات تفعيل نقطة نهاية بث الأدلة (`POST /v1/sorafs/proof/stream`) حتى يتمكن CLI من إصدار ملخصات التليمترية.
- اضبط سياسة `sorafs_alias_cache` باستخدام القيم الافتراضية في `iroha_config` أو عبر أداة CLI المساعدة (`sorafs_cli manifest submit --alias-*`).
- وفّر رموز البث (أو بيانات اعتماد Torii) عبر مدير أسرار آمن.
- فعّل مصدّرات التليمترية (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) وأرسلها إلى حزمة Prometheus/OTel الخاصة بك.

## استراتيجية الإطلاق

1. **مانيفستات blue/green**
   - استخدم `manifest submit --summary-out` لأرشفة الاستجابات لكل إطلاق.
   - راقب `torii_sorafs_gateway_refusals_total` لاكتشاف عدم تطابق القدرات مبكرًا.
2. **التحقق من الأدلة**
   - اعتبر إخفاقات `sorafs_cli proof stream` عوائق للنشر؛ غالبًا ما تشير قمم الكمون إلى خنق المزوّد أو طبقات غير مضبوطة.
   - يجب أن يكون `proof verify` جزءًا من اختبار smoke بعد التثبيت لضمان أن CAR المستضاف لدى المزوّدين لا يزال يطابق digest المانيفست.
3. **لوحات التليمترية**
   - استورد `docs/examples/sorafs_proof_streaming_dashboard.json` إلى Grafana.
   - أضف لوحات لصحة سجل التثبيت (`docs/source/sorafs/runbooks/pin_registry_ops.md`) وإحصاءات chunk range.
4. **تمكين متعدد المصادر**
   - اتبع خطوات الإطلاق المرحلي في `docs/source/sorafs/runbooks/multi_source_rollout.md` عند تفعيل المُنسِّق، وأرشِف آرتيفاكتات scoreboard/التليمترية لأغراض التدقيق.

## التعامل مع الحوادث

- اتبع مسارات التصعيد في `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` لانقطاعات البوابة واستنفاد stream-token.
  - `dispute_revocation_runbook.md` عند وقوع نزاعات النسخ المتماثل.
  - `sorafs_node_ops.md` لصيانة مستوى العقدة.
  - `multi_source_rollout.md` لتجاوزات المُنسِّق، وإدراج الأقران في القائمة السوداء، والإطلاق المرحلي.
- سجّل إخفاقات الأدلة وشذوذات الكمون في GovernanceLog عبر واجهات PoR tracker الحالية حتى تتمكن الحوكمة من تقييم أداء المزوّدين.

## الخطوات التالية

- ادمج أتمتة المُنسِّق (`sorafs_car::multi_fetch`) عند توفر مُنسِّق الجلب متعدد المصادر (SF-6b).
- تتبع ترقيات PDP/PoTR ضمن SF-13/SF-14؛ سيتطور CLI والوثائق لإظهار المواعيد النهائية واختيار الطبقات بمجرد استقرار تلك الأدلة.

من خلال الجمع بين ملاحظات النشر هذه وبين دليل البدء السريع ووصفات CI، يمكن للفرق الانتقال من التجارب المحلية إلى خطوط أنابيب SoraFS جاهزة للإنتاج بعملية قابلة للتكرار وقابلة للرصد.
