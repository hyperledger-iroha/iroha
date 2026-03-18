---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementación de desarrollador
título: ملاحظات نشر SoraFS
sidebar_label: ملاحظات النشر
descripción: قائمة تحقق لترقية خط أنابيب SoraFS من CI إلى الإنتاج.
---

:::nota المصدر المعتمد
Utilice el botón `docs/source/sorafs/developer/deployment.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف الوثائق القديمة.
:::

# ملاحظات النشر

Utilice el controlador SoraFS para instalar el CI en el dispositivo. استخدم هذه القائمة عند نشر الأدوات على بوابات ومزوّدي تخزين حقيقيين.

## قبل التشغيل

- **مواءمة السجل** — أكد أن ملفات fragmentador y تشير إلى نفس ثلاثية `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **سياسة القبول** — راجع إعلانات المزوّدين الموقّعة وأدلة alias المطلوبة لـ `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **دليل تشغيل سجل التثبيت** — احتفظ بـ `docs/source/sorafs/runbooks/pin_registry_ops.md` للسيناريوهات الاستردادية (تدوير alias, إخفاقات النسخ المتماثل).

## إعدادات البيئة

- Para obtener información sobre el producto (`POST /v1/sorafs/proof/stream`), acceda a la CLI de la aplicación.
- Utilice la interfaz `sorafs_alias_cache` para conectar la CLI a `iroha_config` y la CLI (`sorafs_cli manifest submit --alias-*`).
- وفّر رموز البث (أو بيانات اعتماد Torii) عبر مدير أسرار آمن.
- Utilice el teléfono móvil (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) y el teléfono móvil Prometheus/OTel.

## استراتيجية الإطلاق1. **azul/verde**
   - استخدم `manifest submit --summary-out` لأرشفة الاستجابات لكل إطلاق.
   - راقب `torii_sorafs_gateway_refusals_total` لاكتشاف عدم تطابق القدرات مبكرًا.
2. **التحقق من الأدلة**
   - اعتبر إخفاقات `sorafs_cli proof stream` عوائق للنشر؛ غالبًا ما تشير قمم الكمون إلى خنق المزوّد أو طبقات غير مضبوطة.
   - يجب أن يكون `proof verify` جزءًا من اختبار smoke بعد التثبيت لضمان أن CAR المستضاف لدى المزوّدين لا يزال يطابق digest المانيفست.
3. **لوحات التليمترية**
   - Nombre `docs/examples/sorafs_proof_streaming_dashboard.json` o Grafana.
   - أضف لوحات لصحة سجل التثبيت (`docs/source/sorafs/runbooks/pin_registry_ops.md`) y rango de fragmentos.
4. **تمكين متعدد المصادر**
   - اتبع خطوات الإطلاق المرحلي في `docs/source/sorafs/runbooks/multi_source_rollout.md` عند تفعيل المُنسِّق، وأرشِف آرتيفاكتات marcador/التليمترية لأغراض التدقيق.

## التعامل مع الحوادث

- Ajustes según `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` لانقطاعات البوابة y stream-token.
  - `dispute_revocation_runbook.md` عند وقوع نزاعات النسخ المتماثل.
  - `sorafs_node_ops.md` لصيانة مستوى العقدة.
  - `multi_source_rollout.md` لتجاوزات المُنسِّق، وإدراج الأقران في القائمة السوداء، والإطلاق المرحلي.
- Aplicaciones de GovernanceLog y rastreadores de PoR para aplicaciones de seguimiento de datos المزوّدين.

## الخطوات التالية

- ادمج أتمتة المُنسِّق (`sorafs_car::multi_fetch`) عند توفر مُنسِّق الجلب متعدد المصادر (SF-6b).
- تتبع ترقيات PDP/PoTR como SF-13/SF-14؛ Haga clic en CLI y haga clic en el enlace del enlace.من خلال الجمع بين ملاحظات النشر هذه وبين دليل البدء السريع ووصفات CI، يمكن للفرق الانتقال من التجارب المحلية Asegúrese de que el cable SoraFS esté encendido y apagado.