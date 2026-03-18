---
lang: ar
direction: rtl
source: docs/source/README.md
status: complete
translator: manual
source_hash: 4a6e55a3232ff38c5c2f45b0a8a3d97471a14603bea75dc2034d7c9c4fb3f862
source_last_modified: "2025-11-10T19:43:50.185052+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

# فهرس توثيق Iroha VM و Kotodama

يربط هذا الفهرس بين أهم مستندات التصميم والمرجع الخاصة بـ IVM وKotodama
وخط الأنابيب (pipeline) المعتمد على IVM أولًا. لنسخة اليابانية، راجع
‎[`README.ja.md`](./README.ja.md)‎.

- بنية IVM وعلاقة الخرائط مع اللغة: ‎`../../ivm.md`‎
- واجهة ABI لنداءات النظام في IVM: ‎`ivm_syscalls.md`‎
- ثوابت نداءات النظام المولّدة: ‎`ivm_syscalls_generated.md`‎ (شغّل
  ‎`make docs-syscalls`‎ لإعادة توليدها)
- ترويسة بايت كود IVM: ‎`ivm_header.md`‎
- قواعد وتركيبة Kotodama (النحو والدلالات): ‎`kotodama_grammar.md`‎
- أمثلة Kotodama وربطها بنداءات النظام: ‎`kotodama_examples.md`‎
- خط أنابيب المعاملات (IVM‑first): ‎`../../new_pipeline.md`‎
- واجهة عقود Torii (manifests): ‎`torii_contracts_api.md`‎
- ظرف (envelope) استعلام JSON (للـ CLI / والأدوات): ‎`query_json.md`‎
- مرجع وحدة بث Norito: ‎`norito_streaming.md`‎
- عينات ABI وقت التشغيل: ‎`samples/runtime_abi_active.md`‎،
  ‎`samples/runtime_abi_hash.md`‎، ‎`samples/find_active_abi_versions.md`‎
- واجهة تطبيق ZK (المرفقات، الـ prover، وعدّ الأصوات): ‎`zk_app_api.md`‎
- كتيّب التشغيل للمرفقات/المثبت (prover) في Torii: ‎`zk/prover_runbook.md`‎
- دليل تشغيل واجهة ZK App API في Torii (مرفقات/prover؛ وثائق crate):
  ‎`../../crates/iroha_torii/docs/zk_app_api.md`‎
- Torii MCP API guide (agent/tool bridge; crate doc): `../../crates/iroha_torii/docs/mcp_api.md`
- دورة حياة مفاتيح التحقق/البراهين VK/proofs (التسجيل، التحقق، التليمترية):
  ‎`zk/lifecycle.md`‎
- أدوات مساعدة لمشغّل Torii (نقاط نهاية لمراقبة الحالة): ‎`references/operator_aids.md`‎
- دليل البدء السريع لممر Nexus الافتراضي: ‎`quickstart/default_lane.md`‎
- دليل البدء والبنية لمشرف MOCHI: ‎`mochi/index.md`‎
- أدلة JavaScript SDK (البدء السريع، التهيئة، النشر): ‎`sdk/js/index.md`‎
- لوحات المتابعة الخاصة بمقاييس Swift SDK وCI: ‎`references/ios_metrics.md`‎
- الحوكمة: ‎`../../gov.md`‎
- برومبتات تنسيق/توضيح الأسئلة: ‎`coordination_llm_prompts.md`‎
- خارطة الطريق: ‎`../../roadmap.md`‎
- استخدام صورة الـ Docker للبناء: ‎`docker_build.md`‎

نصائح الاستخدام

- قم ببناء وتشغيل الأمثلة الموجودة في ‎`examples/`‎ باستخدام الأدوات الخارجية
  ‎(`koto_compile`, `ivm_run`)‎:
  - ‎`make examples-run`‎ (و‎`make examples-inspect`‎ إذا كانت أداة ‎`ivm_tool`‎ متاحة)
- توجد اختبارات تكامل اختيارية (تُتجاهل افتراضيًا) للأمثلة وفحوصات الترويسة
  في ‎`integration_tests/tests/`‎.

تهيئة خط الأنابيب

- يُضبط كل سلوك وقت التشغيل عبر ملفات ‎`iroha_config`‎. لا تُستخدم متغيرات
  البيئة (environment variables) في مسارات التشغيل الخاصة بالمشغلين.
- توجد قيم افتراضية معقولة؛ معظم عمليات النشر لن تحتاج إلى تعديلها.
- المفاتيح المهمة ضمن القسم ‎`[pipeline]`‎:
  - ‎`dynamic_prepass`‎: تفعيل مرحلة prepass للقراءة فقط في IVM لاشتقاق مجموعات
    الوصول (القيمة الافتراضية: true).
  - ‎`access_set_cache_enabled`‎: تخزين مؤقت لمجموعات الوصول المشتقة لكل
    ‎`(code_hash, entrypoint)`‎؛ عطّلها لتتبّع التلميحات (افتراضيًا: true).
  - ‎`parallel_overlay`‎: إنشاء طبقات overlay بشكل متوازٍ؛ بينما يبقى
    الالتزام (commit) حتميًا (افتراضيًا: true).
  - ‎`gpu_key_bucket`‎: تجميع اختيار للـ keys في prepass الخاص بالجدولة باستخدام
    radix ثابت على ‎`(key, tx_idx, rw_flag)`‎؛ مسار الـ CPU الحتمي يظل مفعّلًا
    دائمًا (افتراضيًا: false).
  - ‎`cache_size`‎: سعة ذاكرة التخزين المؤقت العالمية لمرحلة ما قبل فك التشفير
    في IVM (streams مفككة مسبقًا). القيمة الافتراضية: 128. رفع القيمة قد يقلّل
    زمن فك التشفير في الحالات المتكررة.

فحوصات مزامنة التوثيق

- ثوابت نداءات النظام ‎(`docs/source/ivm_syscalls_generated.md`)‎
  - إعادة التوليد: ‎`make docs-syscalls`‎
  - التحقق فقط: ‎`bash scripts/check_syscalls_doc.sh`‎
- جدول ABI لنداءات النظام ‎(`crates/ivm/docs/syscalls.md`)‎
  - التحقق فقط:
    ‎`cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`‎
  - تحديث القسم المولّد (وجداول التوثيق في الكود):
    ‎`cargo run -p ivm --bin gen_syscalls_doc -- --write`‎
- جداول pointer‑ABI ‎(`crates/ivm/docs/pointer_abi.md`‎ و ‎`ivm.md`‎)
  - التحقق فقط:
    ‎`cargo run -p ivm --bin gen_pointer_types_doc -- --check`‎
  - تحديث الأقسام:
    ‎`cargo run -p ivm --bin gen_pointer_types_doc -- --write`‎
- سياسة ترويسة IVM وهاشات الـ ABI ‎(`docs/source/ivm_header.md`)‎
  - التحقق فقط:
    ‎`cargo run -p ivm --bin gen_header_doc -- --check`‎ و
    ‎`cargo run -p ivm --bin gen_abi_hash_doc -- --check`‎
  - تحديث الأقسام:
    ‎`cargo run -p ivm --bin gen_header_doc -- --write`‎ و
    ‎`cargo run -p ivm --bin gen_abi_hash_doc -- --write`‎

تكامل CI

- يقوم الـ workflow ‎`.github/workflows/check-docs.yml`‎ في GitHub Actions بتشغيل
  هذه الفحوصات على كل push/PR، وسيفشل إذا خرجت المستندات المولّدة عن
  التوافق مع التنفيذ الفعلي.
- ‎[دليل الحوكمة](governance_playbook.md)‎
</div>
