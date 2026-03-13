---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-deployment.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: نشر المطور
العنوان: Notas de despligue de SoraFS
Sidebar_label: ملاحظات النشر
الوصف: قائمة التحقق لتعزيز خط أنابيب SoraFS من CI للإنتاج.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/developer/deployment.md`. حافظ على الإصدارات المتزامنة حتى يتم سحب المستندات المتوارثة.
:::

# ملاحظات الرفض

التدفق المعبأ لـ SoraFS يعزز التحديد، حيث يتطلب عبور CI الإنتاج عمليات حماية بشكل أساسي. تستخدم هذه القائمة عندما توفر الأدوات والبوابات وموردي التخزين الحقيقيين.

## التحضير المسبق

- **تسجيل الدخول** — يؤكد أن ملفات التعريف والبيانات تشير إلى نفس المستوى `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **سياسة القبول** — قم بمراجعة إعلانات المورِّد الثابتة والأسماء المستعارة للإثباتات اللازمة لـ `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook de pin Registration** — احتفظ بـ `docs/source/sorafs/runbooks/pin_registry_ops.md` يدويًا لسيناريوهات الاسترداد (تدوير الاسم المستعار، فشل النسخ المتماثل).

## تكوين الإعداد- تحتاج البوابات إلى تمكين نقطة نهاية تدفق الأدلة (`POST /v2/sorafs/proof/stream`) حتى يقوم CLI بإصدار نتائج القياس عن بعد.
- تكوين السياسة `sorafs_alias_cache` باستخدام القيم المحددة مسبقًا لـ `iroha_config` أو مساعد CLI (`sorafs_cli manifest submit --alias-*`).
- توزيع الرموز المميزة للتيار (أو بيانات اعتماد Torii) من خلال مدير الأسرار الآمنة.
- تأهيل مصدري القياس عن بعد (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) وإرسال مكدسك Prometheus/OTel.

## استراتيجية الغزو1. **يظهر باللون الأزرق/الأخضر**
   - Usa `manifest submit --summary-out` لحفظ الإجابات التي تم إرسالها.
   - Vigila `torii_sorafs_gateway_refusals_total` للكشف عن السعة المؤقتة.
2. **التحقق من البراهين**
   - قم بتمرير السقوط إلى `sorafs_cli proof stream` مثل أقفال الإيقاف؛ تشير صور زمن الوصول إلى اختناق المورد أو مستويات التكوين غير الصحيحة.
   - يجب أن يقوم `proof verify` بتكوين جزء من اختبار الدخان الخلفي للتأكد من وصول السيارة من قبل الموردين الذين يتزامنون مع ملخص البيان.
3. **لوحات القياس عن بعد**
   - استيراد `docs/examples/sorafs_proof_streaming_dashboard.json` وGrafana.
   - إضافة لوحات إضافية لسلامة التسجيل (`docs/source/sorafs/runbooks/pin_registry_ops.md`) وإحصائيات النطاق.
4. ** التأهيل متعدد المصادر **
   - اتبع خطوات النزول للخطوات في `docs/source/sorafs/runbooks/multi_source_rollout.md` لتنشيط المنسق وحفظ عناصر لوحة النتائج/القياس عن بعد للمستمعين.

## إدارة الأحداث- متابعة مسارات التسلق في `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` لمفتاح البوابة ورمز الدفق المميز.
  - `dispute_revocation_runbook.md` عند حدوث نزاعات النسخ.
  - `sorafs_node_ops.md` للمحافظة على مستوى العقدة.
  - `multi_source_rollout.md` لتجاوز orquestador وقوائم الأقران السوداء وحذف البيانات.
- قم بتسجيل أخطاء الأدلة وحالات التأخير في GovernanceLog من خلال تتبع واجهات برمجة التطبيقات الخاصة بـ PoR الموجودة حتى تتمكن من تقييم أداء المورد.

## الخطوة التالية

- أتمتة متكاملة للأداة (`sorafs_car::multi_fetch`) أثناء تشغيل أداة الجلب متعددة المصادر (SF-6b).
- متابعة تحديثات PDP/PoTR باستخدام SF-13/SF-14؛ تم تطوير CLI والمستندات لتوضيح الساحات واختيار الطبقات عندما يتم تثبيت هذه الأدلة.

من خلال الجمع بين ملاحظات النزول مع Quickstart وبيانات CI، يمكن للمعدات المرور بتجربة محلية إلى خطوط الأنابيب SoraFS في الإنتاج من خلال عملية متكررة ويمكن ملاحظتها.