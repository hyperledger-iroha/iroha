---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-deployment.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: نشر المطور
العنوان: نشر ملاحظات SoraFS
Sidebar_label: تعليقات النشر
الوصف: قائمة التحقق من خط الأنابيب SoraFS من CI إلى الإنتاج.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/developer/deployment.md`. احرص على التأكد من النسختين متزامنتين إلى أن يتم إيقاف الوثائق القديمة.
:::

#تعليقات النشر

إنتاج مسار SoraFS الحتمية، لذا فإن الانتقال من CI إلى الإنتاج يتطلب ضوابط تشغيلية أساسًا. استخدم هذه القائمة عند نشر الأدوات على بوابات ومنظم تخزين حقيقي.

## قبل التشغيل

- **سجل مودمة** — شدد على أن ملفات Chunker والمانيفستات تشير إلى نفس ثلاثي `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **سياسة مقبولة** — إعلانات المحكمين الموقّعة وأدلة الاسم المستعار المطلوبة لـ `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **دليل تشغيل سجل التثبيت** — تستخدم بـ `docs/source/sorafs/runbooks/pin_registry_ops.md` للسيناريوهات الاستردادية (تدوير الاسم المستعار، إخفاقات النسخ المتماثل).

## إعدادات البيئة

- يجب على البوابات تفعيل نقطة نهاية مؤشرات الأدلة (`POST /v1/sorafs/proof/stream`) حتى يتمكن CLI من مسح ملخصات التليميترية.
- اضبط لـ `sorafs_alias_cache` باستخدام القيم الافتراضية في `iroha_config` أو عبر أداة CLI المساعدة (`sorafs_cli manifest submit --alias-*`).
- توفير رموز البريد الإلكتروني (أو بيانات موثوقة Torii) عبر مدير آمن.
- ففعل مصدّرات التليميرية (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) وأرسلها إلى الحزمة Prometheus/OTel الخاصة بك.

##فكرة جديدة1. **مانيفستات أزرق/أخضر**
   - استخدم `manifest submit --summary-out` لأرشفة الاستجابات لكل البرامج.
   - راقب `torii_sorafs_gateway_refusals_total` للتأكد من عدم تطابق القدرات.
2. **التحقق من الأدلة**
   - اعتبر إخفاقات `sorafs_cli proof stream` عوائق للنشر؛ غالبا ما نرى قمم الكمون إلى العازف أو طبقات غير مضبوطة.
   - يجب أن يكون `proof verify` جزءًا من اختبار الدخان بعد التثبيت وأظل CAR المستضاف لدى المتحكمين لا يزال يطابق المانيفست.
3. **لوحات تليميترية**
   - استورد `docs/examples/sorafs_proof_streaming_dashboard.json` إلى Grafana.
   - إضافة لوحات تشكيلية للاستهداف (`docs/source/sorafs/runbooks/pin_registry_ops.md`) و Structunk range.
4. **تمكين مصادر متعددة**
   - اتبع الخطوات الفارقة في `docs/source/sorafs/runbooks/multi_source_rollout.md` عند تفعيل المُنسِّق، وأرشِف آرتيفاكتات لوحة النتائج/التليمترية لأغراض التحصين.

## التعامل معها

- اتبع خطوات التصعيد في `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` لانقطاعات البوابة الغربية استنفاد رمز الدفق.
  - `dispute_revocation_runbook.md` نتيجة تفاعلات النسخ المتماثل.
  - `sorafs_node_ops.md` لصيانة مستوى العقدة.
  - `multi_source_rollout.md` لتجاوزات المنسِّق، ودراج الأقران في الشريط الأسود، والإطلاق المرحلي.
- سجّل إخفاقات الأدلة وشذوذات الكمون في GovernanceLog عبر واجهات PoR Tracker الحالية حتى البناء ال تور من تقييم أداء المتحكمين.

##الخطوات التالية

- امتثلت لإعلان المُنسِّق (`sorafs_car::multi_fetch`) عند توفر مُنسِّق الجلب ذو المصادر المتعددة (SF-6b).
- تتبع ترقية PDP/PoTR ضمن SF-13/SF-14؛ سيتطور CLI والوثائق التاريخية حتى المواعيد النهائية حتى بدأت تستقر تلك الأدلة.من خلال ظهور قوائم النشر السريعة هذه والدليل المميز لـ CI، يمكن للفرق الانتقال من التنينات المحلية إلى خطوط الأنابيب SoraFS جاهزة للإنتاج عملية قابلة للتكرار وقابلة للرصد.