---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-deployment.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: نشر المطور
العنوان: إعادة الاتصال SoraFS
Sidebar_label: علامات الاستفهام
الوصف: قائمة مختارة لإنتاج SoraFS من CI في السوق.
---

:::note Канонический источник
:::

# Заметки по разванию

يسهم التغليف العادي SoraFS في تعزيز القدرة على الحسم، حيث يتم عرض فترة زمنية لاحقة من CI في عملية حواجز الحماية الأساسية. استخدم هذه القائمة المختارة من خلال أدوات التشغيل الخاصة بالبوابات الحقيقية وموفري خدمات التخزين.

## التحقق المسبق

- **تسجيل الدخول** — تأكد من أن الملفات التعريفية والبيانات تتزامن مع لوحة واحدة `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **القبول السياسي** — التحقق من إعلانات المزودين وإثباتات الأسماء المستعارة، المطلوبة لـ `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook pin Register** — قم بالرجوع إلى `docs/source/sorafs/runbooks/pin_registry_ops.md` في مكان مناسب لتحسين السيناريو (الاسم المستعار للتدوير، النسخ المتماثلة).

## تخزين التكوين

- يجب أن تشتمل البوابات على دفق إثبات نقطة النهاية (`POST /v1/sorafs/proof/stream`)، بحيث يمكن لـ CLI استخدام البث عن بعد.
- قم بإنشاء السياسة `sorafs_alias_cache` باستخدام مساعد CLI (`sorafs_cli manifest submit --alias-*`).
- إعادة إرسال الرموز المميزة للتيار (أو البيانات الجيدة Torii) من خلال المدير السري الآمن.
- قم بتضمين مصدري القياس عن بعد (`torii_sorafs_proof_stream_*`، `torii_sorafs_chunk_range_*`) وقم بتوجيههم إلى جهازك Prometheus/OTel.## طرح الإستراتيجية

1. **بيانات اللون الأزرق/الأخضر**
   - استخدم `manifest submit --summary-out` للأرشفة عند بدء التشغيل.
   - اتبع الخطوات التالية لـ `torii_sorafs_gateway_refusals_total` لتستمتع بالقدرة على عدم الاحتمال.
2. **أدلة التحقق**
   - اقرأ المزيد في `sorafs_cli proof stream` فتح القفل; تشير معظم الخصائص الكامنة إلى حد ما إلى مزود الاختناق أو الطبقات غير القوية أبدًا.
   - `proof verify` يتم تفريغه في اختبار الدخان بعد الدبوس، مما يجعل السيارة في المقدمة متوافقة تمامًا مع بيان الملخص.
3. ** أجهزة القياس عن بعد للوحات المعلومات **
   - قم باستيراد `docs/examples/sorafs_proof_streaming_dashboard.json` إلى Grafana.
   - إضافة لوحة لتأمين رقم التعريف الشخصي (`docs/source/sorafs/runbooks/pin_registry_ops.md`) ومجموعة الإحصائيات.
4. **ملحقات متعددة المصادر**
   - اتبع الخطوات الأولية لبدء التشغيل من `docs/source/sorafs/runbooks/multi_source_rollout.md` مع تضمين المنسق وأرشفة العناصر الفنية للوحة النتائج/أجهزة القياس عن بعد لعمليات التدقيق.

## أحداث العمل

- اتبع تصعيد الدرجات في `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` لبوابة الانقطاع ورمز الدفق المميز.
  - `dispute_revocation_runbook.md`، عندما يتم استنساخ الجراثيم.
  - `sorafs_node_ops.md` لخدمة عقدة الكمبيوتر.
  - `multi_source_rollout.md` لتجاوز المنظمين وإدراج النظراء في القائمة السوداء وعمليات الطرح التجريبية.
- قم بتسجيل جميع الأدلة والشذوذات الكامنة في GovernanceLog من خلال واجهة برمجة تطبيقات PoR Tracker API، بحيث يمكن للحوكمة أن تزيد من إنتاجية مقدم.

## الخطوات التالية- دمج المنسق التلقائي (`sorafs_car::multi_fetch`)، عندما يقوم بمنسق الجلب متعدد المصادر (SF-6b).
- قم بإلغاء تحديد PDP/PoTR في رامكات SF-13/SF-14؛ سيتم تنشيط واجهة سطر الأوامر (CLI) والمستندات لاستعراض البيانات واختيار الطبقات عندما تستقر البراهين.

الالتزام بهذه الإرشادات المتعلقة بالبدء السريع وتوصيات CI، والأوامر المتدفقة من التجارب المحلية إلى درجة الإنتاج خطوط الأنابيب SoraFS مع عملية متقدمة وواضحة.