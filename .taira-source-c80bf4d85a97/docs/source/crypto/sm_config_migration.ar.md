---
lang: ar
direction: rtl
source: docs/source/crypto/sm_config_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9b1be07edfee6d71031362a5ea95138a6b743a7e596537c1b1c02ce8edef9f
source_last_modified: "2026-01-22T15:38:30.660147+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! ترحيل تكوين SM

# ترحيل تكوين SM

يتطلب طرح مجموعة ميزات SM2/SM3/SM4 أكثر من مجرد تجميع باستخدام
علامة الميزة `sm`. تقوم العقد بوظيفة البوابة الموجودة خلف الطبقات
`iroha_config` ونتوقع أن يحمل بيان التكوين المطابقة
الإعدادات الافتراضية. تلتقط هذه الملاحظة سير العمل الموصى به عند الترويج لأحد المنتجات
الشبكة الحالية من "Ed25519 فقط" إلى "ممكّنة بـ SM".

## 1. تحقق من ملف تعريف البناء

- تجميع الثنائيات باستخدام `--features sm`؛ أضف `sm-ffi-openssl` فقط عندما
  خطط لممارسة مسار معاينة OpenSSL/Tongsuo. يبني بدون `sm`
  ميزة رفض توقيعات `sm2` أثناء القبول حتى إذا تم تمكين التكوين
  لهم.
- تأكد من قيام CI بنشر عناصر `sm` وأن جميع خطوات التحقق من الصحة (`البضائع
  اختبار -p iroha_crypto --ميزات sm`، تركيبات التكامل، أجنحة الزغب).
  على الثنائيات الدقيقة التي تنوي نشرها.

## 2. تجاوزات تكوين الطبقة

ينطبق `iroha_config` على ثلاثة مستويات: `defaults` → `user` → `actual`. شحن SM
يتم التجاوزات في ملف تعريف `actual` الذي يوزعه المشغلون على المدققين و
اترك `user` عند Ed25519 فقط حتى تظل الإعدادات الافتراضية للمطور دون تغيير.

```toml
# defaults/actual/config.toml
[crypto]
enable_sm_openssl_preview = false         # flip to true only when the preview backend is rolled out
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]      # keep sorted for deterministic manifests
sm2_distid_default = "CN12345678901234"   # organisation-specific distinguishing identifier
```

انسخ نفس الكتلة إلى بيان `defaults/genesis` عبر `kagami genesis
إنشاء ...` (add `--التوقيع المسموح به sm2 --التجزئة الافتراضية sm3-256` إذا كنت بحاجة
التجاوزات) لذا فإن كتلة `parameters` والبيانات الوصفية المحقونة تتفق مع
تكوين وقت التشغيل. يرفض الأقران البدء عند ظهور البيان والتكوين
اللقطات تتباعد.

## 3. تجديد بيانات سفر التكوين

- تشغيل `kagami genesis generate --consensus-mode <mode>` لكل
  البيئة وتنفيذ JSON المحدث جنبًا إلى جنب مع تجاوزات TOML.
- قم بالتوقيع على البيان (`kagami genesis sign …`) وقم بتوزيع الحمولة `.nrt`.
  العقد التي يتم تشغيلها من بيان JSON غير الموقع تستمد تشفير وقت التشغيل
  التكوين مباشرة من الملف — لا يزال يخضع لنفس الاتساق
  الشيكات.

## 4. التحقق قبل حركة المرور

- قم بتوفير مجموعة مرحلية تحتوي على الثنائيات والتكوينات الجديدة، ثم تحقق مما يلي:
  - يعرض `/status` `crypto.sm_helpers_available = true` بمجرد إعادة تشغيل النظراء.
  - قبول Torii لا يزال يرفض توقيعات SM2 بينما `sm2` غائب عن
    `allowed_signing` ويقبل دفعات Ed25519/SM2 المختلطة عند القائمة
    يتضمن كلا الخوارزميات.
  - `iroha_cli tools crypto sm2 export …` ذهابا وإيابا المواد الرئيسية المصنفة عبر الجديد
    الإعدادات الافتراضية.
- قم بتشغيل البرامج النصية لدخان التكامل التي تغطي التوقيعات الحتمية SM2 و
  تجزئة SM3 لتأكيد تناسق المضيف/VM.

## 5. خطة التراجع- توثيق الانعكاس: إزالة `sm2` من `allowed_signing` واستعادته
  `default_hash = "blake2b-256"`. ادفع التغيير من خلال نفس `actual`
  خط أنابيب الملف الشخصي بحيث ينقلب كل مدقق بشكل رتيب.
- احتفظ ببيانات SM على القرص؛ الأقران الذين يرون التكوين والنشأة غير متطابقين
  ترفض البيانات البدء، مما يحمي من التراجع الجزئي.
- إذا كانت معاينة OpenSSL/Tongsuo متضمنة، قم بتضمين خطوات التعطيل
  `crypto.enable_sm_openssl_preview` وإزالة الكائنات المشتركة من
  بيئة وقت التشغيل.

## المواد المرجعية

- [`docs/genesis.md`](../../genesis.md) – هيكل نشأة الظاهرة و
  الكتلة `crypto`.
- [`docs/source/references/configuration.md`](../references/configuration.md) –
  نظرة عامة على أقسام `iroha_config` والإعدادات الافتراضية.
- [`docs/source/crypto/sm_operator_rollout.md`](sm_operator_rollout.md) – النهاية إلى
  قائمة مرجعية للمشغل النهائي لشحن تشفير SM.