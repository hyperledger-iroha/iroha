---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-deployment.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: نشر المطور
العنوان: ملاحظات النشر SoraFS
Sidebar_label: ملاحظات النشر
الوصف: خط أنابيب CI للإنتاج SoraFS يروج لقائمة المراجعة.
---

:::ملاحظة مستند ماخذ
:::

# ملاحظات النشر

SoraFS التعبئة والتغليف حتمية سير العمل مضيعة للوقت، وهي تهدف إلى إنتاج CI من خلال حواجز الحماية التشغيلية في مانگتا. فيما يلي بعض البوابات الحقيقية وموفري خدمات التخزين قبل بدء التشغيل أو قائمة التحقق من الاستخدام.

## ما قبل الرحلة

- **محاذاة التسجيل** — يمكنك التحقق من ملفات تعريف وبيانات المقسم في صف `namespace.name@semver` الذي يشير إلى القائمة (`docs/source/sorafs/chunker_registry.md`).
- **سياسة القبول** — `manifest submit` کے لئے دکار إعلانات المزود الموقعة وإثباتات الأسماء المستعارة کا جائزہ لیں (`docs/source/sorafs/provider_admission_policy.md`).
- **دبوس سجل التشغيل** — سيناريوهات الاسترداد (تدوير الاسم المستعار، فشل النسخ المتماثل) `docs/source/sorafs/runbooks/pin_registry_ops.md` قریب رکھیں۔

## تكوين البيئة

- تمكّن البوابات التي تثبت نقطة النهاية للتدفق (`POST /v1/sorafs/proof/stream`) من إرسال ملخصات القياس عن بعد لـ CLI إلى كرنا ہوگا تاکہ.
- سياسة `sorafs_alias_cache` أو الإعدادات الافتراضية `iroha_config` أو مساعد CLI (`sorafs_cli manifest submit --alias-*`) الذي يتم تكوينه.
- رموز الدفق (أو بيانات اعتماد Torii) وهي عبارة عن مدير سري محفوظ.
- يقوم مصدرو القياس عن بعد (`torii_sorafs_proof_stream_*`، `torii_sorafs_chunk_range_*`) بتمكين الشحن والنقل عبر مكدس Prometheus/OTel.

## استراتيجية الطرح1. **بيانات اللون الأزرق/الأخضر**
   - يتم استخدام أرشيف الردود على الطرح `manifest submit --summary-out`.
   - `torii_sorafs_gateway_refusals_total` عدم تطابق قدرة القدرة على النظر إلى الجلد.
2. **التحقق من صحة الإثبات**
   - `sorafs_cli proof stream` من بين حالات الفشل في حاصرات النشر؛ ارتفاع زمن الاستجابة بسبب زيادة اختناق الموفر أو الطبقات التي تمت تهيئتها بشكل خاطئ.
   - اختبار دخان ما بعد الدبوس `proof verify` يتضمن تقنية اللعب التي يقدمها مقدمو خدمة CAR المستضافة ومطابقة ملخص البيان.
3. **لوحات معلومات القياس عن بعد**
   - `docs/examples/sorafs_proof_streaming_dashboard.json` وGrafana يتم استيراده.
   - صحة التسجيل الدبوس (`docs/source/sorafs/runbooks/pin_registry_ops.md`) وإحصائيات نطاق القطعة للوحات الإضافية.
4. **تمكين متعدد المصادر**
   - قام المنسق في `docs/source/sorafs/runbooks/multi_source_rollout.md` بتنظيم خطوات بدء التشغيل، وعمليات التدقيق لأرشفة لوحة النتائج/عناصر القياس عن بعد.

## التعامل مع الحوادث

- `docs/source/sorafs/runbooks/` مسارات التصعيد فالو کریں:
  - `sorafs_gateway_operator_playbook.md` انقطاع البوابة واستنفاد رمز الدفق.
  - `dispute_revocation_runbook.md` جب نزاعات النسخ المتماثل ہوں۔
  - `sorafs_node_ops.md` الصيانة على مستوى العقدة کے لئے۔
  - `multi_source_rollout.md` يتجاوز المنسق، ووضع القائمة السوداء للأقران، وعمليات الطرح المرحلية.
- إثبات حالات الفشل وشذوذ زمن الوصول في GovernanceLog الذي يتضمن واجهات برمجة التطبيقات (APIs) لتعقب PoR الموجودة والتي تسجل أداء موفر الحوكمة وتقييم الأداء.

## الخطوات التالية- منسق جلب متعدد المصادر (SF-6b) يعمل على أتمتة المنسق (`sorafs_car::multi_fetch`) يدمج کریں.
- ترقيات PDP/PoTR إلى SF-13/SF-14 في المسار الصحيح؛ عندما تثبت البراهين المواعيد النهائية لـ CLI والمستندات وسطح اختيار الطبقة.

إن ملاحظات النشر ووصفات التشغيل السريع وCI التي تتضمن تجارب محلية وخطوط أنابيب SoraFS من فئة الإنتاج هي عملية قابلة للتكرار ويمكن ملاحظتها وهي مستمرة.