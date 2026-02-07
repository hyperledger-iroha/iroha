---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : déploiement par le développeur
titre : ملاحظات نشر SoraFS
sidebar_label : noms d'utilisateur
description: قائمة تحقق لترقية خط أنابيب SoraFS من CI إلى الإنتاج.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/developer/deployment.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف الوثائق القديمة.
:::

# ملاحظات النشر

يعزز مسار تغليف SoraFS الحتمية، لذا فإن الانتقال من CI إلى الإنتاج يتطلب أساسًا ضوابط تشغيلية. استخدم هذه القائمة عند نشر الأدوات على بوابات ومزوّدي تخزين حقيقيين.

## قبل التشغيل

- **مواءمة السجل** — Il s'agit d'un chunker qui se trouve dans le `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **سياسة القبول** — راجع إعلانات المزوّدين الموقّعة وأدلة alias المطلوبة لـ `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **دليل تشغيل سجل التثبيت** — احتفظ بـ `docs/source/sorafs/runbooks/pin_registry_ops.md` للسيناريوهات الاستردادية (تدوير alias, إخفاقات النسخ المتماثل).

## إعدادات البيئة

- يجب على البوابات تفعيل نقطة نهاية بث الأدلة (`POST /v1/sorafs/proof/stream`) حتى يتمكن CLI من إصدار ملخصات التليمترية.
- Utilisez le `sorafs_alias_cache` pour créer un lien vers le `iroha_config` et la CLI (`sorafs_cli manifest submit --alias-*`).
- وفّر رموز البث (أو بيانات اعتماد Torii) عبر مدير أسرار آمن.
- فعّل مصدّرات التليمترية (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) et Prometheus/OTel الخاصة بك.

## استراتيجية الإطلاق1. **مانيفستات bleu/vert**
   - استخدم `manifest submit --summary-out` لأرشفة الاستجابات لكل إطلاق.
   - راقب `torii_sorafs_gateway_refusals_total` لاكتشاف عدم تطابق القدرات مبكرًا.
2. **التحقق من الأدلة**
   - اعتبر إخفاقات `sorafs_cli proof stream` عوائق للنشر؛ غالبًا ما تشير قمم الكمون إلى خنق المزوّد أو طبقات غير مضبوطة.
   - يجب أن يكون `proof verify` جزءًا من اختبار smoke بعد التثبيت لضمان أن CAR المستضاف لدى المزوّدين لا يزال يطابق digérer المانيفست.
3. **لوحات التليمترية**
   - Remplacer `docs/examples/sorafs_proof_streaming_dashboard.json` par Grafana.
   - Vous pouvez utiliser la plage de fragments (`docs/source/sorafs/runbooks/pin_registry_ops.md`) pour la plage de fragments.
4. **تمكين متعدد المصادر**
   - اتبع خطوات الإطلاق المرحلي في `docs/source/sorafs/runbooks/multi_source_rollout.md` عند تفعيل المُنسِّق، وأرشِف آرتيفاكتات scoreboard/التليمترية لأغراض التدقيق.

## التعامل مع الحوادث

- اتبع مسارات التصعيد في `docs/source/sorafs/runbooks/` :
  - `sorafs_gateway_operator_playbook.md` pour le jeton de flux.
  - `dispute_revocation_runbook.md` عند وقوع نزاعات النسخ المتماثل.
  - `sorafs_node_ops.md` لصيانة مستوى العقدة.
  - `multi_source_rollout.md` Dispositifs de protection contre les fuites et les pressions.
- Consultez les informations disponibles sur GovernanceLog et le PoR tracker pour plus de détails. المزوّدين.

## الخطوات التالية

- ادمج أتمتة المُنسِّق (`sorafs_car::multi_fetch`) عند توفر مُنسِّق الجلب متعدد المصادر (SF-6b).
- Fonctions PDP/PoTR pour SF-13/SF-14 سيتطور CLI والوثائق لإظهار المواعيد النهائية واختيار الطبقات بمجرد استقرار تلك الأدلة.من خلال الجمع بين ملاحظات النشر هذه وبين دليل البدء السريع وووصفات CI, يمكن للفرق الانتقال من التجارب المحلية إلى خطوط أنابيب SoraFS جاهزة للإنتاج بعملية قابلة للتكرار وقابلة للرصد.