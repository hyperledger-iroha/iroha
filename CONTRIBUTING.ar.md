---
lang: ar
direction: rtl
source: CONTRIBUTING.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71baf5d038cbe6518fd294fcc1b279dff8aaf092e4a83f6159b699a378e51467
source_last_modified: "2025-12-08T10:55:43+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# دليل المساهمة

شكرًا لك على الوقت الذي أمضيته للمساهمة في Iroha 2!

يرجى قراءة هذا الدليل لمعرفة كيف يمكنك المساهمة والإرشادات التي نتوقع منك اتباعها. يتضمن ذلك الإرشادات حول التعليمات البرمجية والوثائق بالإضافة إلى اتفاقياتنا المتعلقة بسير عمل git.

قراءة هذه الإرشادات ستوفر عليك الوقت لاحقًا.

## كيف يمكنني المساهمة؟

هناك الكثير من الطرق التي يمكنك من خلالها المساهمة في مشروعنا:

- الإبلاغ عن [الأخطاء](#reporting-bugs) و[نقاط الضعف](#reporting-vulnerabilities)
- [اقتراح التحسينات](#suggesting-improvements) وتنفيذها
- [طرح الأسئلة](#asking-questions) والتفاعل مع المجتمع

هل أنت جديد في مشروعنا؟ [قم بمساهمتك الأولى](#your-first-code-contribution)!

### TL;DR

- ابحث عن [ZenHub](https://app.zenhub.com/workspaces/iroha-v2-60ddb820813b9100181fc060/board?repos=181739240).
- شوكة [Iroha](https://github.com/hyperledger-iroha/iroha/tree/main).
- إصلاح المشكلة التي تختارها.
- تأكد من اتباع [أدلة النمط] (#style-guides) للحصول على التعليمات البرمجية والوثائق.
- كتابة [الاختبارات](https://doc.rust-lang.org/cargo/commands/cargo-test.html). تأكد من اجتيازهم جميعًا (`cargo test --workspace`). إذا قمت بلمس مكدس التشفير SM، فقم أيضًا بتشغيل `cargo test -p iroha_crypto --features "sm sm_proptest"` لتنفيذ مجموعة Fuzz/الخاصية الاختيارية.
  - ملاحظة: الاختبارات التي تمارس المنفذ IVM ستقوم تلقائيًا بتجميع رمز ثانوي محدد ومحدد للمنفذ في حالة عدم وجود `defaults/executor.to`. ليست هناك حاجة إلى خطوة مسبقة لإجراء الاختبارات. لإنشاء رمز البايت المتعارف عليه للتكافؤ، يمكنك تشغيل:
    -`cargo run --manifest-path scripts/generate_executor_to/Cargo.toml`
    -`cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml`
- إذا قمت بتغيير صناديق المشتق/proc-macro، فقم بتشغيل مجموعات Trybuild UI عبر
  `make check-proc-macro-ui` (أو
  `PROC_MACRO_UI_CRATES="crate1 crate2" make check-proc-macro-ui`) والتحديث
  يتم تثبيت `.stderr` عند تغيير التشخيص للحفاظ على استقرار الرسائل.
- تشغيل `make dev-workflow` (المجمّع حول `scripts/dev_workflow.sh`) لتنفيذ fmt/clippy/build/test باستخدام `--locked` بالإضافة إلى `swift test`؛ توقع أن يستغرق `cargo test --workspace` ساعات وأن يستخدم `--skip-tests` فقط للحلقات المحلية السريعة. راجع `docs/source/dev_workflow.md` للحصول على دليل التشغيل الكامل.
- فرض حواجز الحماية باستخدام `make check-agents-guardrails` لمنع تعديلات `Cargo.lock` وصناديق مساحة العمل الجديدة، و`make check-dependency-discipline` للفشل في التبعيات الجديدة ما لم يسمح بذلك صراحة، و`make check-missing-docs` لمنع حشوات `#[allow(missing_docs)]` الجديدة، وفقدان المستندات على مستوى الصندوق عند لمسها الصناديق، أو العناصر العامة الجديدة بدون تعليقات المستند (يقوم الحارس بتحديث `docs/source/agents/missing_docs_inventory.{json,md}` عبر `scripts/inventory_missing_docs.py`). أضف `make check-tests-guard` بحيث تفشل الوظائف التي تم تغييرها ما لم تشير اختبارات الوحدة إليها (`#[cfg(test)]`/`#[test]` كتل أو صندوق `tests/`؛ أعداد التغطية الحالية) و`make check-docs-tests-metrics` بحيث يتم إقران تغييرات خريطة الطريق مع المستندات والاختبارات والمقاييس/لوحات المعلومات. حافظ على تطبيق TODO عبر `make check-todo-guard` حتى لا يتم إسقاط علامات TODO بدون المستندات/الاختبارات المصاحبة. يقوم `make check-env-config-surface` بإعادة إنشاء مخزون env-toggle ويفشل الآن عند ظهور حشوات البيئة **الإنتاج** الجديدة بالنسبة إلى `AGENTS_BASE_REF`؛ قم بتعيين `ENV_CONFIG_GUARD_ALLOW=1` فقط بعد توثيق الإضافات المتعمدة في متعقب الترحيل. يقوم `make check-serde-guard` بتحديث مخزون serde ويفشل في اللقطات القديمة أو نتائج الإنتاج الجديد `serde`/`serde_json`؛ قم بتعيين `SERDE_GUARD_ALLOW=1` فقط باستخدام خطة ترحيل معتمدة. اجعل التأجيلات الكبيرة مرئية عبر مسارات التنقل TODO وتذاكر المتابعة بدلاً من التأجيل بصمت. قم بتشغيل `make check-std-only` للقبض على `no_std`/`wasm32` cfgs و`make check-status-sync` لضمان بقاء العناصر المفتوحة `roadmap.md` مفتوحة فقط وأن تغييرات خريطة الطريق/الحالة تستقر معًا؛ قم بتعيين `STATUS_SYNC_ALLOW_UNPAIRED=1` فقط لإصلاحات الأخطاء المطبعية النادرة للحالة فقط بعد تثبيت `AGENTS_BASE_REF`. لاستدعاء واحد، استخدم `make agents-preflight` لتشغيل كل حواجز الحماية معًا.
- قم بتشغيل حراس التسلسل المحلي قبل الدفع: `make guards`.
  - يؤدي هذا إلى رفض `serde_json` المباشر في كود الإنتاج، ولا يسمح بعمليات serde المباشرة الجديدة خارج القائمة المسموح بها، ويمنع مساعدي AoS/NCB المخصصين خارج `crates/norito`.
- مصفوفة ميزات Norito التي يتم تشغيلها جافًا بشكل اختياري محليًا: `make norito-matrix` (تستخدم مجموعة فرعية سريعة).
  - للحصول على تغطية كاملة، قم بتشغيل `scripts/run_norito_feature_matrix.sh` بدون `--fast`.
  - لتضمين دخان في اتجاه مجرى النهر لكل مجموعة (الصندوق الافتراضي `iroha_data_model`): `make norito-matrix-downstream` أو `scripts/run_norito_feature_matrix.sh --fast --downstream [crate]`.
- بالنسبة لصناديق proc-macro، قم بإضافة مجموعة `trybuild` UI (`tests/ui.rs` + `tests/ui/pass`/`tests/ui/fail`) وقم بإجراء تشخيصات `.stderr` للحالات الفاشلة. حافظ على استقرار التشخيص وعدم الذعر؛ قم بتحديث التركيبات باستخدام `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` وحمايتها باستخدام `cfg(all(feature = "trybuild-tests", not(coverage)))`.
- تنفيذ روتين الالتزام المسبق مثل التنسيق وتجديد العناصر (راجع [`pre-commit.sample`](./hooks/pre-commit.sample))
- مع تعيين `upstream` لتتبع [مستودع Hyperledger Iroha](https://github.com/hyperledger-iroha/iroha)، و`git pull -r upstream main`، و`git commit -s`، و`git push <your-fork>`، و[إنشاء سحب طلب](https://github.com/hyperledger-iroha/iroha/compare) إلى فرع `main`. تأكد من أنه يتبع [إرشادات طلب السحب] (#pull-request-etiquette).

### البدء السريع لسير عمل الوكلاء

- قم بتشغيل `make dev-workflow` (المجمّع حول `scripts/dev_workflow.sh`، الموثق في `docs/source/dev_workflow.md`). وهو يلتف حول `cargo fmt --all`، و`cargo clippy --workspace --all-targets --locked -- -D warnings`، و`cargo build/test --workspace --locked` (يمكن أن تستغرق الاختبارات عدة ساعات)، و`swift test`.
- استخدم `scripts/dev_workflow.sh --skip-tests` أو `--skip-swift` لتكرارات أسرع؛ أعد تشغيل التسلسل الكامل قبل فتح طلب السحب.
- حواجز الحماية: تجنب لمس `Cargo.lock`، أو إضافة أعضاء جدد في مساحة العمل، أو تقديم تبعيات جديدة، أو إضافة حشوات `#[allow(missing_docs)]` جديدة، أو حذف المستندات على مستوى الصندوق، أو تخطي الاختبارات عند تغيير الوظائف، أو إسقاط علامات TODO بدون مستندات/اختبارات، أو إعادة تقديم `no_std`/`wasm32` cfgs دون موافقة. تشغيل `make check-agents-guardrails` (أو `AGENTS_BASE_REF=origin/main bash ci/check_agents_guardrails.sh`) بالإضافة إلى `make check-dependency-discipline`، `make check-missing-docs` (تحديث `docs/source/agents/missing_docs_inventory.{json,md}`)، `make check-tests-guard` (يفشل عندما تتغير وظائف الإنتاج دون دليل اختبار الوحدة - إما أن تتغير الاختبارات في الفرق أو يجب أن تشير الاختبارات الموجودة إلى دالة)، `make check-docs-tests-metrics` (يفشل عندما تفتقر تغييرات خريطة الطريق إلى تحديثات المستندات/الاختبارات/المقاييس)، `make check-todo-guard`، `make check-env-config-surface` (يفشل في المخزونات القديمة أو تبديل بيئة الإنتاج الجديدة؛ التجاوز باستخدام `ENV_CONFIG_GUARD_ALLOW=1` فقط بعد تحديث المستندات)، و `make check-serde-guard` (فشل في مخزونات serde التي لا معنى لها أو نتائج serde الإنتاج الجديدة؛ التجاوز مع `SERDE_GUARD_ALLOW=1` فقط مع خطة ترحيل معتمدة) محليًا للإشارة المبكرة، `make check-std-only` للحارس القياسي فقط، واحتفظ بمزامنة `roadmap.md`/`status.md` مع `make check-status-sync` (قم بتعيين `STATUS_SYNC_ALLOW_UNPAIRED=1` فقط لإصلاحات الأخطاء المطبعية النادرة للحالة فقط بعد تثبيت `AGENTS_BASE_REF`). استخدم `make agents-preflight` إذا كنت تريد أمرًا واحدًا لتشغيل جميع الحراس قبل فتح العلاقات العامة.

### الإبلاغ عن الأخطاء

*الخطأ* هو خطأ أو عيب في التصميم أو فشل أو خطأ في Iroha يؤدي إلى إنتاج نتيجة أو سلوك غير صحيح أو غير متوقع أو غير مقصود.

نحن نتتبع أخطاء Iroha عبر [GitHub Issues](https://github.com/hyperledger-iroha/iroha/issues?q=is%3Aopen+is%3Aissue+label%3ABug) المسمى بعلامة `Bug`.

عندما تقوم بإنشاء عدد جديد، يوجد نموذج يمكنك ملؤه. وإليك القائمة المرجعية لما يجب عليك فعله عند الإبلاغ عن الأخطاء:
- [ ] أضف العلامة `Bug`
- [ ] اشرح المشكلة
- [ ] تقديم الحد الأدنى من الأمثلة العملية
- [ ] أرفق لقطة شاشة

<details> <summary>الحد الأدنى من الأمثلة العملية</summary>

لكل خطأ، يجب عليك تقديم [الحد الأدنى من مثال العمل](https://en.wikipedia.org/wiki/Minimal_working_example). على سبيل المثال:

```
# Minting negative Assets with value spec `Numeric`.

I was able to mint negative values, which shouldn't be possible in Iroha. This is bad because <X>.

# Given

I managed to mint negative values by running
<paste the code here>

# I expected

not to be able to mint negative values

# But, I got

<code showing negative value>

<paste a screenshot>
```

</التفاصيل>

---
**ملاحظة:** يجب أن تستخدم مشكلات مثل الوثائق القديمة أو الوثائق غير الكافية أو طلبات الميزات التصنيفات `Documentation` أو `Enhancement`. انهم ليسوا البق.

---

### الإبلاغ عن نقاط الضعف

على الرغم من أننا نعمل بشكل استباقي على منع المشكلات الأمنية، فمن الممكن أن تصادف ثغرة أمنية قبل أن نفعل ذلك.

- قبل الإصدار الرئيسي الأول (2.0)، كانت جميع الثغرات الأمنية تعتبر أخطاء، لذا لا تتردد في إرسالها كأخطاء [باتباع الإرشادات أعلاه](#reporting-bugs).
- بعد الإصدار الرئيسي الأول، استخدم [برنامج مكافأة الأخطاء](https://hackerone.com/hyperledger) لإرسال نقاط الضعف والحصول على مكافأتك.

:exclamation: لتقليل الضرر الناتج عن ثغرة أمنية لم يتم تصحيحها، يجب عليك الكشف عن الثغرة الأمنية مباشرة إلى Hyperledger في أقرب وقت ممكن و **تجنب الكشف عن نفس الثغرة الأمنية علنًا** لفترة زمنية معقولة.

إذا كانت لديك أي أسئلة بخصوص تعاملنا مع الثغرات الأمنية، فلا تتردد في الاتصال بأي من المشرفين النشطين حاليًا في رسائل Rocket.Chat الخاصة.

### اقتراح التحسينات

قم بإنشاء [مشكلة] (https://github.com/hyperledger-iroha/iroha/issues/new) على GitHub باستخدام العلامات المناسبة (`Optimization`، `Enhancement`) ووصف التحسين الذي تقترحه. يمكنك أن تترك هذه الفكرة لنا أو لشخص آخر ليقوم بتطويرها، أو يمكنك تنفيذها بنفسك.

إذا كنت تنوي تنفيذ الاقتراح بنفسك، قم بما يلي:

1. قم بتعيين المشكلة التي قمت بإنشائها لنفسك **قبل** أن تبدأ العمل عليها.
2. اعمل على الميزة التي اقترحتها واتبع [إرشاداتنا الخاصة بالكود والوثائق](#style-guides).
3. عندما تكون مستعدًا لفتح طلب سحب، تأكد من اتباع [إرشادات طلب السحب](#pull-request-etiquette) ووضع علامة عليه باعتباره تنفيذ المشكلة التي تم إنشاؤها مسبقًا:

   ```
   feat: Description of the feature

   Explanation of the feature

   Closes #1234
   ```

4. إذا كان التغيير الذي أجريته يتطلب تغيير واجهة برمجة التطبيقات (API)، فاستخدم العلامة `api-changes`.

   **ملاحظة:** الميزات التي تتطلب تغييرات في واجهة برمجة التطبيقات (API) قد تستغرق وقتًا أطول للتنفيذ والموافقة لأنها تتطلب من صانعي مكتبة Iroha تحديث التعليمات البرمجية الخاصة بهم.### طرح الأسئلة

السؤال هو أي مناقشة ليست خطأ أو ميزة أو طلب تحسين.

<تفاصيل> <ملخص> كيف يمكنني طرح سؤال؟ </ملخص>

يرجى نشر أسئلتك على [إحدى منصات المراسلة الفورية لدينا] (#contact) حتى يتمكن الموظفون وأعضاء المجتمع من مساعدتك في الوقت المناسب.

أنت، كجزء من المجتمع المذكور أعلاه، يجب أن تفكر في مساعدة الآخرين أيضًا. إذا قررت المساعدة، يرجى القيام بذلك [بطريقة محترمة](CODE_OF_CONDUCT.md).

</التفاصيل>

## مساهمتك الأولى بالرمز

1. ابحث عن إصدار مناسب للمبتدئين من بين الإصدارات ذات التصنيف [الإصدار الأول الجيد](https://github.com/hyperledger-iroha/iroha/labels/good%20first%20issue).
2. تأكد من عدم قيام أي شخص آخر بالعمل على القضايا التي اخترتها عن طريق التحقق من عدم إسنادها لأي شخص.
3. قم بتخصيص المشكلة لنفسك حتى يتمكن الآخرون من رؤية أن هناك من يعمل عليها.
4. اقرأ [دليل نمط الصدأ] (#rust-style-guide) قبل البدء في كتابة التعليمات البرمجية.
5. عندما تكون مستعدًا لتنفيذ التغييرات، اقرأ [إرشادات طلب السحب](#pull-request-etiquette).

## آداب طلب السحب

من فضلك [تفرع](https://docs.github.com/en/get-started/quickstart/fork-a-repo) [المستودع](https://github.com/hyperledger-iroha/iroha/tree/main) و[إنشاء فرع ميزات](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) لمساهماتك. عند العمل مع **PRs from forks**، راجع [هذا الدليل](https://help.github.com/articles/checking-out-pull-requests-locally).

#### العمل على المساهمة بالكود:
- اتبع [دليل نمط الصدأ](#rust-style-guide) و[دليل نمط التوثيق](#documentation-style-guide).
- التأكد من أن الكود الذي كتبته مشمول بالاختبارات. إذا أصلحت خطأ ما، فيرجى تحويل الحد الأدنى من مثال العمل الذي يعيد إنتاج الخطأ إلى اختبار.
- عند لمس صناديق المشتق/proc-macro، قم بتشغيل `make check-proc-macro-ui` (أو
  قم بالتصفية باستخدام `PROC_MACRO_UI_CRATES="crate1 crate2"`) لذا حاول إنشاء تركيبات واجهة المستخدم
  البقاء متزامنًا وتبقى التشخيصات مستقرة.
- توثيق واجهات برمجة التطبيقات العامة الجديدة (`//!` و`///` على العناصر الجديدة) وتشغيلها
  `make check-missing-docs` للتحقق من الدرابزين. استدعاء المستندات/الاختبارات لك
  تمت إضافتها في وصف طلب السحب الخاص بك.

#### الالتزام بعملك:
- اتبع [دليل أنماط Git](#git-workflow).
- قم بإلغاء التزاماتك [إما قبل](https://www.git-tower.com/learn/git/faq/git-squash/) أو [أثناء الدمج](https://rietta.com/blog/github-merge-types/).
- إذا أصبح فرعك قديمًا أثناء إعداد طلب السحب، فقم بإعادة تأسيسه محليًا باستخدام `git pull --rebase upstream main`. وبدلاً من ذلك، يمكنك استخدام القائمة المنسدلة للزر `Update branch` واختيار الخيار `Update with rebase`.

  من أجل تسهيل هذه العملية للجميع، حاول ألا يكون لديك أكثر من عدد قليل من الالتزامات لطلب السحب، وتجنب إعادة استخدام فروع الميزات.

#### إنشاء طلب سحب:
- استخدم وصفًا مناسبًا لطلب السحب باتباع الإرشادات الواردة في قسم [آداب طلب السحب](#pull-request-etiquette). تجنب الانحراف عن هذه الإرشادات إن أمكن.
- قم بإضافة [عنوان طلب السحب] (#pull-request-titles) بتنسيق مناسب.
- إذا كنت تشعر أن الكود الخاص بك ليس جاهزًا للدمج، ولكنك تريد من المشرفين الاطلاع عليه، فقم بإنشاء مسودة طلب سحب.

#### دمج عملك:
- يجب أن يجتاز طلب السحب جميع الفحوصات الآلية قبل دمجه. كحد أدنى، يجب أن يتم تنسيق الكود، واجتياز كافة الاختبارات، بالإضافة إلى عدم وجود خطوط `clippy` معلقة.
- لا يمكن دمج طلب السحب بدون مراجعتين موافقتين من المشرفين النشطين.
- سيقوم كل طلب سحب بإخطار مالكي الكود تلقائيًا. يمكن العثور على قائمة محدثة بالمشرفين الحاليين في [MAINTAINERS.md](MAINTAINERS.md).

#### آداب المراجعة:
- لا تحل المحادثة بنفسك. دع المراجع يتخذ القرار.
- الإقرار بتعليقات المراجعة والتفاعل مع المراجع (الموافقة، عدم الموافقة، التوضيح، الشرح، وما إلى ذلك). لا تتجاهل التعليقات.
- للحصول على اقتراحات بسيطة لتغيير التعليمات البرمجية، إذا قمت بتطبيقها مباشرة، فيمكنك حل المحادثة.
- تجنب الكتابة فوق التزاماتك السابقة عند دفع التغييرات الجديدة. إنه يحجب ما تغير منذ المراجعة الأخيرة ويجبر المراجع على البدء من الصفر. يتم سحق الالتزامات قبل الدمج تلقائيًا.

### سحب عناوين الطلب

نقوم بتحليل عناوين كافة طلبات السحب المدمجة لإنشاء سجلات التغيير. نتحقق أيضًا من أن العنوان يتبع الاصطلاح من خلال فحص *`check-PR-title`*.

لاجتياز اختبار *`check-PR-title`*، يجب أن يلتزم عنوان طلب السحب بالإرشادات التالية:

<details> <summary> قم بالتوسيع لقراءة إرشادات العنوان التفصيلية</summary>

1. اتبع تنسيق [الالتزامات التقليدية](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers).

2. إذا كان طلب السحب يحتوي على التزام واحد، فيجب أن يكون عنوان PR هو نفس رسالة الالتزام.

</التفاصيل>

### سير عمل جيت

- [Fork](https://docs.github.com/en/get-started/quickstart/fork-a-repo) و[المستودع](https://github.com/hyperledger-iroha/iroha/tree/main) و[إنشاء فرع ميزات](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) لمساهماتك.
- [قم بتكوين جهاز التحكم عن بعد](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/configuring-a-remote-repository-for-a-fork) لمزامنة شوكتك مع [مستودع Hyperledger Iroha](https://github.com/hyperledger-iroha/iroha/tree/main).
- استخدم [سير عمل Git Rebase](https://git-rebase.io/). تجنب استخدام `git pull`. استخدم `git pull --rebase` بدلاً من ذلك.
- استخدم [خطافات git](./hooks/) المتوفرة لتسهيل عملية التطوير.

اتبع إرشادات الالتزام هذه:

- **تسجيل الخروج من كل التزام**. إذا لم تقم بذلك، فلن يسمح لك [DCO](https://github.com/apps/dco) بالدمج.

  استخدم `git commit -s` لإضافة `Signed-off-by: $NAME <$EMAIL>` تلقائيًا باعتباره السطر الأخير لرسالة الالتزام الخاصة بك. يجب أن يكون اسمك وبريدك الإلكتروني هو نفسه المحدد في حساب GitHub الخاص بك.

  نحن نشجعك أيضًا على توقيع التزاماتك باستخدام مفتاح GPG باستخدام `git commit -sS` ([مزيد من المعلومات](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)).

  يمكنك استخدام [الخطاف `commit-msg`](./hooks/) لتسجيل الخروج من التزاماتك تلقائيًا.

- يجب أن تتبع رسائل الالتزام [الالتزامات التقليدية](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) ونفس مخطط التسمية كما هو الحال في [عناوين طلبات السحب](#pull-request-titles). هذا يعني:
  - **استخدم زمن المضارع** ("أضف ميزة"، وليس "ميزة مضافة")
  - **استخدم الحالة الأمرية** ("النشر إلى عامل الإرساء..." وليس "النشر إلى عامل الإرساء...")
- اكتب رسالة التزام ذات معنى.
- حاول إبقاء رسالة الالتزام قصيرة.
- إذا كنت بحاجة إلى رسالة التزام أطول:
  - حدد السطر الأول من رسالة الالتزام بـ 50 حرفًا أو أقل.
  - يجب أن يحتوي السطر الأول من رسالة الالتزام على ملخص العمل الذي قمت به. إذا كنت بحاجة إلى أكثر من سطر واحد، فاترك سطرًا فارغًا بين كل فقرة ووصف التغييرات التي أجريتها في المنتصف. يجب أن يكون السطر الأخير هو تسجيل الخروج.
- إذا قمت بتعديل المخطط (تحقق من خلال إنشاء المخطط باستخدام `kagami schema` وdiff)، فيجب عليك إجراء جميع التغييرات على المخطط في التزام منفصل بالرسالة `[schema]`.
- حاول الالتزام بالتزام واحد لكل تغيير ذي معنى.
  - إذا قمت بإصلاح العديد من المشكلات في علاقة عامة واحدة، فقم بمنحهم التزامات منفصلة.
  - كما ذكرنا سابقًا، يجب إجراء التغييرات على `schema` وواجهة برمجة التطبيقات (API) في التزامات مناسبة منفصلة عن بقية عملك.
  - إضافة اختبارات للوظيفة في نفس الالتزام مثل تلك الوظيفة.

## الاختبارات والمقاييس

- لتشغيل الاختبارات المستندة إلى التعليمات البرمجية المصدر، قم بتنفيذ [`cargo test`](https://doc.rust-lang.org/cargo/commands/cargo-test.html) في جذر Iroha. لاحظ أن هذه عملية طويلة.
- لتشغيل المعايير، قم بتنفيذ [`cargo bench`](https://doc.rust-lang.org/cargo/commands/cargo-bench.html) من جذر Iroha. للمساعدة في تصحيح أخطاء مخرجات قياس الأداء، قم بتعيين متغير البيئة `debug_assertions` مثل: `RUSTFLAGS="--cfg debug_assertions" cargo bench`.
- إذا كنت تعمل على مكون معين، فضع في اعتبارك أنه عند تشغيل `cargo test` في [مساحة عمل](https://doc.rust-lang.org/cargo/reference/workspaces.html)، فإنه سيتم تشغيل الاختبارات الخاصة بمساحة العمل هذه فقط، والتي لا تتضمن عادةً أي [اختبارات تكامل](https://www.testingxperts.com/blog/what-is-integration-testing).
- إذا كنت تريد اختبار تغييراتك على الحد الأدنى من الشبكة، فإن [`docker-compose.yml`](defaults/docker-compose.yml) المتوفر يقوم بإنشاء شبكة مكونة من 4 أقران Iroha في حاويات عامل الإرساء التي يمكن استخدامها لاختبار الإجماع والمنطق المتعلق بنشر الأصول. نوصي بالتفاعل مع تلك الشبكة باستخدام إما [I18NI000000245X](https://github.com/hyperledger-iroha/iroha-python)، أو واجهة سطر الأوامر للعميل Iroha المضمنة.
- لا تقم بإزالة الاختبارات الفاشلة. حتى الاختبارات التي تم تجاهلها سيتم تشغيلها في مسارنا في النهاية.
- إذا كان ذلك ممكنًا، يرجى قياس الكود الخاص بك قبل وبعد إجراء التغييرات، حيث أن التراجع الكبير في الأداء يمكن أن يؤدي إلى تعطيل عمليات تثبيت المستخدمين الحاليين.

### فحوصات حراسة التسلسل

قم بتشغيل `make guards` للتحقق من صحة سياسات المستودع محليًا:

- رفض إدراج `serde_json` المباشر في مصادر الإنتاج (يفضل `norito::json`).
- منع التبعيات/الاستيرادات المباشرة `serde`/`serde_json` خارج القائمة المسموح بها.
- منع إعادة تقديم مساعدات AoS/NCB المخصصة خارج `crates/norito`.

### اختبارات التصحيح

<details> <summary> قم بالتوسيع لتتعلم كيفية تغيير مستوى السجل أو كتابة السجلات إلى JSON.</summary>

إذا فشل أحد اختباراتك، فقد ترغب في تقليل الحد الأقصى لمستوى التسجيل. افتراضيًا، يقوم Iroha بتسجيل رسائل المستوى `INFO` فقط، ولكنه يحتفظ بالقدرة على إنتاج سجلات المستوى `DEBUG` و`TRACE`. يمكن تغيير هذا الإعداد إما باستخدام متغير البيئة `LOG_LEVEL` للاختبارات المستندة إلى التعليمات البرمجية، أو باستخدام نقطة النهاية `/configuration` على أحد النظراء في شبكة منشورة.على الرغم من أن السجلات المطبوعة في `stdout` كافية، فقد تجد أنه من الملائم أكثر إنتاج سجلات بتنسيق `json` في ملف منفصل وتحليلها باستخدام إما [node-bunyan](https://www.npmjs.com/package/bunyan) أو [rust-bunyan](https://crates.io/crates/bunyan).

قم بتعيين متغير البيئة `LOG_FILE_PATH` إلى موقع مناسب لتخزين السجلات وتحليلها باستخدام الحزم المذكورة أعلاه.

</التفاصيل>

### تصحيح الأخطاء باستخدام وحدة تحكم tokio

<details> <summary> قم بالتوسيع لتتعلم كيفية ترجمة Iroha مع دعم وحدة تحكم tokio.</summary>

في بعض الأحيان قد يكون من المفيد تصحيح الأخطاء لتحليل مهام tokio باستخدام [tokio-console](https://github.com/tokio-rs/console).

في هذه الحالة يجب عليك ترجمة Iroha مع دعم وحدة التحكم tokio مثل هذا:

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
```

يمكن تكوين منفذ وحدة تحكم tokio من خلال معلمة التكوين `LOG_TOKIO_CONSOLE_ADDR` (أو متغير البيئة).
يتطلب استخدام وحدة تحكم tokio أن يكون مستوى السجل `TRACE`، ويمكن تمكينه من خلال معلمة التكوين أو متغير البيئة `LOG_LEVEL`.

مثال على تشغيل Iroha مع دعم وحدة تحكم tokio باستخدام `scripts/test_env.sh`:

```bash
# 1. Compile Iroha
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
# 2. Run Iroha with TRACE log level
LOG_LEVEL=TRACE ./scripts/test_env.sh setup
# 3. Access Iroha. Peers will be available on ports 5555, 5556, ...
tokio-console http://127.0.0.1:5555
```

</التفاصيل>

### التنميط

<تفاصيل> <ملخص> قم بالتوسيع للتعرف على كيفية إنشاء ملف تعريف Iroha. </ملخص>

لتحسين الأداء، من المفيد إنشاء ملف تعريف Iroha.

تتطلب إنشاءات ملفات التعريف حاليًا سلسلة أدوات ليلية. لإعداد واحدة، قم بتجميع Iroha مع ملف التعريف والميزة `profiling` باستخدام `cargo +nightly`:

```bash
RUSTFLAGS="-C force-frame-pointers=on" cargo +nightly -Z build-std build --target your-desired-target --profile profiling --features profiling
```

ثم ابدأ تشغيل Iroha وقم بإرفاق ملف تعريف من اختيارك بمعرف الهوية Iroha.

وبدلاً من ذلك، من الممكن إنشاء Iroha داخل عامل الإرساء مع دعم ملف التعريف وملف التعريف Iroha بهذه الطريقة.

```bash
docker build -f Dockerfile.glibc --build-arg="PROFILE=profiling" --build-arg='RUSTFLAGS=-C force-frame-pointers=on' --build-arg='FEATURES=profiling' --build-arg='CARGOFLAGS=-Z build-std' -t iroha:profiling .
```

على سبيل المثال باستخدام الأداء (متوفر فقط على نظام التشغيل Linux):

```bash
# to capture profile
sudo perf record -g -p <PID>
# to analyze profile
sudo perf report
```

لتتمكن من مراقبة ملف تعريف المنفذ أثناء إنشاء ملف تعريف Iroha، يجب تجميع المنفذ دون تجريد الرموز.
يمكن القيام بذلك عن طريق تشغيل:

```bash
# compile executor without optimizations
cargo run --bin kagami -- ivm build ./path/to/executor --out-file executor.to
```

مع تمكين ميزة التوصيف، يعرض Iroha نقطة النهاية لملفات تعريف pprof:

```bash
# profile Iroha for 30 seconds and download the profile data
curl host:port/debug/pprof/profile?seconds=30 -o profile.pb
# analyze profile in browser (required installed go)
go tool pprof -web profile.pb
```

</التفاصيل>

## أدلة الأسلوب

يرجى اتباع هذه الإرشادات عند تقديم مساهمات التعليمات البرمجية لمشروعنا:

### دليل أسلوب جيت

:book: [اقرأ إرشادات git](#git-workflow)

### دليل نمط الصدأ

<details> <summary> :book: اقرأ إرشادات التعليمات البرمجية</summary>

- استخدم `cargo fmt --all` (إصدار 2024) لتنسيق التعليمات البرمجية.

إرشادات الكود:

- ما لم ينص على خلاف ذلك، راجع [أفضل ممارسات الصدأ] (https://github.com/mre/idiomatic-rust).
- استخدم النمط `mod.rs`. لن تجتاز [الوحدات النمطية ذاتية التسمية](https://rust-lang.github.io/rust-clippy/master/) التحليل الثابت، باستثناء اختبارات [`trybuild`](https://crates.io/crates/trybuild).
- استخدم بنية الوحدات النمطية للمجال أولاً.

  مثال: لا تفعل `constants::logger`. بدلاً من ذلك، قم بعكس التسلسل الهرمي، مع وضع الكائن الذي يتم استخدامه له أولاً: `iroha_logger::constants`.
- استخدم [`expect`](https://learning-rust.github.io/docs/unwrap-and-expect/) مع رسالة خطأ صريحة أو إثبات العصمة بدلاً من `unwrap`.
- لا تتجاهل الخطأ أبدًا. إذا لم تتمكن من `panic` ولا يمكنك استرداده، فيجب على الأقل تسجيله في السجل.
- يفضل إرجاع `Result` بدلاً من `panic!`.
- وظائف المجموعة ذات الصلة مكانيًا، ويفضل أن يكون ذلك داخل الوحدات المناسبة.

  على سبيل المثال، بدلاً من وجود كتلة تحتوي على تعريفات `struct` ثم `impl`s لكل بنية فردية، فمن الأفضل أن يكون `impl`s المرتبط بهذا `struct` بجوارها.
- أعلن قبل التنفيذ: عبارات وثوابت `use` في الأعلى، واختبارات الوحدة في الأسفل.
- حاول تجنب عبارات `use` إذا تم استخدام الاسم المستورد مرة واحدة فقط. وهذا يجعل نقل التعليمات البرمجية الخاصة بك إلى ملف مختلف أسهل.
- لا تقم بإسكات الوبر `clippy` بشكل عشوائي. إذا قمت بذلك، اشرح أسبابك من خلال تعليق (أو رسالة `expect`).
- تفضل `#[outer_attribute]` إلى `#![inner_attribute]` إذا كان أي منهما متاحًا.
- إذا لم تقم وظيفتك بتغيير أي من مدخلاتها (ولا ينبغي لها تغيير أي شيء آخر)، فضع علامة عليها كـ `#[must_use]`.
- تجنب `Box<dyn Error>` إن أمكن (نفضل الكتابة القوية).
- إذا كانت وظيفتك عبارة عن أداة getter/setter، فقم بوضع علامة عليها `#[inline]`.
- إذا كانت وظيفتك منشئة (أي أنها تنشئ قيمة جديدة من معلمات الإدخال وتستدعي `default()`)، فضع علامة عليها `#[inline]`.
- تجنب ربط التعليمات البرمجية الخاصة بك بهياكل بيانات محددة؛ يعد `rustc` ذكيًا بما يكفي لتحويل `Vec<InstructionExpr>` إلى `impl IntoIterator<Item = InstructionExpr>` والعكس عندما يحتاج الأمر إلى ذلك.

إرشادات التسمية:
- استخدم الكلمات الكاملة فقط في أسماء البنية *العامة* والمتغير والطريقة والسمات والثابت وأسماء الوحدات. ومع ذلك، يُسمح بالاختصارات إذا:
  - الاسم محلي (مثل وسيطات الإغلاق).
  - يتم اختصار الاسم حسب اصطلاح Rust (على سبيل المثال `len`، `typ`).
  - الاسم هو اختصار مقبول (مثل `tx`، `wsv` وما إلى ذلك)؛ راجع [مسرد المشروع] (https://docs.iroha.tech/reference/glossary.html) للتعرف على الاختصارات الأساسية.
  - الاسم الكامل قد تم تظليله بواسطة متغير محلي (على سبيل المثال `msg <- message`).
  - الاسم الكامل قد يجعل الكود مرهقًا لأنه يحتوي على أكثر من 5-6 كلمات (على سبيل المثال `WorldStateViewReceiverTrait -> WSVRecvTrait`).
- إذا قمت بتغيير اصطلاحات التسمية، فتأكد من أن الاسم الجديد الذي اخترته أكثر وضوحًا مما كان لدينا من قبل.

إرشادات التعليق:
- عند كتابة تعليقات غير مستندية، بدلاً من وصف *ما* تقوم به وظيفتك، حاول شرح *سبب* قيامها بشيء ما بطريقة معينة. وهذا سيوفر لك ووقت المراجع.
- يمكنك ترك علامات `TODO` في الكود طالما أنك تشير إلى مشكلة قمت بإنشائها لها. عدم إنشاء مشكلة يعني عدم دمجها.

نحن نستخدم التبعيات المثبتة. اتبع هذه الإرشادات للإصدار:

- إذا كان عملك يعتمد على صندوق معين، فتأكد مما إذا لم يتم تثبيته بالفعل باستخدام [`cargo tree`](https://doc.rust-lang.org/cargo/commands/cargo-tree.html) (استخدم `bat` أو `grep`)، وحاول استخدام هذا الإصدار، بدلاً من الإصدار الأحدث.
- استخدم الإصدار الكامل "X.Y.Z" في `Cargo.toml`.
- تقديم المطبات الإصدار في العلاقات العامة منفصلة.

</التفاصيل>

### دليل أسلوب التوثيق

<details> <summary> :كتاب: اقرأ إرشادات التوثيق</summary>


- استخدم التنسيق [`Rust Docs`](https://doc.rust-lang.org/cargo/commands/cargo-doc.html).
- تفضل صيغة التعليق المكونة من سطر واحد. استخدم `///` أعلاه الوحدات النمطية المضمنة و`//!` للوحدات النمطية القائمة على الملفات.
- إذا كان بإمكانك الارتباط بمستندات البنية/الوحدة النمطية/الوظيفة، فافعل ذلك.
- إذا كان بإمكانك تقديم مثال للاستخدام، فافعل ذلك. هذا [أيضًا اختبار](https://doc.rust-lang.org/rustdoc/documentation-tests.html).
- إذا كانت الوظيفة يمكن أن تخطئ أو تثير الذعر، فتجنب الأفعال الشرطية. مثال: `Fails if disk IO fails` بدلاً من `Can possibly fail, if disk IO happens to fail`.
- إذا كان من الممكن أن تخطئ إحدى الوظائف أو تصاب بالذعر لأكثر من سبب واحد، فاستخدم قائمة ذات تعداد نقطي لحالات الفشل، مع متغيرات `Error` المناسبة (إن وجدت).
- وظائف * تفعل * الأشياء. استخدم المزاج الحتمي.
- الهياكل *هي* الأشياء. نصل الى هذه النقطة. على سبيل المثال، `Log level for reloading from the environment` أفضل من `This struct encapsulates the idea of logging levels, and is used for reloading from the environment`.
- الهياكل لها حقول، وهي أيضًا *أشياء*.
- الوحدات *تحتوي* على أشياء، ونحن نعلم ذلك. نصل الى هذه النقطة. مثال: استخدم `Logger-related traits.` بدلاً من `Module which contains logger-related logic`.


</التفاصيل>

## الاتصال

أعضاء مجتمعنا ينشطون في:

| الخدمة | الرابط |
|--------------|--------------------------------------------------------------------|
| ستاكوفرفلوو | https://stackoverflow.com/questions/tagged/hyperledger-iroha |
| القائمة البريدية | https://lists.lfdecentralizedtrust.org/g/iroha |
| برقية | https://t.me/hyperledgeriroha |
| الخلاف | https://discord.com/channels/905194001349627914/905205848547155968 |

---