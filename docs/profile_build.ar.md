---
lang: ar
direction: rtl
source: docs/profile_build.md
status: complete
translator: manual
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-11-02T04:40:28.811778+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/profile_build.md (Profiling iroha_data_model Build) -->

# قياس أداء عملية بناء `iroha_data_model`

للعثور على الخطوات البطيئة في عملية بناء `iroha_data_model`، شغّل سكربت المساعدة:

```sh
./scripts/profile_build.sh
```

يقوم هذا السكربت بتشغيل الأمر `cargo build -p iroha_data_model --timings` وكتابة تقارير
الزمن في المسار `target/cargo-timings/`.
افتح ملف `cargo-timing.html` في المتصفح ثم قم بفرز المهام حسب مدّة التنفيذ لتحديد أي
crates أو مراحل البناء تستغرق أطول وقت.

استخدم هذه القياسات لتركيز جهود التحسين على المهام الأبطأ.

</div>

