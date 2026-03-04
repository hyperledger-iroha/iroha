---
lang: ar
direction: rtl
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-18T17:14:31.034360+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# سير عمل تطوير الوكلاء

يقوم دليل التشغيل هذا بدمج حواجز حماية المساهمين من خارطة طريق الوكلاء
التصحيحات الجديدة تتبع نفس البوابات الافتراضية.

## أهداف البداية السريعة

- قم بتشغيل `make dev-workflow` (المجمع حول `scripts/dev_workflow.sh`) لتنفيذ:
  1.`cargo fmt --all`
  2.`cargo clippy --workspace --all-targets --locked -- -D warnings`
  3.`cargo build --workspace --locked`
  4.`cargo test --workspace --locked`
  5. `swift test` من `IrohaSwift/`
- `cargo test --workspace` طويل الأمد (غالبًا ساعات). للتكرار السريع،
  استخدم `scripts/dev_workflow.sh --skip-tests` أو `--skip-swift`، ثم قم بتشغيل الملف بالكامل
  تسلسل قبل الشحن.
- إذا توقف `cargo test --workspace` عند إنشاء أقفال الدليل، فأعد التشغيل باستخدام
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (أو مجموعة
  `CARGO_TARGET_DIR` إلى مسار معزول) لتجنب التنافس.
- تستخدم جميع خطوات الشحن `--locked` لاحترام سياسة المستودع الخاصة بالحفظ
  `Cargo.lock` لم يمسها أحد. تفضل توسيع الصناديق الموجودة بدلاً من إضافتها
  أعضاء مساحة العمل الجدد؛ اطلب الموافقة قبل تقديم صندوق جديد.

## الدرابزين- يفشل `make check-agents-guardrails` (أو `ci/check_agents_guardrails.sh`) في حالة فشل
  يعدل الفرع `Cargo.lock` أو يقدم أعضاء جدد في مساحة العمل أو يضيف أعضاء جدد
  التبعيات. يقارن البرنامج النصي شجرة العمل و`HEAD` ضدها
  `origin/main` بشكل افتراضي؛ اضبط `AGENTS_BASE_REF=<ref>` لتجاوز القاعدة.
- `make check-dependency-discipline` (أو `ci/check_dependency_discipline.sh`)
  يفرق تبعيات `Cargo.toml` مقابل القاعدة ويفشل في الصناديق الجديدة؛ مجموعة
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` للاعتراف المتعمد
  الإضافات.
- `make check-missing-docs` (أو `ci/check_missing_docs_guard.sh`) كتل جديدة
  إدخالات `#[allow(missing_docs)]`، الأعلام التي لامست الصناديق (أقرب `Cargo.toml`)
  الذي يفتقر `src/lib.rs`/`src/main.rs` إلى مستندات `//!` على مستوى الصندوق، ويرفض الجديد
  العناصر العامة التي لا تحتوي على مستندات `///` بالنسبة إلى المرجع الأساسي؛ مجموعة
  `MISSING_DOCS_GUARD_ALLOW=1` فقط بموافقة المراجع. الحارس أيضاً
  يتحقق من أن `docs/source/agents/missing_docs_inventory.{json,md}` جديد؛
  التجديد باستخدام `python3 scripts/inventory_missing_docs.py`.
- `make check-tests-guard` (أو `ci/check_tests_guard.sh`) أعلام الصناديق التي
  وظائف الصدأ المتغيرة تفتقر إلى أدلة اختبار الوحدة. تغيرت خرائط الحرس الخطوط
  إلى الوظائف، ويمرر إذا تغيرت اختبارات الصندوق في الفرق، ويفحص خلاف ذلك
  ملفات الاختبار الموجودة لمطابقة استدعاءات الوظائف والتغطية الموجودة مسبقًا
  التهم. الصناديق التي لا تحتوي على أي اختبارات مطابقة سوف تفشل. تعيين `TEST_GUARD_ALLOW=1`
  فقط عندما تكون التغييرات محايدة حقًا للاختبار ويوافق المراجع.
- `make check-docs-tests-metrics` (أو `ci/check_docs_tests_metrics_guard.sh`)
  يفرض سياسة خارطة الطريق التي تتحرك بها المعالم جنبًا إلى جنب مع الوثائق،
  الاختبارات والمقاييس/لوحات المعلومات. عندما يتغير `roadmap.md` بالنسبة إلى
  `AGENTS_BASE_REF`، يتوقع الحارس تغيير مستند واحد على الأقل، وتغيير اختبار واحد،
  وتغيير واحد للمقاييس/القياس عن بعد/لوحة المعلومات. تعيين `DOC_TEST_METRIC_GUARD_ALLOW=1`
  فقط بموافقة المراجع.
- فشل `make check-todo-guard` (أو `ci/check_todo_guard.sh`) عند ظهور علامات TODO
  تختفي دون تغيير المستندات/الاختبارات المصاحبة. إضافة أو تحديث التغطية
  عند حل مهمة TODO، أو قم بتعيين `TODO_GUARD_ALLOW=1` لعمليات الإزالة المتعمدة.
- كتل `make check-std-only` (أو `ci/check_std_only.sh`) `no_std`/`wasm32`
  cfgs بحيث تظل مساحة العمل `std` فقط. قم بتعيين `STD_ONLY_GUARD_ALLOW=1` فقط لـ
  تجارب CI المعتمدة.
- `make check-status-sync` (أو `ci/check_status_sync.sh`) يبقي خريطة الطريق مفتوحة
  القسم خالي من العناصر المكتملة ويتطلب `roadmap.md`/`status.md`
  التغيير معًا حتى تظل الخطة/الحالة متوافقة؛ مجموعة
  `STATUS_SYNC_ALLOW_UNPAIRED=1` فقط لإصلاحات الأخطاء المطبعية النادرة للحالة فقط بعد ذلك
  تثبيت `AGENTS_BASE_REF`.
- يقوم `make check-proc-macro-ui` (أو `ci/check_proc_macro_ui.sh`) بتشغيل محاولة البناء
  مجموعات واجهة المستخدم لصناديق الاشتقاق/proc-macro. قم بتشغيله عند لمس وحدات الماكرو proc
  حافظ على استقرار تشخيصات `.stderr` واكتشف تراجعات واجهة المستخدم المذعورة؛ مجموعة
  `PROC_MACRO_UI_CRATES="crate1 crate2"` للتركيز على صناديق محددة.
- إعادة بناء `make check-env-config-surface` (أو `ci/check_env_config_surface.sh`)
  مخزون تبديل env (`docs/source/agents/env_var_inventory.{json,md}`)،
  يفشل إذا كان قديمًا، **و** يفشل عندما تظهر حشوات بيئة الإنتاج الجديدة
  نسبة إلى `AGENTS_BASE_REF` (يتم اكتشافه تلقائيًا؛ ويتم ضبطه بشكل صريح عند الحاجة).
  قم بتحديث المتعقب بعد إضافة/إزالة عمليات البحث عن env عبر
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  استخدم `ENV_CONFIG_GUARD_ALLOW=1` فقط بعد توثيق مقابض البيئة المتعمدةفي تعقب الهجرة.
- `make check-serde-guard` (أو `ci/check_serde_guard.sh`) يعيد إنشاء السيردي
  مخزون الاستخدام (`docs/source/norito_json_inventory.{json,md}`) إلى درجة حرارة
  الموقع، ويفشل إذا كان المخزون الملتزم قديمًا، ويرفض أي جديد
  عدد مرات الإنتاج `serde`/`serde_json` بالنسبة إلى `AGENTS_BASE_REF`. تعيين
  `SERDE_GUARD_ALLOW=1` فقط لتجارب CI بعد تقديم خطة الترحيل.
- `make guards` يفرض سياسة التسلسل Norito: فهو يرفض الجديد
  استخدام `serde`/`serde_json` ومساعدي AoS المخصصين وتبعيات SCALE بالخارج
  مقاعد Norito (`scripts/deny_serde_json.sh`،
  `scripts/check_no_direct_serde.sh`، `scripts/deny_handrolled_aos.sh`،
  `scripts/check_no_scale.sh`).
- **سياسة واجهة مستخدم Proc-macro:** يجب على كل صندوق proc-macro شحن `trybuild`
  الحزام (`tests/ui.rs` مع تمرير/فشل الكرة) خلف `trybuild-tests`
  ميزة. ضع عينات المسار السعيد ضمن `tests/ui/pass`، وحالات الرفض ضمن
  `tests/ui/fail` مع مخرجات `.stderr` المخصصة، والاحتفاظ بالتشخيصات
  عدم الذعر ومستقرة. تحديث التركيبات مع
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (اختياريًا مع
  `CARGO_TARGET_DIR=target-codex` لتجنب تحطيم الإصدارات الحالية) و
  تجنب الاعتماد على بنيات التغطية (من المتوقع وجود حراس `cfg(not(coverage))`).
  بالنسبة لوحدات الماكرو التي لا تصدر نقطة دخول ثنائية، تفضل
  `// compile-flags: --crate-type lib` في التركيبات للحفاظ على تركيز الأخطاء. أضف
  حالات سلبية جديدة كلما تغير التشخيص.
- يقوم CI بتشغيل البرامج النصية لحاجز الحماية عبر `.github/workflows/agents-guardrails.yml`
  لذلك تفشل طلبات السحب بسرعة عند انتهاك السياسات.
- يقوم خطاف git النموذجي (`hooks/pre-commit.sample`) بتشغيل حاجز الحماية والتبعية،
  المستندات المفقودة، وstd-only، وenv-config، ونصوص مزامنة الحالة حتى يصبح المساهمون
  القبض على انتهاكات السياسة قبل CI. احتفظ بفتات الخبز TODO لأي غرض متعمد
  المتابعات بدلاً من تأجيل التغييرات الكبيرة بصمت.